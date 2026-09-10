//! The agentic tool-use loop. The model plans and calls tools (run commands,
//! read files, validate, edit CI files); each action is gated by the configured
//! autonomy mode via the `ToolLoopHost`, which the command layer implements with
//! Tauri events + approval channels. This module stays decoupled from Tauri.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Serialize;
use serde_json::{json, Value};

use crate::agent::{ci_edit, command_exec};
use crate::ai::provider::{ContentBlock, LLMProvider, ProviderTurn, ToolDef, ToolMessage};
use crate::engine::app_settings::AgentMode;

/// Hard cap on model↔tool round trips per session.
const MAX_ITERATIONS: usize = 12;
/// Max characters returned from a file read (bounds prompt size).
const MAX_READ_CHARS: usize = 60_000;

/// A side-effecting action awaiting user approval.
#[derive(Debug, Clone, Serialize)]
pub struct PendingAction {
    pub id: String,
    pub tool: String,
    pub summary: String,
    pub reason: String,
}

/// Progress events emitted during a session.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolLoopEvent {
    Assistant {
        text: String,
    },
    ToolStart {
        id: String,
        name: String,
        summary: String,
    },
    ToolResult {
        id: String,
        ok: bool,
        output: String,
    },
    AwaitingApproval {
        pending: PendingAction,
    },
    Done {
        message: String,
    },
    Error {
        message: String,
    },
}

/// Host callbacks: emit progress and request approval. Implemented by the
/// command layer (Tauri events + oneshot channels).
#[async_trait]
pub trait ToolLoopHost: Send + Sync {
    fn emit(&self, event: ToolLoopEvent);
    /// Block until the user approves/rejects `pending`. `true` == proceed.
    async fn request_approval(&self, pending: PendingAction) -> bool;
}

/// The tool set exposed to the model. In `read_only` (Advise) mode only the
/// read/inspect tools are offered — the model literally cannot run commands or
/// edit files, so investigation-and-advice is structurally guaranteed.
fn tool_defs(read_only: bool) -> Vec<ToolDef> {
    let mut defs = vec![
        ToolDef {
            name: "read_file".to_string(),
            description: "Read a UTF-8 text file within the project.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {"path": {"type": "string", "description": "Project-relative file path."}},
                "required": ["path"]
            }),
        },
        ToolDef {
            name: "list_dir".to_string(),
            description: "List the entries of a directory within the project.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {"path": {"type": "string", "description": "Project-relative directory path (\".\" for root)."}},
                "required": ["path"]
            }),
        },
        ToolDef {
            name: "validate_pipeline".to_string(),
            description: "Validate the project's Chibby pipeline configuration.".to_string(),
            input_schema: json!({"type": "object", "properties": {}}),
        },
    ];

    if !read_only {
        defs.push(ToolDef {
            name: "run_command".to_string(),
            description: "Run a shell command in the project directory and return its output."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "The shell command to run."},
                    "working_dir": {"type": "string", "description": "Optional subdirectory relative to the project root."}
                },
                "required": ["command"]
            }),
        });
        defs.push(ToolDef {
            name: "edit_ci_file".to_string(),
            description: "Create or replace a CI/CD config file (.chibby/*.toml, \
                 .github/workflows/*.yml, .circleci/config.yml, .drone.yml, .gitlab-ci.yml). \
                 Provide the COMPLETE new file content."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Project-relative CI file path."},
                    "content": {"type": "string", "description": "The full new file content."}
                },
                "required": ["path", "content"]
            }),
        });
    }

    defs
}

/// Run one agentic session to completion (or until it pauses are resolved).
/// Returns the model's final summary text.
pub async fn run_tool_session(
    provider: Arc<dyn LLMProvider>,
    system_prompt: String,
    project_path: String,
    user_message: String,
    mode: AgentMode,
    read_only: bool,
    host: Arc<dyn ToolLoopHost>,
) -> Result<String> {
    let tools = tool_defs(read_only);
    let mut messages = vec![ToolMessage::user_text(user_message)];
    let mut final_text = String::new();

    for _ in 0..MAX_ITERATIONS {
        let turn: ProviderTurn = provider
            .complete_with_tools(&system_prompt, &messages, &tools)
            .await
            .context("Model tool call failed")?;

        let text = turn.text();
        if !text.is_empty() {
            host.emit(ToolLoopEvent::Assistant { text: text.clone() });
            final_text = text;
        }

        let tool_uses = turn.tool_uses();
        if tool_uses.is_empty() {
            host.emit(ToolLoopEvent::Done {
                message: final_text.clone(),
            });
            return Ok(final_text);
        }

        // Keep the assistant turn (incl. tool_use blocks) in history.
        messages.push(ToolMessage::assistant(turn.blocks.clone()));

        let mut results = Vec::new();
        for (id, name, input) in tool_uses {
            host.emit(ToolLoopEvent::ToolStart {
                id: id.clone(),
                name: name.clone(),
                summary: summarize_call(&name, &input),
            });

            let (content, is_error) = match dispatch_tool(
                &project_path,
                &name,
                &input,
                mode,
                read_only,
                &host,
                &id,
            )
            .await
            {
                Ok(s) => (s, false),
                Err(e) => (e, true),
            };

            host.emit(ToolLoopEvent::ToolResult {
                id: id.clone(),
                ok: !is_error,
                output: truncate(&content, 4000),
            });
            results.push(ContentBlock::ToolResult {
                tool_use_id: id,
                content,
                is_error,
            });
        }
        messages.push(ToolMessage::tool_results(results));
    }

    host.emit(ToolLoopEvent::Done {
        message: "Reached the maximum number of steps for this session.".to_string(),
    });
    Ok(final_text)
}

/// Execute a single tool call, applying the autonomy-mode gate. Returns the
/// tool-result content (Ok) or an error string surfaced to the model (Err).
async fn dispatch_tool(
    project_path: &str,
    name: &str,
    input: &Value,
    mode: AgentMode,
    read_only: bool,
    host: &Arc<dyn ToolLoopHost>,
    id: &str,
) -> Result<String, String> {
    // Belt-and-suspenders: in Advise mode the mutating tools aren't even exposed
    // to the model (see `tool_defs`), but reject them here too if one slips through.
    if read_only && matches!(name, "run_command" | "edit_ci_file") {
        return Err(
            "Advise mode is read-only — switch to Act to run commands or edit files.".to_string(),
        );
    }

    match name {
        "run_command" => {
            let command = input
                .get("command")
                .and_then(|v| v.as_str())
                .ok_or("run_command requires a 'command' string")?;
            let working_dir = input
                .get("working_dir")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let risk = command_exec::classify(command);
            let gated = mode == AgentMode::ProposeApprove || risk.is_risky();
            if gated {
                let pending = PendingAction {
                    id: id.to_string(),
                    tool: "run_command".to_string(),
                    summary: command.to_string(),
                    reason: risk
                        .reason()
                        .map(|r| r.to_string())
                        .unwrap_or_else(|| "runs a command".to_string()),
                };
                if !gate(host, pending).await {
                    return Ok("The user rejected this command; it was not run.".to_string());
                }
            }

            let outcome =
                command_exec::execute_agent_command(command, Path::new(project_path), working_dir)
                    .await
                    .map_err(|e| e.to_string())?;
            Ok(format_outcome(&outcome))
        }

        "read_file" => {
            let rel = input
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or("read_file requires a 'path' string")?;
            let full = safe_path(project_path, rel)?;
            let content =
                std::fs::read_to_string(&full).map_err(|e| format!("Failed to read {rel}: {e}"))?;
            Ok(truncate(&content, MAX_READ_CHARS))
        }

        "list_dir" => {
            let rel = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            let full = safe_path(project_path, rel)?;
            let mut entries: Vec<String> = std::fs::read_dir(&full)
                .map_err(|e| format!("Failed to list {rel}: {e}"))?
                .filter_map(|e| e.ok())
                .map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    if e.path().is_dir() {
                        format!("{name}/")
                    } else {
                        name
                    }
                })
                .collect();
            entries.sort();
            Ok(entries.join("\n"))
        }

        "validate_pipeline" => {
            let path = Path::new(project_path);
            let pipeline = crate::engine::pipeline::load_pipeline(path)
                .map_err(|e| format!("No loadable pipeline: {e}"))?;
            let validation = crate::engine::detector::validate_pipeline(&pipeline, path);
            serde_json::to_string_pretty(&validation)
                .map_err(|e| format!("Failed to serialize validation: {e}"))
        }

        "edit_ci_file" => {
            let rel = input
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or("edit_ci_file requires a 'path' string")?;
            let content = input
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or("edit_ci_file requires a 'content' string")?;

            // Validate + diff before gating so the user sees what would change.
            let (_, diff) = ci_edit::preview_ci_edit(project_path, rel, content)?;

            // Edits are gated except in fully-autonomous mode (they're git-safe).
            let gated = mode != AgentMode::AutonomousCheckpoints;
            if gated {
                let pending = PendingAction {
                    id: id.to_string(),
                    tool: "edit_ci_file".to_string(),
                    summary: format!("Edit {rel}\n\n{diff}"),
                    reason: "writes a CI/CD config file".to_string(),
                };
                if !gate(host, pending).await {
                    return Ok("The user rejected this edit; the file was not changed.".to_string());
                }
            }

            let res = ci_edit::edit_ci_file(project_path, rel, content)?;
            Ok(format_edit_result(&res))
        }

        other => Err(format!("Unknown tool '{other}'")),
    }
}

/// Await approval for a pending action. The host is responsible for surfacing
/// the request (registering its channel first to avoid a race) and returning
/// the decision.
async fn gate(host: &Arc<dyn ToolLoopHost>, pending: PendingAction) -> bool {
    host.request_approval(pending).await
}

/// Resolve a project-relative path, ensuring it stays inside the project.
/// Does not create directories (unlike the edit guard).
fn safe_path(project_path: &str, rel: &str) -> Result<std::path::PathBuf, String> {
    if rel.contains("..") || rel.starts_with('/') || rel.starts_with('\\') {
        return Err("path must be relative and within the project".to_string());
    }
    let full = Path::new(project_path).join(rel);
    let canon_project =
        std::fs::canonicalize(project_path).map_err(|e| format!("invalid project: {e}"))?;
    let to_check = if full.exists() {
        full.clone()
    } else {
        full.parent()
            .unwrap_or(Path::new(project_path))
            .to_path_buf()
    };
    let canon = std::fs::canonicalize(&to_check).map_err(|e| format!("invalid path: {e}"))?;
    if !canon.starts_with(&canon_project) {
        return Err("path resolves outside the project".to_string());
    }
    Ok(full)
}

fn summarize_call(name: &str, input: &Value) -> String {
    match name {
        "run_command" => input
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "read_file" | "list_dir" | "edit_ci_file" => input
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    }
}

fn format_outcome(o: &command_exec::CommandOutcome) -> String {
    let mut s = format!("exit_code: {:?}\n", o.exit_code);
    if !o.stdout.is_empty() {
        s.push_str(&format!("--- stdout ---\n{}\n", o.stdout));
    }
    if !o.stderr.is_empty() {
        s.push_str(&format!("--- stderr ---\n{}\n", o.stderr));
    }
    if o.timed_out {
        s.push_str("(command timed out)\n");
    }
    s
}

fn format_edit_result(r: &ci_edit::EditResult) -> String {
    let mut s = format!("Edited {} ({:?}).\n", r.path, r.format);
    if r.git_committed {
        s.push_str(&format!(
            "Committed to branch {} (from {}), sha {}.\n",
            r.branch.as_deref().unwrap_or("?"),
            r.original_branch.as_deref().unwrap_or("?"),
            r.commit_sha.as_deref().unwrap_or("?"),
        ));
    }
    if let Some(note) = &r.note {
        s.push_str(&format!("{note}\n"));
    }
    if let Some(bak) = &r.backup_path {
        s.push_str(&format!("Backup: {bak}\n"));
    }
    s
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n… (truncated)", &s[..end])
}
