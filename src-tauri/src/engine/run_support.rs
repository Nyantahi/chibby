use crate::engine::models::{
    Environment, NotifyPayload, Pipeline, PipelineRun, RollbackOutcome, RunKind, RunStatus,
};
use crate::engine::redact::Redactor;
use crate::engine::{
    app_settings, artifacts, cleanup, executor, locks, notify, persistence, pipeline, rollback,
    secrets,
};
use crate::state::SharedPipelineState;
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

const DEFAULT_PIPELINE_FILE: &str = "pipeline";

/// Load the selected pipeline file for execution.
pub fn load_selected_pipeline(repo_path: &Path, pipeline_file: Option<&str>) -> Result<Pipeline> {
    match pipeline_file {
        Some(name) => pipeline::load_pipeline_by_name(repo_path, name),
        None => pipeline::load_pipeline(repo_path),
    }
}

/// Resolved environment + variables for one run.
#[derive(Debug, Default, Clone)]
pub struct ExecutionContext {
    pub environment: Option<Environment>,
    /// Environment variables and resolved secret values, merged.
    pub vars: HashMap<String, String>,
    /// Keys in `vars` whose values came from the keychain.
    pub secret_names: Vec<String>,
}

/// Resolve environment variables and secrets for a run.
pub fn resolve_execution_context(
    repo_path: &Path,
    environment_name: Option<&str>,
) -> Result<ExecutionContext> {
    let Some(env_name) = environment_name else {
        return Ok(ExecutionContext::default());
    };

    let envs_config = pipeline::load_environments_layered(repo_path)?;
    let environment = envs_config
        .environments
        .iter()
        .find(|e| e.name == env_name)
        .cloned();

    let mut vars: HashMap<String, String> = HashMap::new();
    let mut secret_names: Vec<String> = Vec::new();

    if let Some(ref env) = environment {
        vars.extend(env.variables.clone());
    }

    let secrets_config = pipeline::load_secrets_config(repo_path)?;
    if !secrets_config.secrets.is_empty() {
        let repo_path_str = repo_path.to_string_lossy().to_string();
        let secret_vars =
            secrets::resolve_secrets_for_env(&repo_path_str, env_name, &secrets_config)?;
        secret_names.extend(secret_vars.keys().cloned());
        vars.extend(secret_vars);
    }

    Ok(ExecutionContext {
        environment,
        vars,
        secret_names,
    })
}

/// Build the log redactor for a run: the resolved secret values, masked
/// wherever they appear in stage output. Returns a no-op redactor when the user
/// has turned masking off — pattern-based redaction still applies downstream.
pub fn build_redactor(context: &ExecutionContext) -> Redactor {
    let enabled = app_settings::load_app_settings()
        .map(|s| s.mask_secrets_in_logs)
        .unwrap_or(true);

    if !enabled {
        return Redactor::default();
    }

    Redactor::new(
        context
            .secret_names
            .iter()
            .filter_map(|name| context.vars.get(name).cloned()),
    )
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

    // Best-effort summary update: the run is already saved, so a projects.json
    // hiccup must never fail a completed run. The atomic helper serializes the
    // read-modify-write against concurrent completions.
    let _ = persistence::mutate_projects(|projects| {
        if let Some(proj) = projects.iter_mut().find(|p| p.path == run.repo_path) {
            proj.last_run_at = Some(run.started_at);
            proj.last_run_status = Some(run.status.clone());
        }
    });

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
                rollback: run.rollback_outcome,
                run_kind: Some(run.run_kind),
                trigger_id: run.trigger_id.clone(),
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

/// Announce a triggered run that failed *before* it produced a run record —
/// a lock held by another process, an unparseable pipeline, a secret that
/// would not resolve.
///
/// [`post_run_housekeeping`] only runs once a run exists, so without this a
/// nightly that never started is silent, which is precisely what the
/// unattended notification policy exists to prevent.
pub async fn notify_trigger_failure(
    repo_path: &str,
    trigger_id: &str,
    run_kind: RunKind,
    error: &str,
) {
    let path = Path::new(repo_path);
    let config = match notify::resolve_notify_config(path) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("Failed to load notify config for {repo_path}: {e}");
            return;
        }
    };

    let project = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| repo_path.to_string());

    let payload = NotifyPayload {
        project,
        version: None,
        environment: None,
        status: RunStatus::Failed,
        duration_ms: None,
        message: format!("Trigger '{trigger_id}' could not start: {error}"),
        rollback: None,
        run_kind: Some(run_kind),
        trigger_id: Some(trigger_id.to_string()),
    };
    notify::send_notifications(&config, &payload).await;
}

/// Track a pipeline as running for `repo_path` for the duration of `operation`.
/// Cleanup always runs, so a failed run never leaves a stale "running" entry.
pub async fn with_pipeline_tracking<T, E, F>(
    pipeline_state: SharedPipelineState,
    repo_path: &str,
    operation: F,
) -> Result<T, E>
where
    F: Future<Output = Result<T, E>>,
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

/// Everything needed to execute one run: what to run, and how to tag it.
#[derive(Default)]
pub struct ExecuteRunRequest {
    pub repo_path: PathBuf,
    pub pipeline_file: Option<String>,
    pub environment: Option<String>,
    /// Stage filter; None = all stages.
    pub stages: Option<Vec<String>>,
    pub run_kind: RunKind,
    pub parent_run_id: Option<String>,
    pub retry_number: Option<u32>,
    pub retry_from_stage: Option<String>,
    pub rollback_target_id: Option<String>,
    /// On an automatic rollback: the failed run that caused it.
    pub auto_rollback_of: Option<String>,
    /// Pre-built pipeline; when set, skips loading from disk (used by rollback,
    /// which replays a past run's `pipeline_snapshot`).
    pub pipeline_override: Option<Pipeline>,
    /// Run id to execute under; None generates a fresh one. Callers that build
    /// a log callback carrying the run id set this so logs and run agree.
    pub run_id: Option<String>,
    /// Which trigger started this run (`scheduled:<id>`, `watch:<id>`,
    /// `hook:pre-push`). None when a human started it directly.
    pub trigger_id: Option<String>,
}

/// Execute one run end to end: load pipeline, resolve context, run stages,
/// annotate, persist, and perform post-run housekeeping.
///
/// Takes the cross-process run lock first, so a headless trigger and the open
/// desktop app can never run the same repo at the same time.
pub async fn execute_run(
    req: ExecuteRunRequest,
    on_log: Option<executor::LogCallback>,
    cancel_state: Option<SharedPipelineState>,
    on_stage_complete: Option<executor::StageCallback>,
) -> Result<PipelineRun> {
    let repo_path_str = req.repo_path.to_string_lossy().to_string();
    let _lock = locks::acquire_run_lock_with_id(&repo_path_str, req.run_id.as_deref())?
        .ok_or_else(|| busy_error(&repo_path_str))?;

    execute_run_holding_lock(req, on_log, cancel_state, on_stage_complete).await
}

/// Why a run was refused, naming the process that already owns the repo.
fn busy_error(repo_path: &str) -> anyhow::Error {
    let holder = locks::current_holder(repo_path)
        .ok()
        .flatten()
        .map(|h| format!(" (pid {}, started {})", h.pid, h.started_at.to_rfc3339()))
        .unwrap_or_default();
    anyhow!("A run is already in progress for {repo_path}{holder}")
}

/// As [`execute_run`], for callers that already hold the run lock.
///
/// Auto-rollback re-enters this path from inside a run, so it must not try to
/// take a lock its own parent is holding.
pub(crate) async fn execute_run_holding_lock(
    req: ExecuteRunRequest,
    on_log: Option<executor::LogCallback>,
    cancel_state: Option<SharedPipelineState>,
    on_stage_complete: Option<executor::StageCallback>,
) -> Result<PipelineRun> {
    let ExecuteRunRequest {
        repo_path,
        pipeline_file,
        environment,
        stages,
        run_kind,
        parent_run_id,
        retry_number,
        retry_from_stage,
        rollback_target_id,
        auto_rollback_of,
        pipeline_override,
        run_id,
        trigger_id,
    } = req;

    let repo_path_str = repo_path.to_string_lossy().to_string();
    let run_id = run_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    // Shared so an auto-rollback run can stream into the same log sink; a
    // `LogCallback` is a plain `Box<dyn Fn>` and cannot be cloned.
    let shared_log = on_log.map(Arc::new);
    let on_log = delegating_log_callback(&shared_log);

    let execute = async {
        let pipeline = match pipeline_override {
            Some(p) => p,
            None => load_selected_pipeline(&repo_path, pipeline_file.as_deref())?,
        };
        let context = resolve_execution_context(&repo_path, environment.as_deref())?;
        let redactor = build_redactor(&context);

        let mut run = executor::run_pipeline(
            &pipeline,
            &repo_path,
            context.environment.as_ref(),
            context.vars,
            on_log,
            stages.as_deref(),
            cancel_state.clone(),
            on_stage_complete,
            &run_id,
            redactor,
        )
        .await?;

        annotate_run(&mut run, &pipeline, pipeline_file.as_deref());
        Ok::<(PipelineRun, Pipeline), anyhow::Error>((run, pipeline))
    };

    // Only track (and therefore allow cancelling) when a state was supplied.
    let (mut run, pipeline) = match cancel_state.clone() {
        Some(state) => with_pipeline_tracking(state, &repo_path_str, execute).await?,
        None => execute.await?,
    };

    run.run_kind = run_kind;
    run.parent_run_id = parent_run_id;
    run.retry_number = retry_number;
    run.retry_from_stage = retry_from_stage;
    run.rollback_target_id = rollback_target_id;
    run.auto_rollback_of = auto_rollback_of;
    run.trigger_id = trigger_id;

    persist_completed_run(&run)?;

    // Every entry point (GUI and CLI, run/retry/rollback) funnels through here,
    // so this is the one place auto-rollback has to be wired in. It runs before
    // housekeeping so the single notification can tell the whole story.
    apply_auto_rollback(
        &mut run,
        &pipeline,
        cancel_state,
        delegating_log_callback(&shared_log),
    )
    .await;

    post_run_housekeeping(&repo_path_str, &run).await;

    Ok(run)
}

/// A fresh `LogCallback` delegating to the shared one, or None.
fn delegating_log_callback(
    shared: &Option<Arc<executor::LogCallback>>,
) -> Option<executor::LogCallback> {
    let shared = shared.clone()?;
    Some(Box::new(move |stage: &str, kind: &str, msg: &str| {
        shared(stage, kind, msg)
    }))
}

/// Run the pipeline's auto-rollback policy against a completed run and record
/// the link on both sides. Never fails the original run.
async fn apply_auto_rollback(
    run: &mut PipelineRun,
    pipeline: &Pipeline,
    cancel_state: Option<SharedPipelineState>,
    on_log: Option<executor::LogCallback>,
) {
    let outcome = match rollback::maybe_auto_rollback(run, pipeline, cancel_state, on_log).await {
        // No policy applied — nothing worth recording on the run.
        Ok(rollback::AutoRollback::NotApplicable) => return,
        // A policy applied but a guard refused it: the bad release is still
        // live, so record why rather than leaving the run looking untouched.
        Ok(rollback::AutoRollback::Skipped(reason)) => {
            run.rollback_skip_reason = Some(reason);
            RollbackOutcome::Skipped
        }
        Ok(rollback::AutoRollback::Ran(rollback_run)) => {
            run.rollback_run_id = Some(rollback_run.id.clone());
            if rollback_run.status == RunStatus::Success {
                RollbackOutcome::Succeeded
            } else {
                RollbackOutcome::Failed
            }
        }
        Err(e) => {
            log::error!("[rollback] auto-rollback for run {} failed: {e}", run.id);
            run.rollback_skip_reason = Some(e.to_string());
            RollbackOutcome::Failed
        }
    };

    run.rollback_outcome = Some(outcome);

    // Re-save so the failed run carries the link too, not just the rollback run.
    if let Err(e) = persistence::save_run(run) {
        log::warn!(
            "[rollback] could not record rollback link on run {}: {e}",
            run.id
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::models::{Environment, EnvironmentsConfig, SecretRef, SecretsConfig};
    use crate::engine::models::{Pipeline, PipelineRun, Stage};
    use crate::state::create_pipeline_state;
    use tempfile::TempDir;

    fn sample_pipeline() -> Pipeline {
        Pipeline {
            name: "deploy".to_string(),
            on_health_failure: None,
            stages: vec![
                Stage {
                    name: "build".to_string(),
                    commands: vec!["echo build".to_string()],
                    ..Default::default()
                },
                Stage {
                    name: "deploy".to_string(),
                    commands: vec!["echo deploy".to_string()],
                    ..Default::default()
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

    #[test]
    fn test_execution_context_separates_secrets_from_plain_variables() {
        let temp = TempDir::new().unwrap();
        let repo = temp.path();
        let repo_str = repo.to_string_lossy().to_string();

        pipeline::save_environments(
            repo,
            &EnvironmentsConfig {
                environments: vec![Environment {
                    name: "prod".to_string(),
                    ssh_host: None,
                    ssh_port: None,
                    variables: HashMap::from([("NODE_ENV".to_string(), "production".to_string())]),
                }],
            },
        )
        .unwrap();
        pipeline::save_secrets_config(
            repo,
            &SecretsConfig {
                secrets: vec![SecretRef {
                    name: "API_KEY".to_string(),
                    environments: vec!["prod".to_string()],
                }],
            },
        )
        .unwrap();
        secrets::set_secret(&repo_str, "prod", "API_KEY", "s3cr3t").unwrap();

        let context = resolve_execution_context(repo, Some("prod")).unwrap();
        let _ = secrets::delete_secret(&repo_str, "prod", "API_KEY");

        assert_eq!(context.environment.map(|e| e.name).as_deref(), Some("prod"));
        assert_eq!(
            context.vars.get("NODE_ENV").map(String::as_str),
            Some("production")
        );
        assert_eq!(
            context.vars.get("API_KEY").map(String::as_str),
            Some("s3cr3t")
        );
        assert_eq!(context.secret_names, vec!["API_KEY".to_string()]);
    }

    #[test]
    fn test_build_redactor_masks_only_secret_values() {
        let context = ExecutionContext {
            environment: None,
            vars: HashMap::from([
                ("API_KEY".to_string(), "s3cr3t-value-1234".to_string()),
                ("NODE_ENV".to_string(), "production".to_string()),
            ]),
            secret_names: vec!["API_KEY".to_string()],
        };

        let redactor = build_redactor(&context);
        let line = redactor.redact("key=s3cr3t-value-1234 env=production");

        assert!(
            !line.contains("s3cr3t-value-1234"),
            "secret leaked: {line:?}"
        );
        assert!(line.contains("production"), "plain var masked: {line:?}");
    }

    #[test]
    fn test_build_redactor_is_noop_without_secrets() {
        assert!(build_redactor(&ExecutionContext::default()).is_noop());
    }

    #[test]
    fn test_execution_context_is_empty_without_environment() {
        let temp = TempDir::new().unwrap();
        let context = resolve_execution_context(temp.path(), None).unwrap();

        assert!(context.environment.is_none());
        assert!(context.vars.is_empty());
        assert!(context.secret_names.is_empty());
    }
}
