use crate::engine::deploy::{build_ssh_command, check_docker_compose_services, run_health_check};
use crate::engine::models::{
    Backend, Environment, Pipeline, PipelineRun, RunStatus, Stage, StageResult, StageStatus,
};
use crate::state::SharedPipelineState;
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

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
) -> Result<PipelineRun> {
    let env_name = environment.map(|e| e.name.clone());
    let mut run = PipelineRun::new_with_id(
        run_id,
        &pipeline.name,
        &repo_path.to_string_lossy(),
        env_name,
    );
    run.status = RunStatus::Running;
    let mut had_failures = false;

    for stage in &pipeline.stages {
        // Skip stages not in the filter (if a filter is provided).
        if let Some(filter) = &stage_filter {
            if !filter.iter().any(|f| f == &stage.name) {
                run.stage_results.push(skipped_stage(&stage.name));
                continue;
            }
        }

        let (result, control) = run_stage(
            stage,
            &mut run,
            repo_path,
            environment,
            &env_vars,
            &on_log,
            &cancel_state,
            &on_stage_complete,
        )
        .await;

        if let StageControl::Cancelled = control {
            run.status = RunStatus::Cancelled;
            run.stage_results.push(result);
            mark_remaining_skipped(&mut run, pipeline);
            finalize_run(&mut run);
            return Ok(run);
        }

        let failed = result.status == StageStatus::Failed;
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

/// Run a single stage: stream its commands, run any health check, and report the
/// result plus what the loop should do next. Pushes a `Running` placeholder and
/// persists it before execution so crash recovery sees the active stage.
#[allow(clippy::too_many_arguments)]
async fn run_stage(
    stage: &Stage,
    run: &mut PipelineRun,
    repo_path: &Path,
    environment: Option<&Environment>,
    env_vars: &HashMap<String, String>,
    on_log: &Option<LogCallback>,
    cancel_state: &Option<SharedPipelineState>,
    on_stage_complete: &Option<StageCallback>,
) -> (StageResult, StageControl) {
    let stage_start = Utc::now();

    if let Some(ref cb) = on_log {
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

    let mut stage_stdout = String::new();
    let mut stage_stderr = String::new();
    let mut stage_exit_code: Option<i32> = None;
    let mut stage_status = StageStatus::Running;

    for cmd_str in &stage.commands {
        // Check for cancellation before each command.
        if is_cancelled(cancel_state, repo_path).await {
            if let Some(ref cb) = on_log {
                cb(&stage.name, "warn", "Pipeline cancelled by user");
            }
            return (
                cancelled_stage(&stage.name, stage_start, stage_stdout, stage_stderr),
                StageControl::Cancelled,
            );
        }

        if let Some(ref cb) = on_log {
            cb(&stage.name, "cmd", &format!("$ {}", cmd_str));
        }

        log::info!(
            "[executor] stage='{}' spawning command: {}",
            stage.name,
            cmd_str.chars().take(120).collect::<String>()
        );

        let mut child = match stage.backend {
            Backend::Local => {
                match build_local_command(cmd_str, repo_path, &stage.working_dir, env_vars) {
                    Ok(c) => c,
                    Err(e) => {
                        log::error!(
                            "[executor] stage='{}' failed to spawn command: {}",
                            stage.name,
                            e
                        );
                        if let Some(ref cb) = on_log {
                            cb(&stage.name, "error", &format!("Failed to spawn: {}", e));
                        }
                        stage_stderr.push_str(&format!("Failed to spawn command: {}\n", e));
                        stage_status = StageStatus::Failed;
                        break;
                    }
                }
            }
            Backend::Ssh => {
                match build_ssh_command(cmd_str, environment, &stage.working_dir, env_vars) {
                    Ok(c) => c,
                    Err(e) => {
                        log::error!("[executor] stage='{}' SSH command error: {}", stage.name, e);
                        if let Some(ref cb) = on_log {
                            cb(&stage.name, "error", &format!("SSH error: {}", e));
                        }
                        stage_stderr.push_str(&format!("SSH command error: {}\n", e));
                        stage_status = StageStatus::Failed;
                        break;
                    }
                }
            }
        };

        // Register the child PID for cancellation handling.
        let child_pid = child.id();
        log::info!(
            "[executor] stage='{}' spawned pid={:?}",
            stage.name,
            child_pid
        );
        if let (Some(state), Some(pid)) = (cancel_state, child_pid) {
            let mut guard = state.write().await;
            guard.set_running_pid(&repo_path.to_string_lossy(), pid);
        }

        // Stream stdout and stderr concurrently to avoid pipe deadlock. Reading
        // them sequentially can hang if the child fills the stderr pipe buffer
        // while we're still draining stdout (or vice-versa). Also monitor for
        // cancellation and kill the child if requested.
        let was_cancelled;
        {
            let stdout_pipe = child.stdout.take();
            let stderr_pipe = child.stderr.take();

            let stage_name_out = stage.name.clone();
            let stage_name_err = stage.name.clone();
            let on_log_ref = on_log;

            let stdout_task = async {
                let mut out = String::new();
                if let Some(pipe) = stdout_pipe {
                    let reader = BufReader::new(pipe);
                    let mut lines = reader.lines();
                    while let Some(line) = lines.next_line().await.unwrap_or(None) {
                        out.push_str(&line);
                        out.push('\n');
                        if let Some(ref cb) = on_log_ref {
                            cb(&stage_name_out, "stdout", &line);
                        }
                    }
                }
                out
            };

            let stderr_task = async {
                let mut err = String::new();
                if let Some(pipe) = stderr_pipe {
                    let reader = BufReader::new(pipe);
                    let mut lines = reader.lines();
                    while let Some(line) = lines.next_line().await.unwrap_or(None) {
                        err.push_str(&line);
                        err.push('\n');
                        if let Some(ref cb) = on_log_ref {
                            cb(&stage_name_err, "stderr", &line);
                        }
                    }
                }
                err
            };

            // Cancellation monitor - polls every 200ms and kills the child.
            let cancel_state_ref = cancel_state.clone();
            let repo_path_str = repo_path.to_string_lossy().to_string();
            let cancel_task = async {
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    if let Some(ref state) = cancel_state_ref {
                        let cancelled = {
                            let guard = state.read().await;
                            guard.is_cancelled(&repo_path_str)
                        };
                        if cancelled {
                            return true;
                        }
                    }
                }
            };

            // Race the I/O tasks against the cancellation monitor.
            tokio::select! {
                biased;

                (out, err) = async { tokio::join!(stdout_task, stderr_task) } => {
                    stage_stdout.push_str(&out);
                    stage_stderr.push_str(&err);
                    was_cancelled = false;
                }
                _ = cancel_task => {
                    // Cancellation requested - kill the child process.
                    if let Err(e) = child.kill().await {
                        // Process may have already exited.
                        if let Some(ref cb) = on_log {
                            cb(&stage.name, "warn", &format!("Failed to kill process: {}", e));
                        }
                    }
                    was_cancelled = true;
                }
            }
        }

        // Clear the running PID.
        if let Some(state) = cancel_state {
            let mut guard = state.write().await;
            guard.clear_running_pid(&repo_path.to_string_lossy());
        }

        if was_cancelled {
            if let Some(ref cb) = on_log {
                cb(&stage.name, "warn", "Pipeline cancelled by user");
            }
            return (
                cancelled_stage(&stage.name, stage_start, stage_stdout, stage_stderr),
                StageControl::Cancelled,
            );
        }

        log::info!(
            "[executor] stage='{}' I/O complete, waiting for exit",
            stage.name
        );
        let output = match child.wait().await {
            Ok(status) => status,
            Err(e) => {
                log::error!("[executor] stage='{}' wait() failed: {}", stage.name, e);
                if let Some(ref cb) = on_log {
                    cb(&stage.name, "error", &format!("Process wait failed: {}", e));
                }
                stage_stderr.push_str(&format!("Process wait failed: {}\n", e));
                stage_status = StageStatus::Failed;
                break;
            }
        };
        stage_exit_code = output.code();
        log::info!(
            "[executor] stage='{}' exited code={:?} success={}",
            stage.name,
            stage_exit_code,
            output.success()
        );

        if !output.success() {
            stage_status = StageStatus::Failed;
            if let Some(ref cb) = on_log {
                cb(
                    &stage.name,
                    "error",
                    &format!("Command failed with exit code: {:?}", stage_exit_code),
                );
            }
            break;
        }
    }

    if stage_status != StageStatus::Failed {
        stage_status = StageStatus::Success;
    }

    // Run health check if the stage succeeded and has one configured.
    let health_check_passed = stage_health_result(
        stage,
        &mut stage_status,
        environment,
        repo_path,
        env_vars,
        on_log,
    )
    .await;

    let stage_end = Utc::now();
    let duration = (stage_end - stage_start).num_milliseconds() as u64;

    let result = StageResult {
        stage_name: stage.name.clone(),
        status: stage_status.clone(),
        exit_code: stage_exit_code,
        stdout: stage_stdout,
        stderr: stage_stderr,
        started_at: Some(stage_start),
        finished_at: Some(stage_end),
        duration_ms: Some(duration),
        health_check_passed,
    };

    let control = if stage_status == StageStatus::Failed && stage.fail_fast {
        StageControl::FailFast
    } else {
        StageControl::Continue
    };

    (result, control)
}

/// Resolve a stage's health status: run its configured health check, or
/// auto-check docker compose services when a compose-up ran over SSH. Flips
/// `stage_status` to `Failed` on a failed check. Returns `None` when neither
/// applies.
async fn stage_health_result(
    stage: &Stage,
    stage_status: &mut StageStatus,
    environment: Option<&Environment>,
    repo_path: &Path,
    env_vars: &HashMap<String, String>,
    on_log: &Option<LogCallback>,
) -> Option<bool> {
    if *stage_status != StageStatus::Success {
        return None;
    }

    if let Some(ref hc) = stage.health_check {
        let passed = run_health_check(
            hc,
            &stage.backend,
            environment,
            repo_path,
            &stage.working_dir,
            env_vars,
            on_log,
            &stage.name,
        )
        .await;

        if !passed {
            *stage_status = StageStatus::Failed;
            if let Some(ref cb) = on_log {
                cb(&stage.name, "error", "Health check failed after all retries");
            }
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
        let docker_ok = check_docker_compose_services(
            environment,
            &stage.working_dir,
            env_vars,
            on_log,
            &stage.name,
        )
        .await;
        if !docker_ok {
            *stage_status = StageStatus::Failed;
        }
        return Some(docker_ok);
    }

    None
}

/// Whether the run was cancelled for `repo_path` (false when no cancel state).
async fn is_cancelled(cancel_state: &Option<SharedPipelineState>, repo_path: &Path) -> bool {
    if let Some(state) = cancel_state {
        let guard = state.read().await;
        guard.is_cancelled(&repo_path.to_string_lossy())
    } else {
        false
    }
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
    use crate::engine::models::{Backend, Pipeline, Stage};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_non_fail_fast_failure_marks_run_failed() {
        let repo = TempDir::new().unwrap();
        let pipeline = Pipeline {
            name: "test".to_string(),
            stages: vec![
                Stage {
                    name: "fails".to_string(),
                    commands: vec!["exit 1".to_string()],
                    backend: Backend::Local,
                    working_dir: None,
                    fail_fast: false,
                    health_check: None,
                },
                Stage {
                    name: "still-runs".to_string(),
                    commands: vec!["echo ok".to_string()],
                    backend: Backend::Local,
                    working_dir: None,
                    fail_fast: true,
                    health_check: None,
                },
            ],
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
        )
        .await
        .unwrap();

        assert_eq!(run.status, RunStatus::Failed);
        assert_eq!(run.stage_results[0].status, StageStatus::Failed);
        assert_eq!(run.stage_results[1].status, StageStatus::Success);
    }
}
