//! Tauri commands for local triggers: schedules, watches and git hooks.

use crate::engine::models::PipelineRun;
use crate::engine::trigger_state::{self, TriggerStateEntry};
use crate::engine::triggers::hooks::{self, HookKind, HookState, InstallMode};
use crate::engine::triggers::{self, runner, schedule, HookSpec, TriggersConfig};
use crate::state::SharedPipelineState;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tauri::State;

/// How many upcoming fire times `next_run_times` returns by default.
const DEFAULT_PREVIEW_COUNT: usize = 5;

/// Load `.chibby/triggers.toml` merged with `.chibby/triggers.local.toml`.
#[tauri::command]
pub fn load_triggers(repo_path: String, layered: Option<bool>) -> Result<TriggersConfig, String> {
    let path = Path::new(&repo_path);
    let config = match layered.unwrap_or(true) {
        true => triggers::load_triggers_layered(path),
        false => triggers::load_triggers(path),
    };
    config.map_err(|e| e.to_string())
}

/// Load `.chibby/triggers.local.toml` alone — the per-machine overrides, with
/// nothing merged in. The editor needs this to save back only what that file
/// owns; writing the merged view to it would shadow every later team edit.
#[tauri::command]
pub fn load_triggers_local(repo_path: String) -> Result<TriggersConfig, String> {
    triggers::load_triggers_local(Path::new(&repo_path)).map_err(|e| e.to_string())
}

/// Save triggers to the committed file, or the per-developer local override.
#[tauri::command]
pub fn save_triggers(
    repo_path: String,
    config: TriggersConfig,
    local: Option<bool>,
) -> Result<(), String> {
    let path = Path::new(&repo_path);
    match local.unwrap_or(false) {
        true => triggers::save_triggers_local(path, &config),
        false => triggers::save_triggers(path, &config),
    }
    .map_err(|e| e.to_string())
}

/// Upcoming fire times for a cron expression, so the UI can show "next: ...".
#[tauri::command]
pub fn next_run_times(cron: String, count: Option<usize>) -> Result<Vec<DateTime<Utc>>, String> {
    schedule::next_run_times(&cron, Utc::now(), count.unwrap_or(DEFAULT_PREVIEW_COUNT))
        .map_err(|e| e.to_string())
}

/// Run a configured trigger immediately.
///
/// The run state is passed through so the run shows as in progress and can be
/// cancelled from the window that started it, like any other GUI run.
#[tauri::command]
pub async fn fire_trigger_now(
    pipeline_state: State<'_, SharedPipelineState>,
    repo_path: String,
    trigger_id: String,
) -> Result<PipelineRun, String> {
    runner::fire_trigger_now(
        Path::new(&repo_path),
        &trigger_id,
        Some(pipeline_state.inner().clone()),
    )
    .await
    .map_err(|e| e.to_string())
}

/// Install a git hook for a repo.
#[tauri::command]
pub fn install_git_hooks(
    repo_path: String,
    kind: HookKind,
    spec: HookSpec,
    mode: Option<InstallMode>,
) -> Result<hooks::InstallReport, String> {
    hooks::install(
        &PathBuf::from(repo_path),
        kind,
        &spec,
        mode.unwrap_or_default(),
    )
    .map_err(|e| e.to_string())
}

/// Remove Chibby's block from a git hook, leaving foreign content intact.
#[tauri::command]
pub fn uninstall_git_hooks(repo_path: String, kind: HookKind) -> Result<(), String> {
    hooks::uninstall(&PathBuf::from(repo_path), kind).map_err(|e| e.to_string())
}

/// What is currently installed at a repo's hook path.
#[tauri::command]
pub fn git_hook_status(repo_path: String, kind: HookKind) -> Result<HookState, String> {
    hooks::status(&PathBuf::from(repo_path), kind).map_err(|e| e.to_string())
}

/// Last fire time, run id and skip reason for every trigger in a repo.
#[tauri::command]
pub fn get_trigger_state(repo_path: String) -> Result<HashMap<String, TriggerStateEntry>, String> {
    trigger_state::trigger_state_for_repo(&repo_path).map_err(|e| e.to_string())
}
