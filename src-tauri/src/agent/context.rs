use serde::{Deserialize, Serialize};

use crate::engine::models::{Pipeline, PipelineRun, RunStatus, StageStatus};

/// Regex patterns for common secret values that should be redacted from logs
/// before inclusion in LLM prompts.
static SECRET_PATTERNS: &[&str] = &[
    // Generic API keys / tokens (20+ alphanumeric chars after a key-like prefix)
    r"(?i)(password|passwd|secret|token|api[_-]?key|apikey|auth|credential|private[_-]?key)\s*[:=]\s*\S+",
    // AWS-style keys
    r"(?i)AKIA[0-9A-Z]{16}",
    // Bearer tokens
    r"(?i)bearer\s+[a-zA-Z0-9\-._~+/]+=*",
    // Base64-encoded long strings that look like secrets (64+ chars)
    r"[A-Za-z0-9+/]{64,}={0,3}",
];

/// Sanitize a log line by redacting potential secrets and escaping markdown
/// code fence breaks that could enable prompt injection.
fn sanitize_log_line(line: &str) -> String {
    let mut sanitized = line.to_string();

    // 1. Redact secret-like patterns
    for pattern in SECRET_PATTERNS {
        if let Ok(re) = regex::Regex::new(pattern) {
            sanitized = re.replace_all(&sanitized, "[REDACTED]").to_string();
        }
    }

    // 2. Escape backtick sequences that could break out of markdown code fences
    sanitized = sanitized.replace("```", "` ` `");

    sanitized
}

/// Context provided to the agent for analysis or chat.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnalysisContext {
    /// The pipeline run being analyzed (if any).
    pub run: Option<PipelineRun>,
    /// The pipeline definition.
    pub pipeline_def: Option<Pipeline>,
    /// Detected project types (e.g., ["node", "rust"]).
    pub project_types: Vec<String>,
    /// Git branch name.
    pub branch: Option<String>,
    /// Recent git commits (short hash + message).
    pub recent_commits: Vec<String>,
    /// Project path on disk.
    pub project_path: Option<String>,
    /// Relevant memories for this project.
    pub memories: Vec<MemoryContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryContext {
    pub key: String,
    pub value: String,
}

impl AnalysisContext {
    pub fn empty() -> Self {
        Self::default()
    }

    /// Build from a pipeline run, adding relevant log excerpts.
    pub fn from_run(run: &PipelineRun, pipeline: Option<&Pipeline>) -> Self {
        Self {
            run: Some(run.clone()),
            pipeline_def: pipeline.cloned(),
            branch: run.branch.clone(),
            ..Default::default()
        }
    }

    /// Convert to a prompt section string for injection into the system prompt.
    pub fn to_prompt_section(&self) -> String {
        let mut parts = Vec::new();

        if let Some(path) = &self.project_path {
            parts.push(format!("**Project path:** {}", path));
        }

        if !self.project_types.is_empty() {
            parts.push(format!(
                "**Project types:** {}",
                self.project_types.join(", ")
            ));
        }

        if let Some(branch) = &self.branch {
            parts.push(format!("**Branch:** {}", branch));
        }

        if !self.recent_commits.is_empty() {
            parts.push("**Recent commits:**".to_string());
            for commit in self.recent_commits.iter().take(10) {
                parts.push(format!("- {}", commit));
            }
        }

        if !self.memories.is_empty() {
            parts.push("**Remembered facts:**".to_string());
            for mem in &self.memories {
                parts.push(format!("- {}: {}", mem.key, mem.value));
            }
        }

        if let Some(run) = &self.run {
            parts.push(format!(
                "\n**Pipeline run:** {} (status: {:?})",
                run.pipeline_name, run.status
            ));

            if let Some(env) = &run.environment {
                parts.push(format!("**Environment:** {}", env));
            }

            if let Some(dur) = run.duration_ms {
                parts.push(format!("**Duration:** {}ms", dur));
            }

            // Include stage results with truncated logs
            for stage in &run.stage_results {
                let status_icon = match stage.status {
                    StageStatus::Success => "✓",
                    StageStatus::Failed => "✗",
                    StageStatus::Skipped => "⊘",
                    StageStatus::Running => "⟳",
                    StageStatus::Pending => "○",
                };

                parts.push(format!(
                    "\n### Stage: {} [{}] (exit: {:?}, {:?}ms)",
                    stage.stage_name,
                    status_icon,
                    stage.exit_code,
                    stage.duration_ms
                ));

                // Truncate logs to last 50 lines each to manage context window
                let stdout_lines: Vec<&str> = stage.stdout.lines().collect();
                let stderr_lines: Vec<&str> = stage.stderr.lines().collect();

                if !stdout_lines.is_empty() {
                    let start = stdout_lines.len().saturating_sub(50);
                    parts.push("**stdout** (last 50 lines):".to_string());
                    parts.push("```".to_string());
                    for line in &stdout_lines[start..] {
                        parts.push(sanitize_log_line(line));
                    }
                    parts.push("```".to_string());
                }

                if !stderr_lines.is_empty() {
                    let start = stderr_lines.len().saturating_sub(50);
                    parts.push("**stderr** (last 50 lines):".to_string());
                    parts.push("```".to_string());
                    for line in &stderr_lines[start..] {
                        parts.push(sanitize_log_line(line));
                    }
                    parts.push("```".to_string());
                }
            }
        }

        if let Some(pipeline) = &self.pipeline_def {
            parts.push(format!("\n**Pipeline definition:** {}", pipeline.name));
            for stage in &pipeline.stages {
                parts.push(format!(
                    "- Stage '{}': {} commands, backend: {:?}",
                    stage.name,
                    stage.commands.len(),
                    stage.backend
                ));
            }
        }

        if parts.is_empty() {
            "No additional context available.".to_string()
        } else {
            parts.join("\n")
        }
    }

    /// Check if the run has any failed stages.
    pub fn has_failed_stages(&self) -> bool {
        self.run
            .as_ref()
            .map(|r| {
                r.stage_results
                    .iter()
                    .any(|s| s.status == StageStatus::Failed)
            })
            .unwrap_or(false)
    }

    /// Check if the run failed on a deploy stage (heuristic: stage name contains "deploy").
    pub fn failed_on_deploy(&self) -> bool {
        self.run
            .as_ref()
            .map(|r| {
                r.stage_results.iter().any(|s| {
                    s.status == StageStatus::Failed
                        && s.stage_name.to_lowercase().contains("deploy")
                })
            })
            .unwrap_or(false)
    }

    /// Check if the run status is failed.
    pub fn is_failed_run(&self) -> bool {
        self.run
            .as_ref()
            .map(|r| r.status == RunStatus::Failed)
            .unwrap_or(false)
    }
}
