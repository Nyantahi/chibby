use crate::engine::deploy::{
    build_ssh_command, check_docker_compose_services, run_health_check, StageLog,
};
use crate::engine::git;
use crate::engine::locks;
use crate::engine::models::{
    Backend, Backoff, Environment, Pipeline, PipelineRun, RunStatus, Stage, StageResult,
    StageStatus,
};
use crate::engine::redact::Redactor;
use crate::engine::stage_when::{self, WhenContext};
use crate::state::SharedPipelineState;
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::Instant;

/// How often the cancel monitor polls while a command runs or a retry backs off.
const CANCEL_POLL: Duration = Duration::from_millis(200);

/// Callback signature for streaming log lines during execution.
pub type LogCallback = Box<dyn Fn(&str, &str, &str) + Send + Sync>;

/// Callback invoked after each stage completes, receiving the in-progress run.
/// Used for incremental persistence so partial results survive a crash.
pub type StageCallback = Box<dyn Fn(&PipelineRun) + Send + Sync>;

/// What the pipeline loop should do after a stage finishes.
enum StageControl {
    /// Continue to the next stage.
    Continue,
    /// User cancelled mid-stage; mark the run cancelled and stop.
    Cancelled,
    /// Stage failed and is `fail_fast`; mark the run failed and stop.
    FailFast,
}

/// Everything a stage needs beyond its own definition. Built per stage so
/// `env_vars` can carry the stage's own env overlay.
#[derive(Clone, Copy)]
struct StageContext<'a> {
    repo_path: &'a Path,
    environment: Option<&'a Environment>,
    /// Run variables merged with the stage's `env` overlay.
    env_vars: &'a HashMap<String, String>,
    on_log: &'a Option<LogCallback>,
    cancel_state: &'a Option<SharedPipelineState>,
    redactor: &'a Redactor,
}

/// Result of a single attempt at a stage, before retry accounting.
struct AttemptOutcome {
    status: StageStatus,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    health_check_passed: Option<bool>,
    /// Commands succeeded and only the health check failed. `HealthCheck` has
    /// its own retry budget, so the stage retry policy must not re-run a
    /// completed deploy on top of it.
    health_check_only_failure: bool,
    /// User cancelled mid-attempt.
    cancelled: bool,
}

/// Execute an entire pipeline, stage by stage.
///
/// Supports both local and SSH execution backends. Environment variables
/// and resolved secrets are injected into every command. This is a thin loop
/// over `run_stage`; per-stage execution, streaming, and cancellation live there.
#[allow(clippy::too_many_arguments)]
pub async fn run_pipeline(
    pipeline: &Pipeline,
    repo_path: &Path,
    environment: Option<&Environment>,
    env_vars: HashMap<String, String>,
    on_log: Option<LogCallback>,
    stage_filter: Option<&[String]>,
    cancel_state: Option<SharedPipelineState>,
    on_stage_complete: Option<StageCallback>,
    run_id: &str,
    redactor: Redactor,
) -> Result<PipelineRun> {
    let env_name = environment.map(|e| e.name.clone());
    let mut run = PipelineRun::new_with_id(
        run_id,
        &pipeline.name,
        &repo_path.to_string_lossy(),
        env_name,
    );
    run.status = RunStatus::Running;

    // Git provenance is recorded here rather than in the callers so every entry
    // point (GUI, CLI, retry, rollback) gets it. The branch also feeds `when`.
    let (branch, commit) = git_provenance(repo_path).await;
    run.branch = branch.clone();
    run.commit = commit;

    let environment_name = run.environment.clone();
    let when_ctx = WhenContext {
        branch: branch.as_deref(),
        environment: environment_name.as_deref(),
    };

    let mut had_failures = false;

    for stage in &pipeline.stages {
        // Skip stages not in the filter (if a filter is provided).
        if let Some(filter) = &stage_filter {
            if !filter.iter().any(|f| f == &stage.name) {
                run.stage_results.push(skipped_stage(&stage.name));
                continue;
            }
        }

        match stage_when::skip_reason(&stage.when, &when_ctx) {
            Ok(None) => {}
            Ok(Some(reason)) => {
                if let Some(ref cb) = on_log {
                    cb(
                        &stage.name,
                        "info",
                        &format!("--- Skipping stage {}: {} ---", stage.name, reason),
                    );
                }
                run.stage_results
                    .push(skipped_with_reason(&stage.name, reason));
                continue;
            }
            Err(e) => {
                // A malformed condition means the pipeline definition can't be
                // trusted — fail loudly instead of silently skipping.
                let message = format!("Invalid `when` condition on stage '{}': {}", stage.name, e);
                log::error!("[executor] {}", message);
                if let Some(ref cb) = on_log {
                    cb(&stage.name, "error", &message);
                }
                run.stage_results
                    .push(condition_error_stage(&stage.name, message));
                run.status = RunStatus::Failed;
                mark_remaining_skipped(&mut run, pipeline);
                finalize_run(&mut run);
                return Ok(run);
            }
        }

        let effective_env = effective_stage_env(&env_vars, stage);
        let ctx = StageContext {
            repo_path,
            environment,
            env_vars: &effective_env,
            on_log: &on_log,
            cancel_state: &cancel_state,
            redactor: &redactor,
        };

        let (result, control) = run_stage(stage, &mut run, &ctx, &on_stage_complete).await;

        if let StageControl::Cancelled = control {
            run.status = RunStatus::Cancelled;
            run.stage_results.push(result);
            mark_remaining_skipped(&mut run, pipeline);
            finalize_run(&mut run);
            return Ok(run);
        }

        let failed = result.status.is_failure();
        if failed_health_check(&result) && run.health_failure_stage.is_none() {
            run.health_failure_stage = Some(stage.name.clone());
        }
        run.stage_results.push(result);

        // Persist partial run state so results survive a crash.
        if let Some(ref cb) = on_stage_complete {
            cb(&run);
        }

        if failed {
            had_failures = true;
        }

        if let StageControl::FailFast = control {
            run.status = RunStatus::Failed;
            mark_remaining_skipped(&mut run, pipeline);
            break;
        }
    }

    if run.status == RunStatus::Running {
        run.status = if had_failures {
            RunStatus::Failed
        } else {
            RunStatus::Success
        };
    }

    finalize_run(&mut run);
    Ok(run)
}

/// Whether a stage failed *because* its health check failed, rather than
/// because a command failed. `health_check_passed` is only recorded once the
/// commands themselves succeeded, so `Some(false)` pins the cause exactly.
fn failed_health_check(result: &StageResult) -> bool {
    result.status.is_failure() && result.health_check_passed == Some(false)
}

/// Read the repo's branch and short commit off the executor thread. `git` is a
/// blocking `std::process::Command`, so it must not run on the async runtime.
async fn git_provenance(repo_path: &Path) -> (Option<String>, Option<String>) {
    let path = repo_path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        // `current_branch` yields "HEAD" when detached — that is not a branch.
        let branch = git::current_branch(&path).ok().filter(|b| b != "HEAD");
        (branch, git::head_short_commit(&path))
    })
    .await
    .unwrap_or((None, None))
}

/// Run variables with the stage's own `env` overlaid on top.
fn effective_stage_env(
    env_vars: &HashMap<String, String>,
    stage: &Stage,
) -> HashMap<String, String> {
    let mut effective = env_vars.clone();
    if let Some(ref overlay) = stage.env {
        effective.extend(overlay.clone());
    }
    effective
}

/// Run a single stage, retrying per its policy, and report the result plus what
/// the loop should do next. Pushes a `Running` placeholder and persists it
/// before execution so crash recovery sees the active stage.
async fn run_stage(
    stage: &Stage,
    run: &mut PipelineRun,
    ctx: &StageContext<'_>,
    on_stage_complete: &Option<StageCallback>,
) -> (StageResult, StageControl) {
    let stage_start = Utc::now();

    if let Some(ref cb) = ctx.on_log {
        cb(
            &stage.name,
            "info",
            &format!("--- Starting stage: {} ---", stage.name),
        );
    }

    // Push a Running placeholder and persist BEFORE execution starts. If the app
    // crashes mid-stage, recovery will see which stage was active. Then remove
    // it — the real result is returned to the caller to push.
    run.stage_results
        .push(running_placeholder(&stage.name, stage_start));
    if let Some(ref cb) = on_stage_complete {
        cb(run);
    }
    run.stage_results.pop();

    let max_attempts = stage.retry.as_ref().map(|r| r.attempts.max(1)).unwrap_or(1);
    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut attempt: u32 = 1;

    let outcome = loop {
        if attempt > 1 {
            let separator = format!("--- attempt {}/{} ---", attempt, max_attempts);
            stdout.push_str(&separator);
            stdout.push('\n');
            stderr.push_str(&separator);
            stderr.push('\n');
            if let Some(ref cb) = ctx.on_log {
                cb(&stage.name, "info", &separator);
            }
        }

        let outcome = run_stage_attempt(stage, ctx).await;
        stdout.push_str(&outcome.stdout);
        stderr.push_str(&outcome.stderr);

        if outcome.cancelled {
            if let Some(ref cb) = ctx.on_log {
                cb(&stage.name, "warn", "Pipeline cancelled by user");
            }
            return (
                cancelled_stage(&stage.name, stage_start, stdout, stderr),
                StageControl::Cancelled,
            );
        }

        let can_retry = outcome.status.is_failure()
            && !outcome.health_check_only_failure
            && attempt < max_attempts;
        if !can_retry {
            break outcome;
        }

        let delay = retry_delay(stage, attempt);
        if let Some(ref cb) = ctx.on_log {
            cb(
                &stage.name,
                "warn",
                &format!(
                    "Attempt {}/{} failed; retrying in {}s",
                    attempt,
                    max_attempts,
                    delay.as_secs()
                ),
            );
        }
        if !sleep_cancellable(delay, ctx).await {
            if let Some(ref cb) = ctx.on_log {
                cb(&stage.name, "warn", "Pipeline cancelled by user");
            }
            return (
                cancelled_stage(&stage.name, stage_start, stdout, stderr),
                StageControl::Cancelled,
            );
        }

        attempt += 1;
    };

    let stage_end = Utc::now();
    let result = StageResult {
        stage_name: stage.name.clone(),
        status: outcome.status.clone(),
        exit_code: outcome.exit_code,
        stdout,
        stderr,
        started_at: Some(stage_start),
        finished_at: Some(stage_end),
        duration_ms: Some((stage_end - stage_start).num_milliseconds() as u64),
        health_check_passed: outcome.health_check_passed,
        attempts: Some(attempt),
        skip_reason: None,
    };

    let control = if outcome.status.is_failure() && stage.fail_fast {
        StageControl::FailFast
    } else {
        StageControl::Continue
    };

    (result, control)
}

/// Delay before the attempt following `attempt` (1-based).
fn retry_delay(stage: &Stage, attempt: u32) -> Duration {
    let Some(ref retry) = stage.retry else {
        return Duration::ZERO;
    };
    let multiplier = match retry.backoff {
        Backoff::Fixed => 1u64,
        Backoff::Exponential => 2u64.saturating_pow(attempt.saturating_sub(1)),
    };
    Duration::from_secs(retry.delay_secs.saturating_mul(multiplier))
}

/// Sleep for `duration`, polling for cancellation. Returns false if the run was
/// cancelled — a long backoff must never make a stage uninterruptible.
async fn sleep_cancellable(duration: Duration, ctx: &StageContext<'_>) -> bool {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        let remaining = deadline - Instant::now();
        tokio::time::sleep(remaining.min(CANCEL_POLL)).await;
        if is_cancelled(ctx.cancel_state, ctx.repo_path).await {
            return false;
        }
    }
    true
}

/// Run every command in the stage once, then its health check. The stage's
/// `timeout_secs` is a single budget for the whole attempt, reset per attempt.
async fn run_stage_attempt(stage: &Stage, ctx: &StageContext<'_>) -> AttemptOutcome {
    let deadline = stage
        .timeout_secs
        .map(|secs| Instant::now() + Duration::from_secs(secs));

    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code: Option<i32> = None;
    let mut status = StageStatus::Running;

    for cmd_str in &stage.commands {
        // Check for cancellation before each command.
        if is_cancelled(ctx.cancel_state, ctx.repo_path).await {
            return cancelled_outcome(stdout, stderr);
        }

        if let Some(ref cb) = ctx.on_log {
            cb(&stage.name, "cmd", &format!("$ {}", cmd_str));
        }

        log::info!(
            "[executor] stage='{}' spawning command: {}",
            stage.name,
            cmd_str.chars().take(120).collect::<String>()
        );

        let mut child = match spawn_command(cmd_str, stage, ctx) {
            Ok(c) => c,
            Err(message) => {
                log::error!("[executor] stage='{}' {}", stage.name, message);
                if let Some(ref cb) = ctx.on_log {
                    cb(&stage.name, "error", &message);
                }
                stderr.push_str(&message);
                stderr.push('\n');
                status = StageStatus::Failed;
                break;
            }
        };

        // Register the child PID for cancellation handling.
        let child_pid = child.id();
        log::info!(
            "[executor] stage='{}' spawned pid={:?}",
            stage.name,
            child_pid
        );
        if let (Some(state), Some(pid)) = (ctx.cancel_state, child_pid) {
            let mut guard = state.write().await;
            guard.set_running_pid(&ctx.repo_path.to_string_lossy(), pid);
        }

        // Stream stdout and stderr concurrently to avoid pipe deadlock. Reading
        // them sequentially can hang if the child fills the stderr pipe buffer
        // while we're still draining stdout (or vice-versa). Also monitor for
        // cancellation and the stage deadline, killing the child if either fires.
        let interrupt;
        {
            let stdout_pipe = child.stdout.take();
            let stderr_pipe = child.stderr.take();

            let stage_name_out = stage.name.clone();
            let stage_name_err = stage.name.clone();
            let on_log_ref = ctx.on_log;
            let redactor = ctx.redactor;

            // Collected through shared buffers rather than the futures' return
            // values: a timeout or a cancel drops these tasks mid-read, and
            // anything they had gathered would go with them — leaving a
            // timed-out stage with the timeout marker and none of the output
            // that explains it.
            let out_buf = Buffer::default();
            let err_buf = Buffer::default();

            // Redaction happens here, at ingest, so the secret never reaches
            // either the streamed callback or the text persisted to disk.
            let stdout_task = {
                let out_buf = out_buf.clone();
                async move {
                    if let Some(pipe) = stdout_pipe {
                        let reader = BufReader::new(pipe);
                        let mut lines = reader.lines();
                        while let Some(line) = lines.next_line().await.unwrap_or(None) {
                            let line = redactor.redact_log(&line);
                            push_line(&out_buf, &line);
                            if let Some(ref cb) = on_log_ref {
                                cb(&stage_name_out, "stdout", &line);
                            }
                        }
                    }
                }
            };

            let stderr_task = {
                let err_buf = err_buf.clone();
                async move {
                    if let Some(pipe) = stderr_pipe {
                        let reader = BufReader::new(pipe);
                        let mut lines = reader.lines();
                        while let Some(line) = lines.next_line().await.unwrap_or(None) {
                            let line = redactor.redact_log(&line);
                            push_line(&err_buf, &line);
                            if let Some(ref cb) = on_log_ref {
                                cb(&stage_name_err, "stderr", &line);
                            }
                        }
                    }
                }
            };

            // Cancellation monitor - polls every 200ms and kills the child.
            let cancel_state_ref = ctx.cancel_state.clone();
            let repo_path_str = ctx.repo_path.to_string_lossy().to_string();
            let cancel_task = async {
                loop {
                    tokio::time::sleep(CANCEL_POLL).await;
                    if let Some(ref state) = cancel_state_ref {
                        let cancelled = {
                            let guard = state.read().await;
                            guard.is_cancelled(&repo_path_str)
                        };
                        if cancelled {
                            return;
                        }
                    }
                    if locks::cancel_requested(&repo_path_str) {
                        return;
                    }
                }
            };

            // Never resolves when the stage has no timeout configured.
            let timeout_task = async {
                match deadline {
                    Some(at) => tokio::time::sleep_until(at).await,
                    None => std::future::pending::<()>().await,
                }
            };

            // Race the I/O tasks against the cancellation monitor and deadline.
            tokio::select! {
                biased;

                _ = async { tokio::join!(stdout_task, stderr_task) } => {
                    interrupt = None;
                }
                _ = cancel_task => {
                    kill_child(&mut child, stage, ctx).await;
                    interrupt = Some(StageStatus::Skipped);
                }
                _ = timeout_task => {
                    kill_child(&mut child, stage, ctx).await;
                    interrupt = Some(StageStatus::TimedOut);
                }
            }

            // Whatever was read before the interrupt is still the stage's output.
            stdout.push_str(&take_buffer(&out_buf));
            stderr.push_str(&take_buffer(&err_buf));
        }

        // Clear the running PID.
        if let Some(state) = ctx.cancel_state {
            let mut guard = state.write().await;
            guard.clear_running_pid(&ctx.repo_path.to_string_lossy());
        }

        match interrupt {
            Some(StageStatus::TimedOut) => {
                let marker = format!(
                    "[chibby] stage timed out after {}s",
                    stage.timeout_secs.unwrap_or(0)
                );
                log::warn!("[executor] stage='{}' {}", stage.name, marker);
                if let Some(ref cb) = ctx.on_log {
                    cb(&stage.name, "error", &marker);
                }
                stderr.push_str(&marker);
                stderr.push('\n');
                status = StageStatus::TimedOut;
                break;
            }
            Some(_) => return cancelled_outcome(stdout, stderr),
            None => {}
        }

        log::info!(
            "[executor] stage='{}' I/O complete, waiting for exit",
            stage.name
        );
        let output = match child.wait().await {
            Ok(s) => s,
            Err(e) => {
                log::error!("[executor] stage='{}' wait() failed: {}", stage.name, e);
                if let Some(ref cb) = ctx.on_log {
                    cb(&stage.name, "error", &format!("Process wait failed: {}", e));
                }
                stderr.push_str(&format!("Process wait failed: {}\n", e));
                status = StageStatus::Failed;
                break;
            }
        };
        exit_code = output.code();
        log::info!(
            "[executor] stage='{}' exited code={:?} success={}",
            stage.name,
            exit_code,
            output.success()
        );

        if !output.success() {
            status = StageStatus::Failed;
            if let Some(ref cb) = ctx.on_log {
                cb(
                    &stage.name,
                    "error",
                    &format!("Command failed with exit code: {:?}", exit_code),
                );
            }
            break;
        }
    }

    if !status.is_failure() {
        status = StageStatus::Success;
    }

    let commands_succeeded = status == StageStatus::Success;
    let health_check_passed = stage_health_result(stage, &mut status, ctx).await;
    let health_check_only_failure = commands_succeeded && status.is_failure();

    AttemptOutcome {
        status,
        exit_code,
        stdout,
        stderr,
        health_check_passed,
        health_check_only_failure,
        cancelled: false,
    }
}

/// Spawn one command on the stage's backend, mapping build errors to a message.
fn spawn_command(
    cmd_str: &str,
    stage: &Stage,
    ctx: &StageContext<'_>,
) -> Result<tokio::process::Child, String> {
    match stage.backend {
        Backend::Local => {
            build_local_command(cmd_str, ctx.repo_path, &stage.working_dir, ctx.env_vars)
                .map_err(|e| format!("Failed to spawn command: {}", e))
        }
        Backend::Ssh => {
            build_ssh_command(cmd_str, ctx.environment, &stage.working_dir, ctx.env_vars)
                .map_err(|e| format!("SSH command error: {}", e))
        }
    }
}

/// Kill a child that overran its deadline or was cancelled.
async fn kill_child(child: &mut tokio::process::Child, stage: &Stage, ctx: &StageContext<'_>) {
    if let Err(e) = child.kill().await {
        // Process may have already exited.
        if let Some(ref cb) = ctx.on_log {
            cb(
                &stage.name,
                "warn",
                &format!("Failed to kill process: {}", e),
            );
        }
    }
}

/// Output collected by a reader task, readable even if that task is dropped.
type Buffer = Arc<Mutex<String>>;

fn push_line(buffer: &Buffer, line: &str) {
    let mut buf = buffer.lock().unwrap_or_else(|e| e.into_inner());
    buf.push_str(line);
    buf.push('\n');
}

fn take_buffer(buffer: &Buffer) -> String {
    std::mem::take(&mut *buffer.lock().unwrap_or_else(|e| e.into_inner()))
}

/// An attempt cut short by user cancellation.
fn cancelled_outcome(stdout: String, stderr: String) -> AttemptOutcome {
    AttemptOutcome {
        status: StageStatus::Skipped,
        exit_code: None,
        stdout,
        stderr,
        health_check_passed: None,
        health_check_only_failure: false,
        cancelled: true,
    }
}

/// Resolve a stage's health status: run its configured health check, or
/// auto-check docker compose services when a compose-up ran over SSH. Flips
/// `stage_status` to `Failed` on a failed check. Returns `None` when neither
/// applies.
async fn stage_health_result(
    stage: &Stage,
    stage_status: &mut StageStatus,
    ctx: &StageContext<'_>,
) -> Option<bool> {
    if *stage_status != StageStatus::Success {
        return None;
    }

    let log = StageLog::new(ctx.on_log, ctx.redactor, &stage.name);

    if let Some(ref hc) = stage.health_check {
        let passed = run_health_check(
            hc,
            &stage.backend,
            ctx.environment,
            ctx.repo_path,
            &stage.working_dir,
            ctx.env_vars,
            &log,
        )
        .await;

        if !passed {
            *stage_status = StageStatus::Failed;
            log.emit("error", "Health check failed after all retries");
        }
        return Some(passed);
    }

    // Auto-check docker compose services if a compose up command was run.
    if stage.backend == Backend::Ssh
        && stage
            .commands
            .iter()
            .any(|c| c.contains("docker compose up"))
    {
        let docker_ok =
            check_docker_compose_services(ctx.environment, &stage.working_dir, ctx.env_vars, &log)
                .await;
        if !docker_ok {
            *stage_status = StageStatus::Failed;
        }
        return Some(docker_ok);
    }

    None
}

/// Whether the run was cancelled for `repo_path`.
///
/// Checks the in-process flag and the lock directory's cancel file, so
/// `chibby cancel` can stop a run started by another process (the desktop app,
/// or a scheduled trigger).
async fn is_cancelled(cancel_state: &Option<SharedPipelineState>, repo_path: &Path) -> bool {
    let repo_path_str = repo_path.to_string_lossy().to_string();
    if let Some(state) = cancel_state {
        let guard = state.read().await;
        if guard.is_cancelled(&repo_path_str) {
            return true;
        }
    }
    locks::cancel_requested(&repo_path_str)
}

/// A `Running` placeholder result persisted before a stage executes.
fn running_placeholder(name: &str, started_at: DateTime<Utc>) -> StageResult {
    StageResult {
        stage_name: name.to_string(),
        status: StageStatus::Running,
        exit_code: None,
        stdout: String::new(),
        stderr: String::new(),
        started_at: Some(started_at),
        finished_at: None,
        duration_ms: None,
        health_check_passed: None,
        attempts: None,
        skip_reason: None,
    }
}

/// A `Skipped` result for a stage that never ran (filtered or after a stop).
fn skipped_stage(name: &str) -> StageResult {
    StageResult {
        stage_name: name.to_string(),
        status: StageStatus::Skipped,
        exit_code: None,
        stdout: String::new(),
        stderr: String::new(),
        started_at: None,
        finished_at: None,
        duration_ms: None,
        health_check_passed: None,
        attempts: None,
        skip_reason: None,
    }
}

/// A `Skipped` result for a stage excluded by its `when` condition.
fn skipped_with_reason(name: &str, reason: String) -> StageResult {
    StageResult {
        skip_reason: Some(reason),
        ..skipped_stage(name)
    }
}

/// A `Failed` result for a stage whose `when` condition could not be evaluated.
fn condition_error_stage(name: &str, message: String) -> StageResult {
    let now = Utc::now();
    StageResult {
        stage_name: name.to_string(),
        status: StageStatus::Failed,
        exit_code: None,
        stdout: String::new(),
        stderr: message,
        started_at: Some(now),
        finished_at: Some(now),
        duration_ms: Some(0),
        health_check_passed: None,
        attempts: None,
        skip_reason: None,
    }
}

/// A `Skipped` result for the stage interrupted by cancellation, keeping the
/// partial output captured before the interrupt.
fn cancelled_stage(
    name: &str,
    started_at: DateTime<Utc>,
    stdout: String,
    stderr: String,
) -> StageResult {
    StageResult {
        stage_name: name.to_string(),
        status: StageStatus::Skipped,
        exit_code: None,
        stdout,
        stderr,
        started_at: Some(started_at),
        finished_at: Some(Utc::now()),
        duration_ms: None,
        health_check_passed: None,
        attempts: None,
        skip_reason: None,
    }
}

/// Append a `Skipped` result for every stage not yet processed.
fn mark_remaining_skipped(run: &mut PipelineRun, pipeline: &Pipeline) {
    let done = run.stage_results.len();
    for remaining in pipeline.stages.iter().skip(done) {
        run.stage_results.push(skipped_stage(&remaining.name));
    }
}

/// Stamp the run's finish time and total duration.
fn finalize_run(run: &mut PipelineRun) {
    let end = Utc::now();
    run.finished_at = Some(end);
    run.duration_ms = Some((end - run.started_at).num_milliseconds() as u64);
}

/// Build a local shell command with environment variable injection.
/// Shared with the agent command chokepoint (`agent::command_exec`) and the
/// deploy health checks (`engine::deploy`).
pub(crate) fn build_local_command(
    cmd_str: &str,
    repo_path: &Path,
    working_dir: &Option<String>,
    env_vars: &HashMap<String, String>,
) -> Result<tokio::process::Child> {
    let work_dir = match working_dir {
        Some(wd) => repo_path.join(wd),
        None => repo_path.to_path_buf(),
    };

    let shell = get_shell();
    let shell_flag = get_shell_flag();

    // Use an interactive login shell so the user's PATH (from .zshrc/.bashrc)
    // is available. Tauri apps launched from Dock/Finder get a minimal env
    // without tools like npm, node, cargo, etc.
    let child = Command::new(&shell)
        .arg("-l")
        .arg("-i")
        .arg(&shell_flag)
        .arg(cmd_str)
        .current_dir(&work_dir)
        .envs(env_vars)
        // Tell well-behaved CLI tools to disable color/ANSI output
        .env("NO_COLOR", "1")
        .env("FORCE_COLOR", "0")
        .env("TERM", "dumb")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Kill the child if its handle is dropped (cancel / agent timeout) so we
        // never orphan a running process.
        .kill_on_drop(true)
        .spawn()?;

    Ok(child)
}

/// Get the default shell for the current platform.
///
/// On macOS, prefer /bin/zsh (the system default) over $SHELL because Tauri
/// apps launched from the Dock may not have $SHELL set, falling back to
/// /bin/sh which doesn't source the user's profile.
fn get_shell() -> String {
    #[cfg(target_os = "windows")]
    {
        "cmd".to_string()
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    }
}

/// Get the shell flag to execute a command string.
fn get_shell_flag() -> String {
    #[cfg(target_os = "windows")]
    {
        "/C".to_string()
    }
    #[cfg(not(target_os = "windows"))]
    {
        "-c".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::models::{HealthCheck, Pipeline, Stage, StageRetry, StageWhen};
    use tempfile::TempDir;

    /// Run `pipeline` in `repo` with no environment, callbacks or secrets.
    async fn run_bare(pipeline: &Pipeline, repo: &Path) -> PipelineRun {
        run_pipeline(
            pipeline,
            repo,
            None,
            HashMap::new(),
            None,
            None,
            None,
            None,
            "test-run",
            Redactor::default(),
        )
        .await
        .unwrap()
    }

    /// The orchestration layer keys auto-rollback off this field, so a health
    /// check failure must name the stage that caused it.
    #[tokio::test]
    async fn test_failed_health_check_records_the_stage() {
        let repo = TempDir::new().unwrap();
        let pipeline = Pipeline {
            name: "test".to_string(),
            on_health_failure: None,
            stages: vec![Stage {
                name: "deploy".to_string(),
                commands: vec!["echo deployed".to_string()],
                health_check: Some(HealthCheck {
                    command: "exit 1".to_string(),
                    retries: 1,
                    delay_secs: 0,
                }),
                ..Default::default()
            }],
        };

        let run = run_bare(&pipeline, repo.path()).await;

        assert_eq!(run.status, RunStatus::Failed);
        assert_eq!(run.health_failure_stage.as_deref(), Some("deploy"));
        assert_eq!(run.stage_results[0].health_check_passed, Some(false));
    }

    /// A stage whose *command* failed never reached its health check, so it
    /// must not be mistaken for one.
    #[tokio::test]
    async fn test_command_failure_does_not_record_a_health_failure() {
        let repo = TempDir::new().unwrap();
        let pipeline = Pipeline {
            name: "test".to_string(),
            on_health_failure: None,
            stages: vec![Stage {
                name: "deploy".to_string(),
                commands: vec!["exit 1".to_string()],
                health_check: Some(HealthCheck {
                    command: "exit 0".to_string(),
                    retries: 1,
                    delay_secs: 0,
                }),
                ..Default::default()
            }],
        };

        let run = run_bare(&pipeline, repo.path()).await;

        assert_eq!(run.status, RunStatus::Failed);
        assert!(run.health_failure_stage.is_none());
        assert!(run.stage_results[0].health_check_passed.is_none());
    }

    #[tokio::test]
    async fn test_passing_health_check_records_no_failure() {
        let repo = TempDir::new().unwrap();
        let pipeline = Pipeline {
            name: "test".to_string(),
            on_health_failure: None,
            stages: vec![Stage {
                name: "deploy".to_string(),
                commands: vec!["echo deployed".to_string()],
                health_check: Some(HealthCheck {
                    command: "exit 0".to_string(),
                    retries: 1,
                    delay_secs: 0,
                }),
                ..Default::default()
            }],
        };

        let run = run_bare(&pipeline, repo.path()).await;

        assert_eq!(run.status, RunStatus::Success);
        assert!(run.health_failure_stage.is_none());
        assert_eq!(run.stage_results[0].health_check_passed, Some(true));
    }

    #[tokio::test]
    async fn test_non_fail_fast_failure_marks_run_failed() {
        let repo = TempDir::new().unwrap();
        let pipeline = Pipeline {
            name: "test".to_string(),
            on_health_failure: None,
            stages: vec![
                Stage {
                    name: "fails".to_string(),
                    commands: vec!["exit 1".to_string()],
                    fail_fast: false,
                    ..Default::default()
                },
                Stage {
                    name: "still-runs".to_string(),
                    commands: vec!["echo ok".to_string()],
                    ..Default::default()
                },
            ],
        };

        let run = run_bare(&pipeline, repo.path()).await;

        assert_eq!(run.status, RunStatus::Failed);
        assert_eq!(run.stage_results[0].status, StageStatus::Failed);
        assert_eq!(run.stage_results[1].status, StageStatus::Success);
    }

    #[tokio::test]
    async fn test_stage_timeout_kills_command_and_fails_run() {
        let repo = TempDir::new().unwrap();
        let pipeline = Pipeline {
            name: "test".to_string(),
            on_health_failure: None,
            stages: vec![Stage {
                name: "hangs".to_string(),
                commands: vec!["sleep 30".to_string()],
                timeout_secs: Some(1),
                ..Default::default()
            }],
        };

        let started = std::time::Instant::now();
        let run = run_bare(&pipeline, repo.path()).await;
        let elapsed = started.elapsed();

        assert_eq!(run.stage_results[0].status, StageStatus::TimedOut);
        assert_eq!(run.status, RunStatus::Failed);
        assert!(
            run.stage_results[0].stderr.contains("timed out"),
            "missing timeout marker: {:?}",
            run.stage_results[0].stderr
        );
        // The whole run must end with the deadline, not with `sleep 30`.
        assert!(elapsed.as_secs() < 10, "took too long: {elapsed:?}");
    }

    #[tokio::test]
    async fn test_stage_retries_until_success() {
        let repo = TempDir::new().unwrap();
        let marker = repo.path().join("attempts.txt");
        let marker_path = marker.to_string_lossy().to_string();

        let pipeline = Pipeline {
            name: "test".to_string(),
            on_health_failure: None,
            stages: vec![Stage {
                name: "flaky".to_string(),
                // Succeeds only once the marker file has 3 lines.
                commands: vec![format!(
                    "echo x >> '{p}' && test $(wc -l < '{p}') -ge 3",
                    p = marker_path
                )],
                retry: Some(StageRetry {
                    attempts: 3,
                    delay_secs: 0,
                    backoff: Backoff::Fixed,
                }),
                ..Default::default()
            }],
        };

        let run = run_bare(&pipeline, repo.path()).await;

        assert_eq!(run.status, RunStatus::Success);
        assert_eq!(run.stage_results[0].status, StageStatus::Success);
        assert_eq!(run.stage_results[0].attempts, Some(3));
        assert!(
            run.stage_results[0].stdout.contains("--- attempt 3/3 ---"),
            "attempt separators missing: {:?}",
            run.stage_results[0].stdout
        );
    }

    #[tokio::test]
    async fn test_when_mismatch_skips_stage_without_spawning() {
        let repo = TempDir::new().unwrap();
        let sentinel = repo.path().join("ran.txt");
        let environment = Environment {
            name: "staging".to_string(),
            ssh_host: None,
            ssh_port: None,
            variables: HashMap::new(),
        };

        let pipeline = Pipeline {
            name: "test".to_string(),
            on_health_failure: None,
            stages: vec![Stage {
                name: "prod-only".to_string(),
                commands: vec![format!("touch '{}'", sentinel.to_string_lossy())],
                when: Some(StageWhen {
                    environment: vec!["prod".to_string()],
                    ..Default::default()
                }),
                ..Default::default()
            }],
        };

        let run = run_pipeline(
            &pipeline,
            repo.path(),
            Some(&environment),
            HashMap::new(),
            None,
            None,
            None,
            None,
            "test-run",
            Redactor::default(),
        )
        .await
        .unwrap();

        assert_eq!(run.status, RunStatus::Success);
        assert_eq!(run.stage_results[0].status, StageStatus::Skipped);
        assert_eq!(
            run.stage_results[0].skip_reason.as_deref(),
            Some("when: environment 'staging' does not match [prod]")
        );
        assert!(!sentinel.exists(), "skipped stage still ran its command");
    }

    #[tokio::test]
    async fn test_stage_env_is_scoped_to_its_stage() {
        let repo = TempDir::new().unwrap();
        let pipeline = Pipeline {
            name: "test".to_string(),
            on_health_failure: None,
            stages: vec![
                Stage {
                    name: "with-env".to_string(),
                    commands: vec!["echo \"[$CHIBBY_TEST_STAGE_VAR]\"".to_string()],
                    env: Some(HashMap::from([(
                        "CHIBBY_TEST_STAGE_VAR".to_string(),
                        "scoped".to_string(),
                    )])),
                    ..Default::default()
                },
                Stage {
                    name: "without-env".to_string(),
                    commands: vec!["echo \"[$CHIBBY_TEST_STAGE_VAR]\"".to_string()],
                    ..Default::default()
                },
            ],
        };

        let run = run_bare(&pipeline, repo.path()).await;

        assert_eq!(run.status, RunStatus::Success);
        assert!(
            run.stage_results[0].stdout.contains("[scoped]"),
            "stage env missing: {:?}",
            run.stage_results[0].stdout
        );
        assert!(
            run.stage_results[1].stdout.contains("[]"),
            "stage env leaked to next stage: {:?}",
            run.stage_results[1].stdout
        );
    }

    #[tokio::test]
    async fn test_run_records_git_branch_and_commit() {
        let repo = TempDir::new().unwrap();
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .args(args)
                .current_dir(repo.path())
                .output()
                .unwrap()
        };
        git(&["init"]);
        git(&["config", "user.email", "test@chibby.local"]);
        git(&["config", "user.name", "Chibby Test"]);
        std::fs::write(repo.path().join("file.txt"), "hi").unwrap();
        git(&["add", "."]);
        git(&["commit", "-m", "init"]);

        let pipeline = Pipeline {
            name: "test".to_string(),
            on_health_failure: None,
            stages: vec![Stage {
                name: "noop".to_string(),
                commands: vec!["true".to_string()],
                ..Default::default()
            }],
        };

        let run = run_bare(&pipeline, repo.path()).await;

        assert!(run.branch.is_some(), "branch not recorded");
        assert_ne!(run.branch.as_deref(), Some("HEAD"));
        assert!(run.commit.is_some(), "commit not recorded");
    }

    #[tokio::test]
    async fn test_secret_values_are_masked_in_persisted_logs() {
        let repo = TempDir::new().unwrap();
        let pipeline = Pipeline {
            name: "test".to_string(),
            on_health_failure: None,
            stages: vec![Stage {
                name: "leaks".to_string(),
                commands: vec!["echo value-is-sup3rs3cret".to_string()],
                ..Default::default()
            }],
        };

        let run = run_pipeline(
            &pipeline,
            repo.path(),
            None,
            HashMap::new(),
            None,
            None,
            None,
            None,
            "test-run",
            Redactor::new(["sup3rs3cret".to_string()]),
        )
        .await
        .unwrap();

        assert!(
            !run.stage_results[0].stdout.contains("sup3rs3cret"),
            "secret persisted in logs: {:?}",
            run.stage_results[0].stdout
        );
        assert!(run.stage_results[0].stdout.contains("[REDACTED]"));
    }
}
