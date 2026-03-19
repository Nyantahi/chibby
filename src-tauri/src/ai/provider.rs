use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::engine::app_settings;

// ---------------------------------------------------------------------------
// LLM Provider trait
// ---------------------------------------------------------------------------

#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Send a completion request with a system prompt and user message.
    async fn complete(&self, system_prompt: &str, user_message: &str) -> Result<String>;

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
            model: model.unwrap_or_else(|| "claude-sonnet-4-20250514".to_string()),
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

#[async_trait]
impl LLMProvider for AnthropicProvider {
    async fn complete(&self, system_prompt: &str, user_message: &str) -> Result<String> {
        let api_key = Self::get_api_key()?;

        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            system: system_prompt.to_string(),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: user_message.to_string(),
            }],
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
    async fn complete(&self, system_prompt: &str, user_message: &str) -> Result<String> {
        let api_key = Self::get_api_key()?;

        let request = OpenAIRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            messages: vec![
                OpenAIMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                OpenAIMessage {
                    role: "user".to_string(),
                    content: user_message.to_string(),
                },
            ],
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
    async fn complete(&self, system_prompt: &str, user_message: &str) -> Result<String> {
        match self.primary.complete(system_prompt, user_message).await {
            Ok(result) => Ok(result),
            Err(primary_err) => {
                log::warn!(
                    "Primary provider ({}) failed: {}. Trying fallback ({}).",
                    self.primary.name(),
                    primary_err,
                    self.fallback.name()
                );
                self.fallback
                    .complete(system_prompt, user_message)
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

/// Build an LLM provider based on which API keys are configured.
/// Prefers Anthropic as primary with OpenAI fallback when both are available.
pub fn build_provider() -> Result<Arc<dyn LLMProvider>> {
    let has_anthropic = app_settings::has_app_secret("anthropic");
    let has_openai = app_settings::has_app_secret("openai");

    match (has_anthropic, has_openai) {
        (true, true) => {
            let primary: Arc<dyn LLMProvider> = Arc::new(AnthropicProvider::new(None));
            let fallback: Arc<dyn LLMProvider> = Arc::new(OpenAIProvider::new(None));
            Ok(Arc::new(FallbackProvider::new(primary, fallback)))
        }
        (true, false) => Ok(Arc::new(AnthropicProvider::new(None))),
        (false, true) => Ok(Arc::new(OpenAIProvider::new(None))),
        (false, false) => {
            anyhow::bail!(
                "No AI provider configured. Add an Anthropic or OpenAI API key in Settings."
            )
        }
    }
}
