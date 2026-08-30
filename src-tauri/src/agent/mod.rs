pub mod ci_edit;
pub mod ci_files;
pub mod command_exec;
pub mod context;
pub mod executor;
pub mod project_status;
pub mod skills;
pub mod tool_loop;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::ai::identity_loader::AgentIdentityRegistry;
use crate::ai::provider::{ChatMessage, LLMProvider};

/// A single conversation turn crossing the Tauri boundary. Re-exported alias of
/// the provider's `ChatMessage`.
pub type ChatTurn = ChatMessage;

pub use context::AnalysisContext;

// ---------------------------------------------------------------------------
// Skill modes — detected from context + keywords
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillMode {
    FailureAnalysis,
    PipelineOptimization,
    SecurityReview,
    DeployTroubleshoot,
    ProjectSetup,
    PipelineGenerate,
    Execute,
    General,
}

impl std::fmt::Display for SkillMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FailureAnalysis => write!(f, "failure_analysis"),
            Self::PipelineOptimization => write!(f, "pipeline_optimization"),
            Self::SecurityReview => write!(f, "security_review"),
            Self::DeployTroubleshoot => write!(f, "deploy_troubleshoot"),
            Self::ProjectSetup => write!(f, "project_setup"),
            Self::PipelineGenerate => write!(f, "pipeline_generate"),
            Self::Execute => write!(f, "execute"),
            Self::General => write!(f, "general"),
        }
    }
}

// ---------------------------------------------------------------------------
// Agent output types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub severity: Severity,
    pub title: String,
    pub detail: String,
    pub suggested_command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAnalysis {
    pub summary: String,
    pub findings: Vec<Finding>,
    pub suggested_actions: Vec<String>,
    pub skill_used: SkillMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub message: String,
    pub suggestions: Vec<String>,
    pub skill_used: SkillMode,
}

/// Pipeline generation output formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineFormat {
    Chibby,
    GithubActions,
    CircleCi,
    Drone,
}

impl std::fmt::Display for PipelineFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Chibby => write!(f, "Chibby (TOML)"),
            Self::GithubActions => write!(f, "GitHub Actions (YAML)"),
            Self::CircleCi => write!(f, "CircleCI (YAML)"),
            Self::Drone => write!(f, "Drone (YAML)"),
        }
    }
}

impl PipelineFormat {
    /// Map to the shared `CiFormat`. Generation targets are a subset of editable
    /// formats, so this keeps the two enums consistent (paths, validation) from
    /// one source instead of a divergent second copy.
    pub fn as_ci_format(self) -> ci_files::CiFormat {
        match self {
            Self::Chibby => ci_files::CiFormat::Chibby,
            Self::GithubActions => ci_files::CiFormat::GithubActions,
            Self::CircleCi => ci_files::CiFormat::CircleCi,
            Self::Drone => ci_files::CiFormat::Drone,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedPipeline {
    pub format: PipelineFormat,
    pub file_path: String,
    pub content: String,
    pub explanation: String,
}

/// Deploy stage approval status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageApproval {
    AutoApproved,
    AwaitingApproval(String),
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub run_id: String,
    pub status: crate::engine::models::RunStatus,
    pub stages_completed: Vec<String>,
    pub approval_required: Option<String>,
}

// ---------------------------------------------------------------------------
// ChibbyAgent — the single CI/CD agent
// ---------------------------------------------------------------------------

/// Build the default agent from the configured provider preference, API keys,
/// and identity files. Shared by the Tauri commands and the CLI `--ai` flags.
/// Errors if no provider is configured.
pub fn build_agent() -> Result<ChibbyAgent> {
    use crate::ai::identity_loader::{resolve_identity_path, AgentIdentityRegistry};

    let provider = crate::ai::provider::build_provider()?;
    let identity = match resolve_identity_path() {
        Some(path) => AgentIdentityRegistry::load_from_dir(&path)?,
        None => AgentIdentityRegistry::load_fallback(),
    };
    Ok(ChibbyAgent::new(provider, identity))
}

pub struct ChibbyAgent {
    provider: Arc<dyn LLMProvider>,
    identity: AgentIdentityRegistry,
}

impl ChibbyAgent {
    pub fn new(provider: Arc<dyn LLMProvider>, identity: AgentIdentityRegistry) -> Self {
        Self { provider, identity }
    }

    /// Detect the appropriate skill mode from user message + run context.
    pub fn detect_skill(&self, msg: &str, ctx: &AnalysisContext) -> SkillMode {
        skills::detect_skill_mode(msg, ctx)
    }

    /// Clone-able handle to the underlying provider (for the tool loop).
    pub fn provider(&self) -> Arc<dyn LLMProvider> {
        self.provider.clone()
    }

    /// System prompt for the action-taking tool loop. Uses the lean assembly
    /// (no advisory output-format/memory sections), injects the guidance for the
    /// skill detected from the user's message, and appends tool-use rules +
    /// project context (which includes the CI/CD status summary).
    pub fn tool_system_prompt(&self, msg: &str, ctx: &AnalysisContext, read_only: bool) -> String {
        let base = self.identity.assemble_prompt_for_tools();
        let skill = self.detect_skill(msg, ctx);
        let skill_guidance = skills::skill_guidance(&skill);
        let context_section = ctx.to_prompt_section();

        let tool_use = if read_only {
            "## Tool use — Advise mode (read-only)\n\
             You have read-only tools: read_file, list_dir, validate_pipeline. You CANNOT\n\
             run commands or edit files in this mode.\n\
             - Investigate: read the relevant files, list directories, validate the pipeline.\n\
             - Explain the issue and the recommended fix in plain terms.\n\
             - Propose the exact commands to run and/or the complete CI/CD file changes, so\n\
             the user can review them — but do not attempt to execute or write anything.\n\
             - Stay strictly within CI/CD; decline unrelated work and offer the CI/CD angle.\n\
             - End with concrete, copy-pasteable next steps, and note that switching to Act\n\
             lets you apply them with approval."
        } else {
            "## Tool use — Act mode\n\
             You run shell commands and edit CI/CD config files with the provided tools.\n\
             - Read files and run safe checks (build/test/lint) before changing anything.\n\
             - Diagnose and propose the change first, then apply the smallest change that\n\
             fixes it; when editing a CI/CD file, pass the COMPLETE new file content.\n\
             - Risky commands (deploy/push/destructive) and file edits may require user\n\
             approval depending on the configured autonomy mode; wait for the tool result\n\
             and, if an action is rejected, adapt and continue.\n\
             - Stay strictly within CI/CD; decline unrelated work and offer the CI/CD angle.\n\
             - When the task is done, give a short summary of what you did."
        };

        format!(
            "{base}\n\n---\n\n## Current focus: {skill}\n\n{skill_guidance}\n\n\
             ---\n\n{tool_use}\n\n\
             ---\n\n## Context\n\n{context_section}"
        )
    }

    /// Build the full system prompt for a given skill mode.
    fn build_prompt(&self, skill: &SkillMode, ctx: &AnalysisContext, is_first_run: bool) -> String {
        let base_prompt = self.identity.assemble_prompt(is_first_run);
        let skill_guidance = skills::skill_guidance(skill);
        let context_section = ctx.to_prompt_section();

        format!(
            "{}\n\n---\n\n## Current Skill: {}\n\n{}\n\n---\n\n## Context\n\n{}",
            base_prompt, skill, skill_guidance, context_section
        )
    }

    /// Analyze a pipeline run (auto-detects skill from run status).
    pub async fn analyze(&self, ctx: AnalysisContext) -> Result<AgentAnalysis> {
        let skill = skills::detect_skill_from_context(&ctx);
        let system_prompt = self.build_prompt(&skill, &ctx, false);

        let user_message = match &ctx.run {
            Some(run) => format!(
                "Analyze this pipeline run. Pipeline: '{}', Status: {:?}, Stages: {}",
                run.pipeline_name,
                run.status,
                run.stage_results.len()
            ),
            None => "Analyze the current project state.".to_string(),
        };

        let response = self
            .provider
            .complete(&system_prompt, &user_message)
            .await?;
        Ok(parse_analysis_response(&response, skill))
    }

    /// Chat with the agent about CI/CD topics (single turn, no history).
    pub async fn chat(
        &self,
        msg: &str,
        ctx: AnalysisContext,
        is_first_run: bool,
    ) -> Result<AgentResponse> {
        self.chat_with_history(msg, &[], ctx, is_first_run).await
    }

    /// Chat with the agent, threading prior conversation turns for context.
    /// `history` holds earlier turns (roles "user"/"assistant"); `msg` is the
    /// new user message appended after it.
    pub async fn chat_with_history(
        &self,
        msg: &str,
        history: &[ChatTurn],
        ctx: AnalysisContext,
        is_first_run: bool,
    ) -> Result<AgentResponse> {
        let skill = self.detect_skill(msg, &ctx);
        let system_prompt = self.build_prompt(&skill, &ctx, is_first_run);

        let mut messages: Vec<ChatMessage> = history.to_vec();
        messages.push(ChatMessage::user(msg));

        let response = self
            .provider
            .complete_conversation(&system_prompt, &messages)
            .await?;
        Ok(AgentResponse {
            message: response,
            suggestions: Vec::new(),
            skill_used: skill,
        })
    }

    /// Generate a pipeline config for a project.
    pub async fn generate_pipeline(
        &self,
        project_path: &str,
        format: PipelineFormat,
        project_info: &str,
    ) -> Result<GeneratedPipeline> {
        let system_prompt = self.build_prompt(
            &SkillMode::PipelineGenerate,
            &AnalysisContext::empty(),
            false,
        );

        let user_message = format!(
            "Generate a {} pipeline config for this project.\n\
             Project path: {}\n\
             Project info:\n{}\n\n\
             Return ONLY the pipeline config file content in a fenced code block, \
             followed by a brief explanation of each stage.",
            format, project_path, project_info
        );

        let response = self
            .provider
            .complete(&system_prompt, &user_message)
            .await?;
        let (content, explanation) = parse_generated_pipeline(&response);

        let file_path = format.as_ci_format().default_path().to_string();

        Ok(GeneratedPipeline {
            format,
            file_path,
            content,
            explanation,
        })
    }
}

// ---------------------------------------------------------------------------
// Response parsing helpers
// ---------------------------------------------------------------------------

fn parse_analysis_response(response: &str, skill: SkillMode) -> AgentAnalysis {
    // Simple parsing: extract the response as-is for now.
    // A more sophisticated parser could extract structured findings from markdown.
    AgentAnalysis {
        summary: extract_summary(response),
        findings: extract_findings(response),
        suggested_actions: extract_actions(response),
        skill_used: skill,
    }
}

fn extract_summary(response: &str) -> String {
    // Take the first paragraph as summary
    response
        .split("\n\n")
        .next()
        .unwrap_or(response)
        .trim()
        .to_string()
}

fn extract_findings(response: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    let lines: Vec<&str> = response.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Look for severity markers
        let severity = if trimmed.starts_with("**Critical") || trimmed.contains("🔴") {
            Some(Severity::Critical)
        } else if trimmed.starts_with("**Warning") || trimmed.contains("🟡") {
            Some(Severity::Warning)
        } else if trimmed.starts_with("**Info") || trimmed.contains("🔵") {
            Some(Severity::Info)
        } else {
            None
        };

        if let Some(sev) = severity {
            let title = trimmed
                .trim_start_matches("**")
                .trim_start_matches("Critical")
                .trim_start_matches("Warning")
                .trim_start_matches("Info")
                .trim_start_matches("**")
                .trim_start_matches(':')
                .trim_start_matches(" — ")
                .trim_start_matches("- ")
                .trim()
                .to_string();

            // Gather detail from following lines
            let detail = lines
                .get(i + 1..std::cmp::min(i + 4, lines.len()))
                .map(|ls| ls.join("\n"))
                .unwrap_or_default()
                .trim()
                .to_string();

            // Look for a suggested command in backticks
            let suggested_command = detail.lines().find(|l| l.contains('`')).and_then(|l| {
                let start = l.find('`')? + 1;
                let end = l[start..].find('`')? + start;
                Some(l[start..end].to_string())
            });

            findings.push(Finding {
                severity: sev,
                title,
                detail,
                suggested_command,
            });
        }
    }

    // If no structured findings found, create a single info finding from the response
    if findings.is_empty() && !response.trim().is_empty() {
        findings.push(Finding {
            severity: Severity::Info,
            title: "Analysis".to_string(),
            detail: response.trim().to_string(),
            suggested_command: None,
        });
    }

    findings
}

fn extract_actions(response: &str) -> Vec<String> {
    let mut actions = Vec::new();
    let mut in_actions = false;

    for line in response.lines() {
        let trimmed = line.trim();

        if trimmed.to_lowercase().contains("suggested action")
            || trimmed.to_lowercase().contains("next step")
            || trimmed.to_lowercase().contains("recommended action")
        {
            in_actions = true;
            continue;
        }

        if in_actions {
            if trimmed.is_empty() {
                in_actions = false;
                continue;
            }
            let action = trimmed
                .trim_start_matches(|c: char| {
                    c.is_ascii_digit() || c == '.' || c == '-' || c == '*' || c == ' '
                })
                .trim()
                .to_string();
            if !action.is_empty() {
                actions.push(action);
            }
        }
    }

    actions
}

fn parse_generated_pipeline(response: &str) -> (String, String) {
    // Extract content between first code fence, and everything else as explanation
    let mut content = String::new();
    let mut explanation = String::new();
    let mut in_code_block = false;
    let mut found_code = false;

    for line in response.lines() {
        if line.trim().starts_with("```") {
            if in_code_block {
                in_code_block = false;
                found_code = true;
                continue;
            } else if !found_code {
                in_code_block = true;
                continue;
            }
        }

        if in_code_block {
            content.push_str(line);
            content.push('\n');
        } else if found_code {
            explanation.push_str(line);
            explanation.push('\n');
        }
    }

    // If no code block found, treat entire response as content
    if content.is_empty() {
        content = response.to_string();
    }

    (content.trim().to_string(), explanation.trim().to_string())
}
