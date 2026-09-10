use crate::engine::executor;
use crate::engine::models::{DeploymentRecord, PipelineRun, RunKind};
use crate::engine::run_support::{execute_run, ExecuteRunRequest};
use crate::engine::{persistence, pipeline, preflight, run_support};
use crate::state::SharedPipelineState;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

fn build_log_callback(app: AppHandle, run_id: String, repo_path: String) -> executor::LogCallback {
    Box::new(move |stage: &str, log_type: &str, msg: &str| {
        let _ = app.emit(
            "pipeline:log",
            serde_json::json!({
                "run_id": run_id,
                "repo_path": repo_path,
                "stage": stage,
                "type": log_type,
                "message": msg,
            }),
        );
    })
}

/// Build a callback that persists the run to disk after each stage completes.
/// This ensures partial results survive an app crash mid-pipeline.
fn build_stage_callback() -> executor::StageCallback {
    Box::new(move |run: &PipelineRun| {
        if let Err(e) = persistence::save_run(run) {
            log::warn!("Failed to persist intermediate run state: {e}");
        }
    })
}

/// Validate the run's target environment, mirroring the CLI's default
/// behaviour. Nothing to validate when the run has no environment.
async fn validate_before_run(
    repo_path: &str,
    environment: Option<&str>,
    pipeline_file: Option<&str>,
) -> Result<(), String> {
    let Some(env_name) = environment else {
        return Ok(());
    };

    let path = Path::new(repo_path);
    let pipe =
        run_support::load_selected_pipeline(path, pipeline_file).map_err(|e| e.to_string())?;
    let envs = pipeline::load_environments_layered(path).map_err(|e| e.to_string())?;
    let secs = pipeline::load_secrets_config(path).map_err(|e| e.to_string())?;

    let result = preflight::validate_preflight(&pipe, repo_path, env_name, &envs, &secs)
        .await
        .map_err(|e| e.to_string())?;

    if result.passed {
        return Ok(());
    }

    let details = result
        .errors
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("; ");
    Err(format!(
        "Preflight validation failed for environment '{env_name}': {details}"
    ))
}

/// Run a pipeline for a given repo path.
///
/// When an environment is specified, resolves environment variables and
/// secrets from the keychain before execution. Preflight runs first unless
/// `skip_preflight` is set, matching the CLI.
#[tauri::command]
pub async fn run_pipeline(
    app: AppHandle,
    pipeline_state: State<'_, SharedPipelineState>,
    repo_path: String,
    environment: Option<String>,
    stages: Option<Vec<String>>,
    pipeline_file: Option<String>,
    skip_preflight: bool,
) -> Result<PipelineRun, String> {
    if !skip_preflight {
        validate_before_run(&repo_path, environment.as_deref(), pipeline_file.as_deref()).await?;
    }

    let run_id = Uuid::new_v4().to_string();
    let on_log = build_log_callback(app, run_id.clone(), repo_path.clone());

    execute_run(
        ExecuteRunRequest {
            repo_path: PathBuf::from(&repo_path),
            pipeline_file,
            environment,
            stages,
            run_id: Some(run_id),
            ..Default::default()
        },
        Some(on_log),
        Some((*pipeline_state).clone()),
        Some(build_stage_callback()),
    )
    .await
    .map_err(|e| e.to_string())
}

/// Get run history for a project.
#[tauri::command]
pub fn get_run_history(repo_path: String) -> Result<Vec<PipelineRun>, String> {
    persistence::load_runs_for_project(&repo_path).map_err(|e| e.to_string())
}

/// Get all runs across all projects.
#[tauri::command]
pub fn get_all_runs() -> Result<Vec<PipelineRun>, String> {
    persistence::load_runs().map_err(|e| e.to_string())
}

/// Get a single run by ID.
#[tauri::command]
pub fn get_run(id: String) -> Result<Option<PipelineRun>, String> {
    persistence::load_run(&id).map_err(|e| e.to_string())
}

/// Cancel a running pipeline.
#[tauri::command]
pub async fn cancel_pipeline(
    pipeline_state: State<'_, SharedPipelineState>,
    repo_path: String,
) -> Result<(), String> {
    let mut state = pipeline_state.write().await;
    state.cancel(&repo_path);
    Ok(())
}

/// Check if a pipeline is currently running for a given repo path.
#[tauri::command]
pub async fn is_pipeline_running(
    pipeline_state: State<'_, SharedPipelineState>,
    repo_path: String,
) -> Result<bool, String> {
    let state = pipeline_state.read().await;
    Ok(state.is_running(&repo_path))
}

/// Retry a failed run, optionally starting from a specific stage.
///
/// If `from_stage` is provided, stages before it are skipped and their results
/// are copied from the original run. If not provided, retries from the first
/// failed stage.
#[tauri::command]
pub async fn retry_run(
    app: AppHandle,
    pipeline_state: State<'_, SharedPipelineState>,
    run_id: String,
    from_stage: Option<String>,
) -> Result<PipelineRun, String> {
    // Load the original run.
    let original = persistence::load_run(&run_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Run {} not found", run_id))?;

    let repo_path = original.repo_path.clone();
    let p = run_support::pipeline_snapshot_for_run(&original).map_err(|e| e.to_string())?;

    // Determine which stage to retry from.
    let retry_stage = from_stage.clone().unwrap_or_else(|| {
        // Find the first failed stage.
        original
            .stage_results
            .iter()
            .find(|s| s.status.is_failure())
            .map(|s| s.stage_name.clone())
            .unwrap_or_else(|| {
                // If no failed stage, start from the beginning.
                p.stages.first().map(|s| s.name.clone()).unwrap_or_default()
            })
    });

    // Build stage filter: include the retry stage and everything after it.
    let stages_to_run =
        run_support::stages_to_run_from_stage(&p, &retry_stage).map_err(|e| e.to_string())?;

    // Calculate retry number.
    let parent_id = original.parent_run_id.as_deref().unwrap_or(&run_id);
    let existing_retries = persistence::retry_count_for_run(parent_id).unwrap_or(0);

    let new_run_id = Uuid::new_v4().to_string();
    let on_log = build_log_callback(app, new_run_id.clone(), repo_path.clone());

    execute_run(
        ExecuteRunRequest {
            repo_path: PathBuf::from(&repo_path),
            pipeline_file: original.pipeline_file.clone(),
            environment: original.environment.clone(),
            stages: Some(stages_to_run),
            run_kind: RunKind::Retry,
            parent_run_id: Some(parent_id.to_string()),
            retry_number: Some(existing_retries + 1),
            retry_from_stage: Some(retry_stage),
            pipeline_override: Some(p),
            run_id: Some(new_run_id),
            ..Default::default()
        },
        Some(on_log),
        Some((*pipeline_state).clone()),
        Some(build_stage_callback()),
    )
    .await
    .map_err(|e| e.to_string())
}

/// Roll back to a previously successful run by re-executing its pipeline
/// configuration in the same environment.
///
/// This creates a new run tagged as a rollback, executing the full pipeline
/// using the same environment as the target run.
#[tauri::command]
pub async fn rollback_to_run(
    app: AppHandle,
    pipeline_state: State<'_, SharedPipelineState>,
    target_run_id: String,
) -> Result<PipelineRun, String> {
    // Load the target run (the one we want to roll back to).
    let target = persistence::load_run(&target_run_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Target run {} not found", target_run_id))?;

    if target.status != crate::engine::models::RunStatus::Success {
        return Err("Can only roll back to a successful run".to_string());
    }

    let repo_path = target.repo_path.clone();
    let p = run_support::pipeline_snapshot_for_run(&target).map_err(|e| e.to_string())?;

    let new_run_id = Uuid::new_v4().to_string();
    let on_log = build_log_callback(app, new_run_id.clone(), repo_path.clone());

    execute_run(
        ExecuteRunRequest {
            repo_path: PathBuf::from(&repo_path),
            pipeline_file: target.pipeline_file.clone(),
            environment: target.environment.clone(),
            run_kind: RunKind::Rollback,
            rollback_target_id: Some(target_run_id),
            pipeline_override: Some(p),
            run_id: Some(new_run_id),
            ..Default::default()
        },
        Some(on_log),
        Some((*pipeline_state).clone()),
        Some(build_stage_callback()),
    )
    .await
    .map_err(|e| e.to_string())
}

/// Get the last successful run for a project, optionally filtered by environment.
#[tauri::command]
pub fn get_last_successful_run(
    repo_path: String,
    environment: Option<String>,
) -> Result<Option<PipelineRun>, String> {
    persistence::last_successful_run(&repo_path, environment.as_deref()).map_err(|e| e.to_string())
}

/// Get deployment history for a project and environment.
#[tauri::command]
pub fn get_deployment_history(
    repo_path: String,
    environment: String,
) -> Result<Vec<DeploymentRecord>, String> {
    persistence::deployment_history(&repo_path, &environment).map_err(|e| e.to_string())
}

/// Delete a run by ID.
#[tauri::command]
pub fn delete_run(id: String) -> Result<(), String> {
    persistence::delete_run(&id).map_err(|e| e.to_string())
}

/// Clear all run history for a project.
#[tauri::command]
pub fn clear_run_history(repo_path: String) -> Result<u32, String> {
    persistence::clear_runs_for_project(&repo_path).map_err(|e| e.to_string())
}
