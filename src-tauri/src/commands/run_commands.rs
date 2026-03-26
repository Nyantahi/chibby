use crate::engine::executor;
use crate::engine::models::{DeploymentRecord, PipelineRun, RunKind};
use crate::engine::{persistence, run_support};
use crate::state::SharedPipelineState;
use std::future::Future;
use std::path::Path;
use tauri::{AppHandle, Emitter, State};

fn build_log_callback(app: AppHandle) -> executor::LogCallback {
    Box::new(move |stage: &str, log_type: &str, msg: &str| {
        let _ = app.emit(
            "pipeline:log",
            serde_json::json!({
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

async fn with_pipeline_tracking<T, F>(
    pipeline_state: SharedPipelineState,
    repo_path: &str,
    operation: F,
) -> Result<T, String>
where
    F: Future<Output = Result<T, String>>,
{
    {
        let mut state = pipeline_state.write().await;
        state.start(repo_path);
    }

    let result = operation.await;

    {
        let mut state = pipeline_state.write().await;
        state.cleanup(repo_path);
    }

    result
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
    let path = Path::new(&repo_path);
    let cancel_state = (*pipeline_state).clone();
    let run = with_pipeline_tracking(cancel_state.clone(), &repo_path, async move {
        let p = run_support::load_selected_pipeline(path, pipeline_file.as_deref())
            .map_err(|e| e.to_string())?;
        let (env_ref, env_vars) =
            run_support::resolve_execution_context(path, environment.as_deref())
                .map_err(|e| e.to_string())?;

        let mut run = executor::run_pipeline(
            &p,
            path,
            env_ref.as_ref(),
            env_vars,
            Some(build_log_callback(app)),
            stages.as_deref(),
            Some(cancel_state.clone()),
            Some(build_stage_callback()),
        )
        .await
        .map_err(|e| e.to_string())?;

        run_support::annotate_run(&mut run, &p, pipeline_file.as_deref());
        Ok(run)
    })
    .await?;

    // Persist the run.
    run_support::persist_completed_run(&run).map_err(|e| e.to_string())?;

    // Post-run housekeeping (notifications + cleanup).
    run_support::post_run_housekeeping(&repo_path, &run).await;

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
    let p = run_support::pipeline_snapshot_for_run(&original).map_err(|e| e.to_string())?;

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
    let stages_to_run =
        run_support::stages_to_run_from_stage(&p, &retry_stage).map_err(|e| e.to_string())?;

    // Calculate retry number.
    let parent_id = original.parent_run_id.as_deref().unwrap_or(&run_id);
    let existing_retries = persistence::retry_count_for_run(parent_id).unwrap_or(0);
    let retry_number = existing_retries + 1;

    let cancel_state = (*pipeline_state).clone();
    let mut run = with_pipeline_tracking(cancel_state.clone(), &repo_path, async move {
        let (env_ref, env_vars) =
            run_support::resolve_execution_context(path, original.environment.as_deref())
                .map_err(|e| e.to_string())?;

        let mut run = executor::run_pipeline(
            &p,
            path,
            env_ref.as_ref(),
            env_vars,
            Some(build_log_callback(app)),
            Some(&stages_to_run),
            Some(cancel_state.clone()),
            Some(build_stage_callback()),
        )
        .await
        .map_err(|e| e.to_string())?;

        run_support::annotate_run(&mut run, &p, original.pipeline_file.as_deref());
        Ok(run)
    })
    .await?;

    // Tag the run as a retry.
    run.run_kind = RunKind::Retry;
    run.parent_run_id = Some(parent_id.to_string());
    run.retry_number = Some(retry_number);
    run.retry_from_stage = Some(retry_stage);

    // Persist the run.
    run_support::persist_completed_run(&run).map_err(|e| e.to_string())?;

    // Post-run housekeeping (notifications + cleanup).
    run_support::post_run_housekeeping(&repo_path, &run).await;

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
    let p = run_support::pipeline_snapshot_for_run(&target).map_err(|e| e.to_string())?;

    let cancel_state = (*pipeline_state).clone();
    let mut run = with_pipeline_tracking(cancel_state.clone(), &repo_path, async move {
        let (env_ref, env_vars) =
            run_support::resolve_execution_context(path, target.environment.as_deref())
                .map_err(|e| e.to_string())?;

        let mut run = executor::run_pipeline(
            &p,
            path,
            env_ref.as_ref(),
            env_vars,
            Some(build_log_callback(app)),
            None,
            Some(cancel_state.clone()),
            Some(build_stage_callback()),
        )
        .await
        .map_err(|e| e.to_string())?;

        run_support::annotate_run(&mut run, &p, target.pipeline_file.as_deref());
        Ok(run)
    })
    .await?;

    // Tag the run as a rollback.
    run.run_kind = RunKind::Rollback;
    run.rollback_target_id = Some(target_run_id);

    // Persist the run.
    run_support::persist_completed_run(&run).map_err(|e| e.to_string())?;

    // Post-run housekeeping (notifications + cleanup).
    run_support::post_run_housekeeping(&repo_path, &run).await;

    Ok(run)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::create_pipeline_state;

    #[tokio::test]
    async fn test_with_pipeline_tracking_cleans_up_after_error() {
        let state = create_pipeline_state();
        let repo_path = "/tmp/chibby-run-error";

        let result: Result<(), String> =
            with_pipeline_tracking(state.clone(), repo_path, async { Err("boom".to_string()) })
                .await;

        assert_eq!(result.unwrap_err(), "boom");

        let guard = state.read().await;
        assert!(!guard.is_running(repo_path));
    }

    #[tokio::test]
    async fn test_with_pipeline_tracking_cleans_up_after_success() {
        let state = create_pipeline_state();
        let repo_path = "/tmp/chibby-run-success";

        let result =
            with_pipeline_tracking(state.clone(), repo_path, async { Ok::<_, String>(42) }).await;

        assert_eq!(result.unwrap(), 42);

        let guard = state.read().await;
        assert!(!guard.is_running(repo_path));
    }
}
