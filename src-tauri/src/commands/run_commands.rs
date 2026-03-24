use crate::engine::executor;
use crate::engine::models::{DeploymentRecord, NotifyPayload, PipelineRun, RunKind};
use crate::engine::{artifacts, cleanup, notify, persistence, pipeline, secrets};
use crate::state::SharedPipelineState;
use std::collections::HashMap;
use std::path::Path;
use tauri::{AppHandle, Emitter, State};

/// Post-run housekeeping: send notifications and run cleanup.
/// Failures are logged but never propagate — housekeeping must not block the caller.
async fn post_run_housekeeping(repo_path: &str, run: &PipelineRun) {
    let path = Path::new(repo_path);

    // --- Notifications ---
    match notify::resolve_notify_config(path) {
        Ok(config) => {
            let status_label = match run.status {
                crate::engine::models::RunStatus::Success => "succeeded",
                crate::engine::models::RunStatus::Failed => "failed",
                crate::engine::models::RunStatus::Cancelled => "cancelled",
                _ => "completed",
            };
            let payload = NotifyPayload {
                project: run.pipeline_name.clone(),
                version: None,
                environment: run.environment.clone(),
                status: run.status.clone(),
                duration_ms: run.duration_ms,
                message: format!("Pipeline '{}' {}", run.pipeline_name, status_label),
            };
            notify::send_notifications(&config, &payload).await;
        }
        Err(e) => log::warn!("Failed to load notify config for {repo_path}: {e}"),
    }

    // --- Cleanup (prune old artifacts & runs) ---
    let cleanup_config = match cleanup::resolve_cleanup_config(path) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("Failed to load cleanup config for {repo_path}: {e}");
            return;
        }
    };
    let artifact_config = match artifacts::load_artifact_config(path) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("Failed to load artifact config for {repo_path}: {e}");
            return;
        }
    };
    if let Err(e) = cleanup::run_cleanup(path, &cleanup_config, &artifact_config, false) {
        log::warn!("Post-run cleanup failed for {repo_path}: {e}");
    }
}

/// Run a pipeline for a given repo path.
///
/// When an environment is specified, resolves environment variables and
/// secrets from the keychain before execution.
#[tauri::command]
pub async fn run_pipeline(
    app: AppHandle,
    pipeline_state: State<'_, SharedPipelineState>,
    repo_path: String,
    environment: Option<String>,
    stages: Option<Vec<String>>,
    pipeline_file: Option<String>,
) -> Result<PipelineRun, String> {
    // Mark pipeline as started (not cancelled)
    {
        let mut state = pipeline_state.write().await;
        state.start(&repo_path);
    }

    let path = Path::new(&repo_path);
    let p = if let Some(ref name) = pipeline_file {
        pipeline::load_pipeline_by_name(path, name).map_err(|e| e.to_string())?
    } else {
        pipeline::load_pipeline(path).map_err(|e| e.to_string())?
    };

    // Resolve environment and secrets if an environment is specified.
    let (env_ref, env_vars) = if let Some(ref env_name) = environment {
        let envs_config = pipeline::load_environments(path).map_err(|e| e.to_string())?;
        let env = envs_config
            .environments
            .iter()
            .find(|e| e.name == *env_name)
            .cloned();

        let mut vars: HashMap<String, String> = HashMap::new();

        if let Some(ref env) = env {
            // Start with environment variables.
            vars.extend(env.variables.clone());
        }

        // Resolve secrets from keychain.
        let secrets_config =
            pipeline::load_secrets_config(path).map_err(|e| e.to_string())?;
        if !secrets_config.secrets.is_empty() {
            match secrets::resolve_secrets_for_env(&repo_path, env_name, &secrets_config) {
                Ok(secret_vars) => vars.extend(secret_vars),
                Err(e) => return Err(format!("Secret resolution failed: {}", e)),
            }
        }

        (env, vars)
    } else {
        (None, HashMap::new())
    };

    // Stream log events to the frontend via Tauri events.
    let on_log: executor::LogCallback = Box::new(move |stage: &str, log_type: &str, msg: &str| {
        let _ = app.emit(
            "pipeline:log",
            serde_json::json!({
                "stage": stage,
                "type": log_type,
                "message": msg,
            }),
        );
    });

    // Clone the state for passing to executor
    let cancel_state = (*pipeline_state).clone();

    let run = executor::run_pipeline(
        &p,
        path,
        env_ref.as_ref(),
        env_vars,
        Some(on_log),
        stages.as_deref(),
        Some(cancel_state.clone()),
    )
    .await
    .map_err(|e| e.to_string())?;

    // Clean up pipeline state
    {
        let mut state = cancel_state.write().await;
        state.cleanup(&repo_path);
    }

    // Persist the run.
    persistence::save_run(&run).map_err(|e| e.to_string())?;

    // Update project last run status.
    if let Ok(mut projects) = persistence::load_projects() {
        if let Some(proj) = projects.iter_mut().find(|p| p.path == repo_path) {
            proj.last_run_at = Some(run.started_at);
            proj.last_run_status = Some(run.status.clone());
            let _ = persistence::save_projects(&projects);
        }
    }

    // Post-run housekeeping (notifications + cleanup).
    post_run_housekeeping(&repo_path, &run).await;

    Ok(run)
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
    let path = Path::new(&repo_path);
    let p = pipeline::load_pipeline(path).map_err(|e| e.to_string())?;

    // Determine which stage to retry from.
    let retry_stage = from_stage.clone().unwrap_or_else(|| {
        // Find the first failed stage.
        original
            .stage_results
            .iter()
            .find(|s| s.status == crate::engine::models::StageStatus::Failed)
            .map(|s| s.stage_name.clone())
            .unwrap_or_else(|| {
                // If no failed stage, start from the beginning.
                p.stages.first().map(|s| s.name.clone()).unwrap_or_default()
            })
    });

    // Build stage filter: include the retry stage and everything after it.
    let retry_idx = p
        .stages
        .iter()
        .position(|s| s.name == retry_stage)
        .unwrap_or(0);
    let stages_to_run: Vec<String> = p.stages[retry_idx..].iter().map(|s| s.name.clone()).collect();

    // Calculate retry number.
    let parent_id = original.parent_run_id.as_deref().unwrap_or(&run_id);
    let existing_retries = persistence::retry_count_for_run(parent_id).unwrap_or(0);
    let retry_number = existing_retries + 1;

    // Mark pipeline as started.
    {
        let mut state = pipeline_state.write().await;
        state.start(&repo_path);
    }

    // Resolve environment and secrets (same as run_pipeline).
    let (env_ref, env_vars) = if let Some(ref env_name) = original.environment {
        let envs_config = pipeline::load_environments(path).map_err(|e| e.to_string())?;
        let env = envs_config
            .environments
            .iter()
            .find(|e| e.name == *env_name)
            .cloned();

        let mut vars: HashMap<String, String> = HashMap::new();
        if let Some(ref env) = env {
            vars.extend(env.variables.clone());
        }

        let secrets_config =
            pipeline::load_secrets_config(path).map_err(|e| e.to_string())?;
        if !secrets_config.secrets.is_empty() {
            match secrets::resolve_secrets_for_env(&repo_path, env_name, &secrets_config) {
                Ok(secret_vars) => vars.extend(secret_vars),
                Err(e) => return Err(format!("Secret resolution failed: {}", e)),
            }
        }

        (env, vars)
    } else {
        (None, HashMap::new())
    };

    // Stream log events to the frontend.
    let on_log: executor::LogCallback = Box::new(move |stage: &str, log_type: &str, msg: &str| {
        let _ = app.emit(
            "pipeline:log",
            serde_json::json!({
                "stage": stage,
                "type": log_type,
                "message": msg,
            }),
        );
    });

    let cancel_state = (*pipeline_state).clone();

    let mut run = executor::run_pipeline(
        &p,
        path,
        env_ref.as_ref(),
        env_vars,
        Some(on_log),
        Some(&stages_to_run),
        Some(cancel_state.clone()),
    )
    .await
    .map_err(|e| e.to_string())?;

    // Tag the run as a retry.
    run.run_kind = RunKind::Retry;
    run.parent_run_id = Some(parent_id.to_string());
    run.retry_number = Some(retry_number);
    run.retry_from_stage = Some(retry_stage);

    // Clean up pipeline state.
    {
        let mut state = cancel_state.write().await;
        state.cleanup(&repo_path);
    }

    // Persist the run.
    persistence::save_run(&run).map_err(|e| e.to_string())?;

    // Update project last run status.
    if let Ok(mut projects) = persistence::load_projects() {
        if let Some(proj) = projects.iter_mut().find(|p| p.path == repo_path) {
            proj.last_run_at = Some(run.started_at);
            proj.last_run_status = Some(run.status.clone());
            let _ = persistence::save_projects(&projects);
        }
    }

    // Post-run housekeeping (notifications + cleanup).
    post_run_housekeeping(&repo_path, &run).await;

    Ok(run)
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
    let path = Path::new(&repo_path);
    let p = pipeline::load_pipeline(path).map_err(|e| e.to_string())?;

    // Mark pipeline as started.
    {
        let mut state = pipeline_state.write().await;
        state.start(&repo_path);
    }

    // Resolve environment and secrets for the target's environment.
    let (env_ref, env_vars) = if let Some(ref env_name) = target.environment {
        let envs_config = pipeline::load_environments(path).map_err(|e| e.to_string())?;
        let env = envs_config
            .environments
            .iter()
            .find(|e| e.name == *env_name)
            .cloned();

        let mut vars: HashMap<String, String> = HashMap::new();
        if let Some(ref env) = env {
            vars.extend(env.variables.clone());
        }

        let secrets_config =
            pipeline::load_secrets_config(path).map_err(|e| e.to_string())?;
        if !secrets_config.secrets.is_empty() {
            match secrets::resolve_secrets_for_env(&repo_path, env_name, &secrets_config) {
                Ok(secret_vars) => vars.extend(secret_vars),
                Err(e) => return Err(format!("Secret resolution failed: {}", e)),
            }
        }

        (env, vars)
    } else {
        (None, HashMap::new())
    };

    // Stream log events to the frontend.
    let on_log: executor::LogCallback = Box::new(move |stage: &str, log_type: &str, msg: &str| {
        let _ = app.emit(
            "pipeline:log",
            serde_json::json!({
                "stage": stage,
                "type": log_type,
                "message": msg,
            }),
        );
    });

    let cancel_state = (*pipeline_state).clone();

    let mut run = executor::run_pipeline(
        &p,
        path,
        env_ref.as_ref(),
        env_vars,
        Some(on_log),
        None, // Run all stages for rollback
        Some(cancel_state.clone()),
    )
    .await
    .map_err(|e| e.to_string())?;

    // Tag the run as a rollback.
    run.run_kind = RunKind::Rollback;
    run.rollback_target_id = Some(target_run_id);

    // Clean up pipeline state.
    {
        let mut state = cancel_state.write().await;
        state.cleanup(&repo_path);
    }

    // Persist the run.
    persistence::save_run(&run).map_err(|e| e.to_string())?;

    // Update project last run status.
    if let Ok(mut projects) = persistence::load_projects() {
        if let Some(proj) = projects.iter_mut().find(|p| p.path == repo_path) {
            proj.last_run_at = Some(run.started_at);
            proj.last_run_status = Some(run.status.clone());
            let _ = persistence::save_projects(&projects);
        }
    }

    // Post-run housekeeping (notifications + cleanup).
    post_run_housekeeping(&repo_path, &run).await;

    Ok(run)
}

/// Get the last successful run for a project, optionally filtered by environment.
#[tauri::command]
pub fn get_last_successful_run(
    repo_path: String,
    environment: Option<String>,
) -> Result<Option<PipelineRun>, String> {
    persistence::last_successful_run(&repo_path, environment.as_deref())
        .map_err(|e| e.to_string())
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
