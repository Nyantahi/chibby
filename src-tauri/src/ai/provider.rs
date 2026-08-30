use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::engine::app_settings;

// ---------------------------------------------------------------------------
// Rate limiter — token-bucket with per-minute cap
// ---------------------------------------------------------------------------

/// Simple token-bucket rate limiter for AI API calls.
struct RateLimiter {
    /// Maximum calls allowed per window.
    max_calls: u32,
    /// Window duration.
    window: std::time::Duration,
    /// Timestamps of recent calls within the window.
    call_times: Vec<std::time::Instant>,
}

impl RateLimiter {
    fn new(max_calls: u32, window: std::time::Duration) -> Self {
        Self {
            max_calls,
            window,
            call_times: Vec::new(),
        }
    }

    /// Check if a call is allowed and record it. Returns Err if rate-limited.
    fn try_acquire(&mut self) -> Result<()> {
        let now = std::time::Instant::now();
        // Remove timestamps outside the window
        self.call_times
            .retain(|t| now.duration_since(*t) < self.window);
        if self.call_times.len() >= self.max_calls as usize {
            anyhow::bail!(
                "Rate limit exceeded: max {} AI API calls per {} seconds. Please wait before trying again.",
                self.max_calls,
                self.window.as_secs()
            );
        }
        self.call_times.push(now);
        Ok(())
    }
}

/// Wraps any LLMProvider with rate limiting.
pub struct RateLimitedProvider {
    inner: Arc<dyn LLMProvider>,
    limiter: Mutex<RateLimiter>,
}

impl RateLimitedProvider {
    pub fn new(inner: Arc<dyn LLMProvider>, max_calls_per_minute: u32) -> Self {
        Self {
            inner,
            limiter: Mutex::new(RateLimiter::new(
                max_calls_per_minute,
                std::time::Duration::from_secs(60),
            )),
        }
    }
}

#[async_trait]
impl LLMProvider for RateLimitedProvider {
    async fn complete_conversation(
        &self,
        system_prompt: &str,
        messages: &[ChatMessage],
    ) -> Result<String> {
        {
            let mut limiter = self.limiter.lock().await;
            limiter.try_acquire()?;
        }
        self.inner
            .complete_conversation(system_prompt, messages)
            .await
    }

    async fn complete_with_tools(
        &self,
        system_prompt: &str,
        messages: &[ToolMessage],
        tools: &[ToolDef],
    ) -> Result<ProviderTurn> {
        {
            let mut limiter = self.limiter.lock().await;
            limiter.try_acquire()?;
        }
        self.inner
            .complete_with_tools(system_prompt, messages, tools)
            .await
    }

    fn name(&self) -> &str {
        self.inner.name()
    }
}

// ---------------------------------------------------------------------------
// LLM Provider trait
// ---------------------------------------------------------------------------

/// A single turn in a conversation. `role` is "user" or "assistant".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tool-calling types (Stage B agentic loop)
// ---------------------------------------------------------------------------

/// A tool the model may call.
#[derive(Debug, Clone, Serialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// A block of content within a tool-enabled message or model response.
/// Serializes to the Anthropic Messages API content-block shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default)]
        is_error: bool,
    },
}

/// A conversation turn carrying structured content blocks (tool loop).
#[derive(Debug, Clone, Serialize)]
pub struct ToolMessage {
    pub role: String,
    pub content: Vec<ContentBlock>,
}

impl ToolMessage {
    pub fn user_text(text: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }
    pub fn assistant(content: Vec<ContentBlock>) -> Self {
        Self {
            role: "assistant".to_string(),
            content,
        }
    }
    /// A user turn carrying tool results.
    pub fn tool_results(results: Vec<ContentBlock>) -> Self {
        Self {
            role: "user".to_string(),
            content: results,
        }
    }
}

/// One assistant turn from a tool-enabled completion.
#[derive(Debug, Clone)]
pub struct ProviderTurn {
    pub blocks: Vec<ContentBlock>,
    pub stop_reason: String,
}

impl ProviderTurn {
    /// The `(id, name, input)` of each tool_use block in this turn.
    pub fn tool_uses(&self) -> Vec<(String, String, serde_json::Value)> {
        self.blocks
            .iter()
            .filter_map(|b| match b {
                ContentBlock::ToolUse { id, name, input } => {
                    Some((id.clone(), name.clone(), input.clone()))
                }
                _ => None,
            })
            .collect()
    }

    /// Concatenated text blocks (the model's prose for this turn).
    pub fn text(&self) -> String {
        self.blocks
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Send a multi-turn conversation with a system prompt.
    async fn complete_conversation(
        &self,
        system_prompt: &str,
        messages: &[ChatMessage],
    ) -> Result<String>;

    /// Convenience: single user message (delegates to `complete_conversation`).
    async fn complete(&self, system_prompt: &str, user_message: &str) -> Result<String> {
        self.complete_conversation(system_prompt, &[ChatMessage::user(user_message)])
            .await
    }

    /// Send a tool-enabled conversation. Providers without native tool support
    /// return an error (the default).
    async fn complete_with_tools(
        &self,
        _system_prompt: &str,
        _messages: &[ToolMessage],
        _tools: &[ToolDef],
    ) -> Result<ProviderTurn> {
        anyhow::bail!(
            "Tool use is not supported by the '{}' provider. Configure an Anthropic API key in Settings.",
            self.name()
        )
    }

    /// Return the provider name for logging.
    fn name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Anthropic provider
// ---------------------------------------------------------------------------

pub struct AnthropicProvider {
    client: reqwest::Client,
    model: String,
}

impl AnthropicProvider {
    pub fn new(model: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            model: model.unwrap_or_else(|| app_settings::DEFAULT_ANTHROPIC_MODEL.to_string()),
        }
    }

    fn get_api_key() -> Result<String> {
        app_settings::get_app_secret("anthropic")
            .context("Anthropic API key not configured. Add it in Settings.")
    }
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<AnthropicMessage>,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

#[derive(Deserialize)]
struct AnthropicContent {
    text: String,
}

#[derive(Serialize)]
struct AnthropicToolRequest<'a> {
    model: String,
    max_tokens: u32,
    system: String,
    messages: &'a [ToolMessage],
    tools: &'a [ToolDef],
}

#[derive(Deserialize)]
struct AnthropicToolResponse {
    content: Vec<ContentBlock>,
    stop_reason: Option<String>,
}

#[async_trait]
impl LLMProvider for AnthropicProvider {
    async fn complete_conversation(
        &self,
        system_prompt: &str,
        messages: &[ChatMessage],
    ) -> Result<String> {
        let api_key = Self::get_api_key()?;

        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            system: system_prompt.to_string(),
            messages: messages
                .iter()
                .map(|m| AnthropicMessage {
                    role: m.role.clone(),
                    content: m.content.clone(),
                })
                .collect(),
        };

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Anthropic API")?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API error ({}): {}", status, body);
        }

        let parsed: AnthropicResponse = response
            .json()
            .await
            .context("Failed to parse Anthropic response")?;

        parsed
            .content
            .first()
            .map(|c| c.text.clone())
            .context("Empty response from Anthropic")
    }

    async fn complete_with_tools(
        &self,
        system_prompt: &str,
        messages: &[ToolMessage],
        tools: &[ToolDef],
    ) -> Result<ProviderTurn> {
        let api_key = Self::get_api_key()?;

        let request = AnthropicToolRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            system: system_prompt.to_string(),
            messages,
            tools,
        };

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send tool request to Anthropic API")?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API error ({}): {}", status, body);
        }

        let parsed: AnthropicToolResponse = response
            .json()
            .await
            .context("Failed to parse Anthropic tool response")?;

        Ok(ProviderTurn {
            blocks: parsed.content,
            stop_reason: parsed.stop_reason.unwrap_or_else(|| "end_turn".to_string()),
        })
    }

    fn name(&self) -> &str {
        "anthropic"
    }
}

// ---------------------------------------------------------------------------
// OpenAI provider
// ---------------------------------------------------------------------------

pub struct OpenAIProvider {
    client: reqwest::Client,
    model: String,
}

impl OpenAIProvider {
    pub fn new(model: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            model: model.unwrap_or_else(|| "gpt-4o".to_string()),
        }
    }

    fn get_api_key() -> Result<String> {
        app_settings::get_app_secret("openai")
            .context("OpenAI API key not configured. Add it in Settings.")
    }
}

#[derive(Serialize)]
struct OpenAIRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<OpenAIMessage>,
}

#[derive(Serialize)]
struct OpenAIMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
}

#[derive(Deserialize)]
struct OpenAIChoice {
    message: OpenAIResponseMessage,
}

#[derive(Deserialize)]
struct OpenAIResponseMessage {
    content: Option<String>,
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    async fn complete_conversation(
        &self,
        system_prompt: &str,
        messages: &[ChatMessage],
    ) -> Result<String> {
        let api_key = Self::get_api_key()?;

        let mut openai_messages = vec![OpenAIMessage {
            role: "system".to_string(),
            content: system_prompt.to_string(),
        }];
        openai_messages.extend(messages.iter().map(|m| OpenAIMessage {
            role: m.role.clone(),
            content: m.content.clone(),
        }));

        let request = OpenAIRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            messages: openai_messages,
        };

        let response = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to OpenAI API")?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("OpenAI API error ({}): {}", status, body);
        }

        let parsed: OpenAIResponse = response
            .json()
            .await
            .context("Failed to parse OpenAI response")?;

        parsed
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .context("Empty response from OpenAI")
    }

    fn name(&self) -> &str {
        "openai"
    }
}

// ---------------------------------------------------------------------------
// Fallback provider: tries primary, falls back to secondary on error
// ---------------------------------------------------------------------------

pub struct FallbackProvider {
    primary: Arc<dyn LLMProvider>,
    fallback: Arc<dyn LLMProvider>,
}

impl FallbackProvider {
    pub fn new(primary: Arc<dyn LLMProvider>, fallback: Arc<dyn LLMProvider>) -> Self {
        Self { primary, fallback }
    }
}

#[async_trait]
impl LLMProvider for FallbackProvider {
    async fn complete_conversation(
        &self,
        system_prompt: &str,
        messages: &[ChatMessage],
    ) -> Result<String> {
        match self
            .primary
            .complete_conversation(system_prompt, messages)
            .await
        {
            Ok(result) => Ok(result),
            Err(primary_err) => {
                log::warn!(
                    "Primary provider ({}) failed: {}. Trying fallback ({}).",
                    self.primary.name(),
                    primary_err,
                    self.fallback.name()
                );
                self.fallback
                    .complete_conversation(system_prompt, messages)
                    .await
                    .context(format!(
                        "Both providers failed. Primary: {}. Fallback",
                        primary_err
                    ))
            }
        }
    }

    async fn complete_with_tools(
        &self,
        system_prompt: &str,
        messages: &[ToolMessage],
        tools: &[ToolDef],
    ) -> Result<ProviderTurn> {
        match self
            .primary
            .complete_with_tools(system_prompt, messages, tools)
            .await
        {
            Ok(result) => Ok(result),
            Err(primary_err) => {
                log::warn!(
                    "Primary provider ({}) tool call failed: {}. Trying fallback ({}).",
                    self.primary.name(),
                    primary_err,
                    self.fallback.name()
                );
                self.fallback
                    .complete_with_tools(system_prompt, messages, tools)
                    .await
                    .context(format!(
                        "Both providers failed. Primary: {}. Fallback",
                        primary_err
                    ))
            }
        }
    }

    fn name(&self) -> &str {
        "fallback"
    }
}

// ---------------------------------------------------------------------------
// Provider factory: build the best available provider from configured keys
// ---------------------------------------------------------------------------

/// Build an LLM provider based on the configured provider preference and which
/// API keys exist. `Auto` prefers Anthropic with an OpenAI fallback; the
/// explicit provider settings force one provider and error if its key is
/// missing. All providers are wrapped with a rate limiter (max 15 calls/minute).
pub fn build_provider() -> Result<Arc<dyn LLMProvider>> {
    use app_settings::AgentProvider;

    let settings = app_settings::load_app_settings().unwrap_or_default();
    let has_anthropic = app_settings::has_app_secret("anthropic");
    let has_openai = app_settings::has_app_secret("openai");

    // Anthropic uses the configured model; OpenAI keeps its built-in default.
    let anthropic = || -> Arc<dyn LLMProvider> {
        Arc::new(AnthropicProvider::new(Some(settings.agent_model.clone())))
    };
    let openai = || -> Arc<dyn LLMProvider> { Arc::new(OpenAIProvider::new(None)) };

    let base: Arc<dyn LLMProvider> = match settings.agent_provider {
        AgentProvider::Anthropic => {
            if !has_anthropic {
                anyhow::bail!(
                    "Anthropic is selected in Settings but no Anthropic API key is configured. \
                     Add a key or switch the provider."
                );
            }
            anthropic()
        }
        AgentProvider::Openai => {
            if !has_openai {
                anyhow::bail!(
                    "OpenAI is selected in Settings but no OpenAI API key is configured. \
                     Add a key or switch the provider."
                );
            }
            openai()
        }
        AgentProvider::Auto => match (has_anthropic, has_openai) {
            (true, true) => Arc::new(FallbackProvider::new(anthropic(), openai())),
            (true, false) => anthropic(),
            (false, true) => openai(),
            (false, false) => anyhow::bail!(
                "No AI provider configured. Add an Anthropic or OpenAI API key in Settings."
            ),
        },
    };

    // Wrap with rate limiter: 15 calls per minute
    Ok(Arc::new(RateLimitedProvider::new(base, 15)))
}
