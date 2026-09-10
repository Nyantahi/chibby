//! Automatic rollback when a stage's post-deploy health check fails.
//!
//! The executor only *records* that a health check was the cause (see
//! `PipelineRun::health_failure_stage`); deciding what to do about it lives
//! here, one layer up, so the executor never has to reach back into
//! persistence or start a nested run.

use crate::engine::models::{
    NotifyPayload, Pipeline, PipelineRun, RollbackMode, RollbackOutcome, RollbackPolicy, RunKind,
    RunStatus, Stage,
};
use crate::engine::{executor, notify, persistence, run_support};
use crate::state::SharedPipelineState;
use anyhow::{anyhow, Result};
use chrono::{Duration, Utc};
use std::path::{Path, PathBuf};

/// Stage name used for rollback decisions in the run log.
const LOG_STAGE: &str = "auto-rollback";

/// What the auto-rollback policy decided.
///
/// `NotApplicable` and `Skipped` both mean no rollback ran, but they are very
/// different operationally: `Skipped` means a policy WAS configured and a guard
/// refused it, so the bad release is still live and a human needs to know.
#[derive(Debug)]
pub enum AutoRollback {
    /// No policy applies — nothing worth recording.
    NotApplicable,
    /// A policy applied but a guard refused it. Carries the reason.
    Skipped(String),
    /// A rollback run was executed.
    Ran(Box<PipelineRun>),
}

/// Outcome of resolving a plan, mirroring `AutoRollback` before execution.
enum PlanOutcome {
    NotApplicable,
    Skipped(String),
    Plan(Box<RollbackPlan>),
}

/// A rollback that passed every guard, ready to execute.
struct RollbackPlan {
    policy: RollbackPolicy,
    /// The pipeline the rollback run executes (a past snapshot, or an
    /// ephemeral single-stage pipeline built from `rollback_commands`).
    pipeline: Pipeline,
    environment: Option<String>,
    /// The historical run being restored, when the mode has one.
    target_id: Option<String>,
    pipeline_file: Option<String>,
}

/// Roll `failed` back if its health-check failure and the pipeline's policy
/// call for it. `Err` means the rollback configuration itself is broken.
pub async fn maybe_auto_rollback(
    failed: &PipelineRun,
    pipeline: &Pipeline,
    cancel_state: Option<SharedPipelineState>,
    on_log: Option<executor::LogCallback>,
) -> Result<AutoRollback> {
    let plan = match plan_rollback(failed, pipeline, &on_log)? {
        PlanOutcome::NotApplicable => return Ok(AutoRollback::NotApplicable),
        PlanOutcome::Skipped(reason) => return Ok(AutoRollback::Skipped(reason)),
        PlanOutcome::Plan(plan) => *plan,
    };

    let run = execute_rollback(failed, plan, cancel_state, on_log).await?;
    Ok(AutoRollback::Ran(Box::new(run)))
}

/// Resolve what (if anything) to roll back to. Every guard lives here so the
/// decision is synchronous and testable without executing a run.
fn plan_rollback(
    failed: &PipelineRun,
    pipeline: &Pipeline,
    on_log: &Option<executor::LogCallback>,
) -> Result<PlanOutcome> {
    if failed.status != RunStatus::Failed {
        return Ok(PlanOutcome::NotApplicable);
    }

    let Some(stage_name) = failed.health_failure_stage.clone() else {
        return Ok(PlanOutcome::NotApplicable);
    };
    let Some(stage) = pipeline.stages.iter().find(|s| s.name == stage_name) else {
        return skip(
            on_log,
            &format!("stage '{stage_name}' is not in the pipeline"),
        );
    };

    let policy = pipeline.rollback_policy_for(stage);
    if policy.mode == RollbackMode::Off {
        return Ok(PlanOutcome::NotApplicable);
    }

    // Guard 1: a rollback that itself health-fails must never trigger another.
    if failed.run_kind == RunKind::Rollback || failed.auto_rollback_of.is_some() {
        return skip(on_log, "the failed run is itself a rollback");
    }

    // "Last known good" is meaningless without a target environment.
    let Some(environment) = failed.environment.clone() else {
        return skip(on_log, "the run has no environment to roll back");
    };

    match policy.mode {
        RollbackMode::Off => Ok(PlanOutcome::NotApplicable),
        RollbackMode::LastGood => plan_last_good(failed, &stage_name, environment, policy, on_log),
        RollbackMode::Commands => {
            plan_commands(failed, pipeline, stage, environment, policy, on_log)
        }
    }
}

/// Replay the newest run that actually deployed this stage successfully.
fn plan_last_good(
    failed: &PipelineRun,
    stage_name: &str,
    environment: String,
    policy: RollbackPolicy,
    on_log: &Option<executor::LogCallback>,
) -> Result<PlanOutcome> {
    let target =
        persistence::last_good_deployment(&failed.repo_path, &environment, stage_name, &failed.id)?;

    let Some(target) = target else {
        return skip(
            on_log,
            &format!("no known-good deployment recorded for '{environment}'"),
        );
    };

    // Guard 2: redundant with `exclude_run_id`, but cheap and explicit.
    if target.id == failed.id {
        return skip(on_log, "the resolved target is the failed run itself");
    }

    let snapshot = run_support::pipeline_snapshot_for_run(&target)?;

    if let Some(reason) = throttle_reason(&failed.repo_path, &environment, &policy)? {
        return skip(on_log, &reason);
    }

    Ok(PlanOutcome::Plan(Box::new(RollbackPlan {
        policy,
        pipeline: snapshot,
        environment: target.environment.clone(),
        target_id: Some(target.id),
        pipeline_file: target.pipeline_file.clone(),
    })))
}

/// Run the failed stage's own undo commands (`kubectl rollout undo`, ...).
fn plan_commands(
    failed: &PipelineRun,
    pipeline: &Pipeline,
    stage: &Stage,
    environment: String,
    policy: RollbackPolicy,
    on_log: &Option<executor::LogCallback>,
) -> Result<PlanOutcome> {
    let ephemeral = commands_rollback_pipeline(pipeline, stage, &policy)?;

    if let Some(reason) = throttle_reason(&failed.repo_path, &environment, &policy)? {
        return skip(on_log, &reason);
    }

    Ok(PlanOutcome::Plan(Box::new(RollbackPlan {
        policy,
        pipeline: ephemeral,
        environment: Some(environment),
        target_id: None,
        pipeline_file: failed.pipeline_file.clone(),
    })))
}

/// Build the throwaway single-stage pipeline that undoes `stage`.
///
/// The rollback stage inherits the failed stage's backend, working directory
/// and env so the undo commands run exactly where the deploy did, plus its
/// health check when the policy asks for verification.
pub fn commands_rollback_pipeline(
    pipeline: &Pipeline,
    stage: &Stage,
    policy: &RollbackPolicy,
) -> Result<Pipeline> {
    let commands = stage.rollback_commands.clone().unwrap_or_default();
    if commands.is_empty() {
        return Err(anyhow!(
            "Stage '{}' sets on_health_failure mode = \"commands\" but defines no rollback_commands",
            stage.name
        ));
    }

    Ok(Pipeline {
        name: format!("{} (rollback)", pipeline.name),
        // Never chain: the rollback pipeline carries no policy of its own.
        on_health_failure: None,
        stages: vec![Stage {
            name: format!("rollback-{}", stage.name),
            commands,
            backend: stage.backend.clone(),
            working_dir: stage.working_dir.clone(),
            env: stage.env.clone(),
            health_check: policy
                .verify_health
                .then(|| stage.health_check.clone())
                .flatten(),
            ..Default::default()
        }],
    })
}

/// Guard 3: why this rollback is throttled, if it is.
///
/// Catches flapping — a deploy broken at source where every scheduled run
/// would otherwise roll back again. Counted per environment rather than per
/// target: a successful `last_good` rollback becomes the new last-known-good,
/// so the target rotates every cycle and a per-target count would never fire.
fn throttle_reason(
    repo_path: &str,
    environment: &str,
    policy: &RollbackPolicy,
) -> Result<Option<String>> {
    let since = Utc::now() - Duration::minutes(i64::from(policy.window_mins));
    let recent = persistence::load_runs_for_project(repo_path)?
        .into_iter()
        .filter(|run| {
            run.auto_rollback_of.is_some()
                && run.started_at >= since
                && run.environment.as_deref() == Some(environment)
        })
        .count() as u32;

    if recent < policy.max_attempts {
        return Ok(None);
    }

    Ok(Some(format!(
        "{recent} auto-rollback(s) already ran in the last {} minutes (max_attempts = {})",
        policy.window_mins, policy.max_attempts
    )))
}

/// Start the rollback run through the same path a manual rollback uses.
async fn execute_rollback(
    failed: &PipelineRun,
    plan: RollbackPlan,
    cancel_state: Option<SharedPipelineState>,
    on_log: Option<executor::LogCallback>,
) -> Result<PipelineRun> {
    let description = match plan.target_id.as_deref() {
        Some(id) => format!("replaying last known-good run {id}"),
        None => "running the stage's rollback_commands".to_string(),
    };
    log::warn!(
        "[rollback] auto-rollback for run {}: {description}",
        failed.id
    );
    if let Some(ref cb) = on_log {
        cb(LOG_STAGE, "warn", &format!("Auto-rollback: {description}"));
    }

    let policy = plan.policy;
    // `Box::pin` breaks the execute_run -> maybe_auto_rollback -> execute_run
    // future cycle, which is otherwise infinitely sized.
    let run = Box::pin(run_support::execute_run_holding_lock(
        run_support::ExecuteRunRequest {
            repo_path: PathBuf::from(&failed.repo_path),
            pipeline_file: plan.pipeline_file,
            environment: plan.environment,
            run_kind: RunKind::Rollback,
            rollback_target_id: plan.target_id,
            auto_rollback_of: Some(failed.id.clone()),
            pipeline_override: Some(plan.pipeline),
            ..Default::default()
        },
        on_log,
        cancel_state,
        None,
    ))
    .await?;

    if run.status != RunStatus::Success {
        notify_rollback_failure(failed, &run, &policy).await;
    }

    Ok(run)
}

/// The one case a human must see: the deploy is unhealthy *and* the automatic
/// rollback did not restore it. Never retried, never chained — escalated.
async fn notify_rollback_failure(
    failed: &PipelineRun,
    rollback: &PipelineRun,
    policy: &RollbackPolicy,
) {
    let environment = failed.environment.as_deref().unwrap_or("unknown");
    let message = format!(
        "MANUAL INTERVENTION REQUIRED: '{}' failed its health check on '{}' and the automatic rollback (run {}) also failed. The environment may be left unhealthy.",
        failed.pipeline_name, environment, rollback.id
    );
    log::error!("[rollback] {message}");

    if !policy.notify {
        return;
    }

    let config = match notify::resolve_notify_config(Path::new(&failed.repo_path)) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("[rollback] could not load notify config: {e}");
            return;
        }
    };

    let payload = NotifyPayload {
        project: failed.pipeline_name.clone(),
        version: None,
        environment: failed.environment.clone(),
        status: RunStatus::Failed,
        duration_ms: rollback.duration_ms,
        message,
        rollback: Some(RollbackOutcome::Failed),
        run_kind: Some(failed.run_kind),
        trigger_id: failed.trigger_id.clone(),
    };
    notify::send_notifications(&config, &payload).await;
}

/// The stage a rollback targets when no failed stage names it: the last stage
/// with a health check, else simply the last stage.
pub fn deploy_stage_name(pipeline: &Pipeline) -> Option<&str> {
    pipeline
        .stages
        .iter()
        .rev()
        .find(|s| s.health_check.is_some())
        .or_else(|| pipeline.stages.last())
        .map(|s| s.name.as_str())
}

/// Record why no rollback happened, then decline.
fn skip(on_log: &Option<executor::LogCallback>, reason: &str) -> Result<PlanOutcome> {
    log::info!("[rollback] skipped: {reason}");
    if let Some(cb) = on_log {
        cb(
            LOG_STAGE,
            "warn",
            &format!("Auto-rollback skipped: {reason}"),
        );
    }
    Ok(PlanOutcome::Skipped(reason.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::models::{Backend, HealthCheck, StageResult, StageStatus};
    use std::collections::HashMap;

    const REPO: &str = "/tmp/chibby-auto-rollback";

    fn deploy_stage() -> Stage {
        Stage {
            name: "deploy".to_string(),
            commands: vec!["./deploy.sh".to_string()],
            backend: Backend::Ssh,
            working_dir: Some("/srv/app".to_string()),
            rollback_commands: Some(vec!["kubectl rollout undo deploy/api".to_string()]),
            health_check: Some(HealthCheck {
                command: "curl -f http://localhost/health".to_string(),
                retries: 2,
                delay_secs: 1,
            }),
            env: Some(HashMap::from([("SLOT".to_string(), "blue".to_string())])),
            ..Default::default()
        }
    }

    fn pipeline_with(mode: RollbackMode) -> Pipeline {
        Pipeline {
            name: "api".to_string(),
            on_health_failure: Some(RollbackPolicy {
                mode,
                ..Default::default()
            }),
            stages: vec![deploy_stage()],
        }
    }

    /// A failed run whose `deploy` stage failed its health check.
    fn health_failed_run(id: &str) -> PipelineRun {
        let mut run = PipelineRun::new_with_id(id, "api", REPO, Some("prod".to_string()));
        run.status = RunStatus::Failed;
        run.health_failure_stage = Some("deploy".to_string());
        run.stage_results = vec![StageResult {
            stage_name: "deploy".to_string(),
            status: StageStatus::Failed,
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
            started_at: None,
            finished_at: None,
            duration_ms: None,
            health_check_passed: Some(false),
            attempts: Some(1),
            skip_reason: None,
        }];
        run
    }

    /// Guard 1 — the guard that actually matters: a rollback run that itself
    /// health-fails must not spawn another rollback.
    #[tokio::test]
    async fn test_rollback_run_never_triggers_another_rollback() {
        let pipeline = pipeline_with(RollbackMode::LastGood);

        let mut by_kind = health_failed_run("rb-kind");
        by_kind.run_kind = RunKind::Rollback;
        assert!(matches!(
            maybe_auto_rollback(&by_kind, &pipeline, None, None)
                .await
                .unwrap(),
            AutoRollback::Skipped(_)
        ));

        let mut by_link = health_failed_run("rb-link");
        by_link.auto_rollback_of = Some("origin".to_string());
        assert!(matches!(
            maybe_auto_rollback(&by_link, &pipeline, None, None)
                .await
                .unwrap(),
            AutoRollback::Skipped(_)
        ));
    }

    /// "Last known good" needs a target environment to mean anything.
    #[tokio::test]
    async fn test_run_without_environment_is_skipped() {
        let pipeline = pipeline_with(RollbackMode::LastGood);
        let mut run = health_failed_run("no-env");
        run.environment = None;

        assert!(matches!(
            maybe_auto_rollback(&run, &pipeline, None, None)
                .await
                .unwrap(),
            AutoRollback::Skipped(_)
        ));
    }

    /// Absent config must behave exactly as before: nothing happens.
    #[tokio::test]
    async fn test_mode_off_does_nothing() {
        let mut pipeline = pipeline_with(RollbackMode::Off);
        pipeline.on_health_failure = None;

        assert!(matches!(
            maybe_auto_rollback(&health_failed_run("off"), &pipeline, None, None)
                .await
                .unwrap(),
            AutoRollback::NotApplicable
        ));
    }

    /// A run that failed on a *command* (no health-check cause recorded) is
    /// left alone.
    #[tokio::test]
    async fn test_command_failure_is_not_rolled_back() {
        let pipeline = pipeline_with(RollbackMode::LastGood);
        let mut run = health_failed_run("cmd-fail");
        run.health_failure_stage = None;

        assert!(matches!(
            maybe_auto_rollback(&run, &pipeline, None, None)
                .await
                .unwrap(),
            AutoRollback::NotApplicable
        ));
    }

    #[tokio::test]
    async fn test_commands_mode_without_rollback_commands_errors() {
        let mut pipeline = pipeline_with(RollbackMode::Commands);
        pipeline.stages[0].rollback_commands = None;

        let err = maybe_auto_rollback(&health_failed_run("no-cmds"), &pipeline, None, None)
            .await
            .unwrap_err();

        assert!(
            err.to_string().contains("no rollback_commands"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_commands_mode_builds_ephemeral_pipeline_inheriting_the_stage() {
        let pipeline = pipeline_with(RollbackMode::Commands);
        let policy = RollbackPolicy {
            mode: RollbackMode::Commands,
            ..Default::default()
        };

        let built = commands_rollback_pipeline(&pipeline, &pipeline.stages[0], &policy).unwrap();
        let stage = &built.stages[0];

        assert_eq!(built.name, "api (rollback)");
        assert!(built.on_health_failure.is_none(), "must never chain");
        assert_eq!(built.stages.len(), 1);
        assert_eq!(stage.name, "rollback-deploy");
        assert_eq!(stage.commands, vec!["kubectl rollout undo deploy/api"]);
        assert_eq!(stage.backend, Backend::Ssh);
        assert_eq!(stage.working_dir.as_deref(), Some("/srv/app"));
        assert_eq!(stage.env, pipeline.stages[0].env);
        assert!(stage.on_health_failure.is_none());
        // verify_health defaults to true, so the health check is inherited.
        assert_eq!(
            stage.health_check.as_ref().map(|h| h.command.as_str()),
            Some("curl -f http://localhost/health")
        );
    }

    #[test]
    fn test_commands_mode_drops_health_check_when_verification_is_off() {
        let pipeline = pipeline_with(RollbackMode::Commands);
        let policy = RollbackPolicy {
            mode: RollbackMode::Commands,
            verify_health: false,
            ..Default::default()
        };

        let built = commands_rollback_pipeline(&pipeline, &pipeline.stages[0], &policy).unwrap();

        assert!(built.stages[0].health_check.is_none());
    }

    /// Guard 3: a flapping deploy must not roll back over and over.
    /// Exercises `plan_rollback` directly — every guard lives there, and it
    /// reaches a decision without starting a run.
    #[test]
    fn test_repeated_rollbacks_in_one_environment_are_throttled() {
        let (_dir, _lock) = persistence::scoped_test_data_dir();

        // A qualifying rollback target.
        let mut target = PipelineRun::new_with_id("target", "api", REPO, Some("prod".to_string()));
        target.status = RunStatus::Success;
        target.started_at = Utc::now() - Duration::minutes(120);
        target.pipeline_snapshot = Some(pipeline_with(RollbackMode::Off));
        target.stage_results = vec![StageResult {
            stage_name: "deploy".to_string(),
            status: StageStatus::Success,
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
            started_at: None,
            finished_at: None,
            duration_ms: None,
            health_check_passed: Some(true),
            attempts: Some(1),
            skip_reason: None,
        }];
        persistence::save_run(&target).unwrap();

        // One auto-rollback against it already ran, five minutes ago.
        let mut previous =
            PipelineRun::new_with_id("previous-rb", "api", REPO, Some("prod".to_string()));
        previous.status = RunStatus::Success;
        previous.started_at = Utc::now() - Duration::minutes(5);
        previous.run_kind = RunKind::Rollback;
        previous.auto_rollback_of = Some("earlier-failure".to_string());
        previous.rollback_target_id = Some("target".to_string());
        persistence::save_run(&previous).unwrap();

        // max_attempts = 1 inside a 60 minute window: refuse, with a reason —
        // the bad release stays live, so this must not look like "no policy".
        let pipeline = pipeline_with(RollbackMode::LastGood);
        let throttled = plan_rollback(&health_failed_run("now"), &pipeline, &None).unwrap();
        let PlanOutcome::Skipped(reason) = throttled else {
            panic!("throttle must report a skip reason");
        };
        assert!(
            reason.contains("auto-rollback"),
            "unexpected reason: {reason}"
        );

        // A one-minute window puts the earlier rollback out of scope, so the
        // throttle no longer applies and a target is found.
        let mut narrow = pipeline_with(RollbackMode::LastGood);
        narrow.on_health_failure = Some(RollbackPolicy {
            mode: RollbackMode::LastGood,
            window_mins: 1,
            ..Default::default()
        });
        let PlanOutcome::Plan(plan) =
            plan_rollback(&health_failed_run("now"), &narrow, &None).unwrap()
        else {
            panic!("throttle must not apply outside the window");
        };
        assert_eq!(plan.target_id.as_deref(), Some("target"));
    }

    #[test]
    fn test_no_known_good_deployment_is_skipped() {
        let (_dir, _lock) = persistence::scoped_test_data_dir();
        let pipeline = pipeline_with(RollbackMode::LastGood);

        assert!(matches!(
            plan_rollback(&health_failed_run("lonely"), &pipeline, &None).unwrap(),
            PlanOutcome::Skipped(_)
        ));
    }

    #[test]
    fn test_deploy_stage_name_prefers_the_last_health_checked_stage() {
        let mut pipeline = pipeline_with(RollbackMode::Off);
        pipeline.stages.push(Stage {
            name: "smoke".to_string(),
            ..Default::default()
        });

        assert_eq!(deploy_stage_name(&pipeline), Some("deploy"));

        pipeline.stages[0].health_check = None;
        assert_eq!(deploy_stage_name(&pipeline), Some("smoke"));
    }
}
