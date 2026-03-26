use crate::engine::models::{Environment, NotifyPayload, Pipeline, PipelineRun, RunStatus};
use crate::engine::{artifacts, cleanup, notify, persistence, pipeline, secrets};
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::path::Path;

const DEFAULT_PIPELINE_FILE: &str = "pipeline";

/// Load the selected pipeline file for execution.
pub fn load_selected_pipeline(repo_path: &Path, pipeline_file: Option<&str>) -> Result<Pipeline> {
    match pipeline_file {
        Some(name) => pipeline::load_pipeline_by_name(repo_path, name),
        None => pipeline::load_pipeline(repo_path),
    }
}

/// Resolve environment variables and secrets for a run.
pub fn resolve_execution_context(
    repo_path: &Path,
    environment_name: Option<&str>,
) -> Result<(Option<Environment>, HashMap<String, String>)> {
    let Some(env_name) = environment_name else {
        return Ok((None, HashMap::new()));
    };

    let envs_config = pipeline::load_environments(repo_path)?;
    let env = envs_config
        .environments
        .iter()
        .find(|e| e.name == env_name)
        .cloned();

    let mut vars: HashMap<String, String> = HashMap::new();

    if let Some(ref env) = env {
        vars.extend(env.variables.clone());
    }

    let secrets_config = pipeline::load_secrets_config(repo_path)?;
    if !secrets_config.secrets.is_empty() {
        let repo_path_str = repo_path.to_string_lossy().to_string();
        let secret_vars =
            secrets::resolve_secrets_for_env(&repo_path_str, env_name, &secrets_config)?;
        vars.extend(secret_vars);
    }

    Ok((env, vars))
}

/// Attach execution metadata to a run before it is persisted.
pub fn annotate_run(run: &mut PipelineRun, pipeline: &Pipeline, pipeline_file: Option<&str>) {
    run.pipeline_snapshot = Some(pipeline.clone());
    run.pipeline_file = Some(pipeline_file.unwrap_or(DEFAULT_PIPELINE_FILE).to_string());
}

/// Recover the exact pipeline definition recorded with a historical run.
pub fn pipeline_snapshot_for_run(run: &PipelineRun) -> Result<Pipeline> {
    run.pipeline_snapshot.clone().ok_or_else(|| {
        anyhow!(
            "Run {} does not include a recorded pipeline snapshot. Re-run the pipeline once on the current version before using retry or rollback.",
            run.id
        )
    })
}

/// Build the stage filter for a retry run starting at the requested stage.
pub fn stages_to_run_from_stage(pipeline: &Pipeline, retry_stage: &str) -> Result<Vec<String>> {
    let retry_idx = pipeline
        .stages
        .iter()
        .position(|s| s.name == retry_stage)
        .with_context(|| {
            format!(
                "Stage '{}' does not exist in pipeline '{}'",
                retry_stage, pipeline.name
            )
        })?;

    Ok(pipeline.stages[retry_idx..]
        .iter()
        .map(|s| s.name.clone())
        .collect())
}

/// Persist a completed run and update the project summary.
pub fn persist_completed_run(run: &PipelineRun) -> Result<()> {
    persistence::save_run(run)?;

    if let Ok(mut projects) = persistence::load_projects() {
        if let Some(proj) = projects.iter_mut().find(|p| p.path == run.repo_path) {
            proj.last_run_at = Some(run.started_at);
            proj.last_run_status = Some(run.status.clone());
            persistence::save_projects(&projects)?;
        }
    }

    Ok(())
}

/// Post-run housekeeping: send notifications and run cleanup.
/// Failures are logged but never propagate.
pub async fn post_run_housekeeping(repo_path: &str, run: &PipelineRun) {
    let path = Path::new(repo_path);

    match notify::resolve_notify_config(path) {
        Ok(config) => {
            let status_label = match run.status {
                RunStatus::Success => "succeeded",
                RunStatus::Failed => "failed",
                RunStatus::Cancelled => "cancelled",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::models::{Backend, Pipeline, PipelineRun, Stage};

    fn sample_pipeline() -> Pipeline {
        Pipeline {
            name: "deploy".to_string(),
            stages: vec![
                Stage {
                    name: "build".to_string(),
                    commands: vec!["echo build".to_string()],
                    backend: Backend::Local,
                    working_dir: None,
                    fail_fast: true,
                    health_check: None,
                },
                Stage {
                    name: "deploy".to_string(),
                    commands: vec!["echo deploy".to_string()],
                    backend: Backend::Local,
                    working_dir: None,
                    fail_fast: true,
                    health_check: None,
                },
            ],
        }
    }

    #[test]
    fn test_annotate_run_persists_snapshot_and_default_file() {
        let pipeline = sample_pipeline();
        let mut run = PipelineRun::new("deploy", "/tmp/repo", None);

        annotate_run(&mut run, &pipeline, None);

        assert_eq!(run.pipeline_file.as_deref(), Some(DEFAULT_PIPELINE_FILE));
        assert_eq!(
            run.pipeline_snapshot.as_ref().map(|p| p.stages.len()),
            Some(2)
        );
    }

    #[test]
    fn test_pipeline_snapshot_for_run_requires_snapshot() {
        let run = PipelineRun::new("deploy", "/tmp/repo", None);
        let err = pipeline_snapshot_for_run(&run).unwrap_err();

        assert!(err
            .to_string()
            .contains("does not include a recorded pipeline snapshot"));
    }

    #[test]
    fn test_stages_to_run_from_stage_validates_requested_stage() {
        let pipeline = sample_pipeline();
        let stages = stages_to_run_from_stage(&pipeline, "deploy").unwrap();

        assert_eq!(stages, vec!["deploy".to_string()]);
        assert!(stages_to_run_from_stage(&pipeline, "missing").is_err());
    }
}
