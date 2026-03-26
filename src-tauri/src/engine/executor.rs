use crate::engine::models::{
    Backend, Environment, HealthCheck, Pipeline, PipelineRun, RunStatus, StageResult, StageStatus,
};
use crate::state::SharedPipelineState;
use anyhow::Result;
use chrono::Utc;
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

/// Execute an entire pipeline, stage by stage.
///
/// Supports both local and SSH execution backends. Environment variables
/// and resolved secrets are injected into every command.
pub async fn run_pipeline(
    pipeline: &Pipeline,
    repo_path: &Path,
    environment: Option<&Environment>,
    env_vars: HashMap<String, String>,
    on_log: Option<LogCallback>,
    stage_filter: Option<&[String]>,
    cancel_state: Option<SharedPipelineState>,
    on_stage_complete: Option<StageCallback>,
) -> Result<PipelineRun> {
    let env_name = environment.map(|e| e.name.clone());
    let mut run = PipelineRun::new(&pipeline.name, &repo_path.to_string_lossy(), env_name);
    run.status = RunStatus::Running;
    let mut had_failures = false;

    for stage in &pipeline.stages {
        // Skip stages not in the filter (if a filter is provided).
        if let Some(filter) = &stage_filter {
            if !filter.iter().any(|f| f == &stage.name) {
                run.stage_results.push(StageResult {
                    stage_name: stage.name.clone(),
                    status: StageStatus::Skipped,
                    exit_code: None,
                    stdout: String::new(),
                    stderr: String::new(),
                    started_at: None,
                    finished_at: None,
                    duration_ms: None,
                    health_check_passed: None,
                });
                continue;
            }
        }
        let stage_start = Utc::now();

        if let Some(ref cb) = on_log {
            cb(
                &stage.name,
                "info",
                &format!("--- Starting stage: {} ---", stage.name),
            );
        }

        let mut stage_stdout = String::new();
        let mut stage_stderr = String::new();
        let mut stage_exit_code: Option<i32> = None;
        let mut stage_status = StageStatus::Running;

        for cmd_str in &stage.commands {
            // Check for cancellation before each command
            if let Some(ref state) = cancel_state {
                let cancelled = {
                    let guard = state.read().await;
                    guard.is_cancelled(&repo_path.to_string_lossy())
                };
                if cancelled {
                    if let Some(ref cb) = on_log {
                        cb(&stage.name, "warn", "Pipeline cancelled by user");
                    }
                    run.status = RunStatus::Cancelled;
                    // Mark current stage as skipped
                    run.stage_results.push(StageResult {
                        stage_name: stage.name.clone(),
                        status: StageStatus::Skipped,
                        exit_code: None,
                        stdout: stage_stdout.clone(),
                        stderr: stage_stderr.clone(),
                        started_at: Some(stage_start),
                        finished_at: Some(Utc::now()),
                        duration_ms: None,
                        health_check_passed: None,
                    });
                    // Mark remaining stages as skipped
                    let done_count = run.stage_results.len();
                    for remaining in pipeline.stages.iter().skip(done_count) {
                        run.stage_results.push(StageResult {
                            stage_name: remaining.name.clone(),
                            status: StageStatus::Skipped,
                            exit_code: None,
                            stdout: String::new(),
                            stderr: String::new(),
                            started_at: None,
                            finished_at: None,
                            duration_ms: None,
                            health_check_passed: None,
                        });
                    }
                    let end = Utc::now();
                    run.finished_at = Some(end);
                    run.duration_ms = Some((end - run.started_at).num_milliseconds() as u64);
                    return Ok(run);
                }
            }

            if let Some(ref cb) = on_log {
                cb(&stage.name, "cmd", &format!("$ {}", cmd_str));
            }

            let mut child = match stage.backend {
                Backend::Local => {
                    build_local_command(cmd_str, repo_path, &stage.working_dir, &env_vars)?
                }
                Backend::Ssh => {
                    build_ssh_command(cmd_str, environment, &stage.working_dir, &env_vars)?
                }
            };

            // Register the child PID for cancellation handling
            let child_pid = child.id();
            if let (Some(ref state), Some(pid)) = (&cancel_state, child_pid) {
                let mut guard = state.write().await;
                guard.set_running_pid(&repo_path.to_string_lossy(), pid);
            }

            // Stream stdout and stderr concurrently to avoid pipe deadlock.
            // Reading them sequentially can hang if the child fills the stderr
            // pipe buffer while we're still draining stdout (or vice-versa).
            // Also monitor for cancellation and kill the child if requested.
            let was_cancelled;
            {
                let stdout_pipe = child.stdout.take();
                let stderr_pipe = child.stderr.take();

                let stage_name_out = stage.name.clone();
                let stage_name_err = stage.name.clone();
                let on_log_ref = &on_log;

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

                // Cancellation monitor task - polls every 200ms and kills the child if cancelled
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

                // Race the I/O tasks against the cancellation monitor
                tokio::select! {
                    biased;

                    (out, err) = async { tokio::join!(stdout_task, stderr_task) } => {
                        stage_stdout.push_str(&out);
                        stage_stderr.push_str(&err);
                        was_cancelled = false;
                    }
                    _ = cancel_task => {
                        // Cancellation requested - kill the child process
                        if let Err(e) = child.kill().await {
                            // Process may have already exited
                            if let Some(ref cb) = on_log {
                                cb(&stage.name, "warn", &format!("Failed to kill process: {}", e));
                            }
                        }
                        was_cancelled = true;
                    }
                }
            }

            // Clear the running PID
            if let Some(ref state) = cancel_state {
                let mut guard = state.write().await;
                guard.clear_running_pid(&repo_path.to_string_lossy());
            }

            // Handle cancellation
            if was_cancelled {
                if let Some(ref cb) = on_log {
                    cb(&stage.name, "warn", "Pipeline cancelled by user");
                }
                run.status = RunStatus::Cancelled;
                // Mark current stage as cancelled/skipped
                run.stage_results.push(StageResult {
                    stage_name: stage.name.clone(),
                    status: StageStatus::Skipped,
                    exit_code: None,
                    stdout: stage_stdout.clone(),
                    stderr: stage_stderr.clone(),
                    started_at: Some(stage_start),
                    finished_at: Some(Utc::now()),
                    duration_ms: None,
                    health_check_passed: None,
                });
                // Mark remaining stages as skipped
                let done_count = run.stage_results.len();
                for remaining in pipeline.stages.iter().skip(done_count) {
                    run.stage_results.push(StageResult {
                        stage_name: remaining.name.clone(),
                        status: StageStatus::Skipped,
                        exit_code: None,
                        stdout: String::new(),
                        stderr: String::new(),
                        started_at: None,
                        finished_at: None,
                        duration_ms: None,
                        health_check_passed: None,
                    });
                }
                let end = Utc::now();
                run.finished_at = Some(end);
                run.duration_ms = Some((end - run.started_at).num_milliseconds() as u64);
                return Ok(run);
            }

            let output = child.wait().await?;
            stage_exit_code = output.code();

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
        let health_check_passed = if stage_status == StageStatus::Success {
            if let Some(ref hc) = stage.health_check {
                let passed = run_health_check(
                    hc,
                    &stage.backend,
                    environment,
                    repo_path,
                    &stage.working_dir,
                    &env_vars,
                    &on_log,
                    &stage.name,
                )
                .await;

                if !passed {
                    stage_status = StageStatus::Failed;
                    if let Some(ref cb) = on_log {
                        cb(
                            &stage.name,
                            "error",
                            "Health check failed after all retries",
                        );
                    }
                }
                Some(passed)
            } else {
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
                        &env_vars,
                        &on_log,
                        &stage.name,
                    )
                    .await;
                    if !docker_ok {
                        stage_status = StageStatus::Failed;
                    }
                    Some(docker_ok)
                } else {
                    None
                }
            }
        } else {
            None
        };

        let stage_end = Utc::now();
        let duration = (stage_end - stage_start).num_milliseconds() as u64;

        run.stage_results.push(StageResult {
            stage_name: stage.name.clone(),
            status: stage_status.clone(),
            exit_code: stage_exit_code,
            stdout: stage_stdout,
            stderr: stage_stderr,
            started_at: Some(stage_start),
            finished_at: Some(stage_end),
            duration_ms: Some(duration),
            health_check_passed,
        });

        // Persist partial run state so results survive a crash.
        if let Some(ref cb) = on_stage_complete {
            cb(&run);
        }

        if stage_status == StageStatus::Failed {
            had_failures = true;
        }

        if stage_status == StageStatus::Failed && stage.fail_fast {
            run.status = RunStatus::Failed;
            // Mark remaining stages as skipped.
            let done_count = run.stage_results.len();
            for remaining in pipeline.stages.iter().skip(done_count) {
                run.stage_results.push(StageResult {
                    stage_name: remaining.name.clone(),
                    status: StageStatus::Skipped,
                    exit_code: None,
                    stdout: String::new(),
                    stderr: String::new(),
                    started_at: None,
                    finished_at: None,
                    duration_ms: None,
                    health_check_passed: None,
                });
            }
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

    let end = Utc::now();
    run.finished_at = Some(end);
    run.duration_ms = Some((end - run.started_at).num_milliseconds() as u64);

    Ok(run)
}

/// Build a local shell command with environment variable injection.
fn build_local_command(
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

    let child = Command::new(&shell)
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
        .spawn()?;

    Ok(child)
}

/// Build an SSH command that executes a command string on a remote host.
fn build_ssh_command(
    cmd_str: &str,
    environment: Option<&Environment>,
    working_dir: &Option<String>,
    env_vars: &HashMap<String, String>,
) -> Result<tokio::process::Child> {
    let env = environment.ok_or_else(|| {
        anyhow::anyhow!("SSH backend requires an environment with ssh_host configured")
    })?;
    let host = env
        .ssh_host
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Environment '{}' has no ssh_host configured", env.name))?;

    // Build the remote command string with env exports and cd.
    let mut remote_parts = Vec::new();

    // Export environment variables on the remote side.
    for (key, value) in env_vars {
        remote_parts.push(format!("export {}={}", key, shell_escape(value)));
    }

    // Change to working directory if specified.
    if let Some(wd) = working_dir {
        remote_parts.push(format!("cd {}", shell_escape(wd)));
    }

    remote_parts.push(cmd_str.to_string());

    let remote_cmd = remote_parts.join(" && ");

    let mut cmd = Command::new("ssh");
    cmd.arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg("StrictHostKeyChecking=accept-new");

    if let Some(port) = env.ssh_port {
        cmd.arg("-p").arg(port.to_string());
    }

    cmd.arg(host)
        .arg(&remote_cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let child = cmd.spawn()?;
    Ok(child)
}

/// Run a health check with retries.
async fn run_health_check(
    health_check: &HealthCheck,
    backend: &Backend,
    environment: Option<&Environment>,
    repo_path: &Path,
    working_dir: &Option<String>,
    env_vars: &HashMap<String, String>,
    on_log: &Option<LogCallback>,
    stage_name: &str,
) -> bool {
    for attempt in 1..=health_check.retries {
        if let Some(ref cb) = on_log {
            cb(
                stage_name,
                "info",
                &format!(
                    "Health check attempt {}/{}: {}",
                    attempt, health_check.retries, health_check.command
                ),
            );
        }

        let result = match backend {
            Backend::Local => {
                build_local_command(&health_check.command, repo_path, working_dir, env_vars)
            }
            Backend::Ssh => {
                build_ssh_command(&health_check.command, environment, working_dir, env_vars)
            }
        };

        match result {
            Ok(mut child) => {
                // Drain stdout/stderr.
                if let Some(stdout) = child.stdout.take() {
                    let reader = BufReader::new(stdout);
                    let mut lines = reader.lines();
                    while let Some(line) = lines.next_line().await.unwrap_or(None) {
                        if let Some(ref cb) = on_log {
                            cb(stage_name, "stdout", &line);
                        }
                    }
                }
                if let Some(stderr) = child.stderr.take() {
                    let reader = BufReader::new(stderr);
                    let mut lines = reader.lines();
                    while let Some(line) = lines.next_line().await.unwrap_or(None) {
                        if let Some(ref cb) = on_log {
                            cb(stage_name, "stderr", &line);
                        }
                    }
                }

                match child.wait().await {
                    Ok(status) if status.success() => {
                        if let Some(ref cb) = on_log {
                            cb(stage_name, "info", "Health check passed");
                        }
                        return true;
                    }
                    _ => {}
                }
            }
            Err(e) => {
                if let Some(ref cb) = on_log {
                    cb(stage_name, "error", &format!("Health check error: {}", e));
                }
            }
        }

        if attempt < health_check.retries {
            if let Some(ref cb) = on_log {
                cb(
                    stage_name,
                    "info",
                    &format!("Retrying in {} seconds...", health_check.delay_secs),
                );
            }
            tokio::time::sleep(std::time::Duration::from_secs(
                health_check.delay_secs as u64,
            ))
            .await;
        }
    }

    false
}

/// Auto-check docker compose services after a `docker compose up` command.
async fn check_docker_compose_services(
    environment: Option<&Environment>,
    working_dir: &Option<String>,
    env_vars: &HashMap<String, String>,
    on_log: &Option<LogCallback>,
    stage_name: &str,
) -> bool {
    if let Some(ref cb) = on_log {
        cb(
            stage_name,
            "info",
            "Checking Docker Compose service status...",
        );
    }

    let check_cmd = "docker compose ps --format json";
    let result = build_ssh_command(check_cmd, environment, working_dir, env_vars);

    match result {
        Ok(mut child) => {
            let mut output = String::new();
            if let Some(stdout) = child.stdout.take() {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Some(line) = lines.next_line().await.unwrap_or(None) {
                    output.push_str(&line);
                    output.push('\n');
                    if let Some(ref cb) = on_log {
                        cb(stage_name, "stdout", &line);
                    }
                }
            }
            if let Some(stderr) = child.stderr.take() {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Some(line) = lines.next_line().await.unwrap_or(None) {
                    if let Some(ref cb) = on_log {
                        cb(stage_name, "stderr", &line);
                    }
                }
            }

            match child.wait().await {
                Ok(status) if status.success() => {
                    // Check for unhealthy or exited services in the output.
                    let has_issues = output.contains("\"exited\"")
                        || output.contains("\"dead\"")
                        || output.contains("\"restarting\"");
                    if has_issues {
                        if let Some(ref cb) = on_log {
                            cb(
                                stage_name,
                                "error",
                                "Some Docker Compose services are not healthy",
                            );
                        }
                        return false;
                    }
                    if let Some(ref cb) = on_log {
                        cb(stage_name, "info", "All Docker Compose services running");
                    }
                    true
                }
                _ => {
                    if let Some(ref cb) = on_log {
                        cb(
                            stage_name,
                            "error",
                            "Failed to check Docker Compose service status",
                        );
                    }
                    false
                }
            }
        }
        Err(e) => {
            if let Some(ref cb) = on_log {
                cb(
                    stage_name,
                    "error",
                    &format!("Docker Compose check error: {}", e),
                );
            }
            false
        }
    }
}

/// Shell-escape a value for safe embedding in a remote command.
fn shell_escape(s: &str) -> String {
    // Use single quotes with escaped single quotes.
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Get the default shell for the current platform.
fn get_shell() -> String {
    #[cfg(target_os = "windows")]
    {
        "cmd".to_string()
    }
    #[cfg(not(target_os = "windows"))]
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
        )
        .await
        .unwrap();

        assert_eq!(run.status, RunStatus::Failed);
        assert_eq!(run.stage_results[0].status, StageStatus::Failed);
        assert_eq!(run.stage_results[1].status, StageStatus::Success);
    }
}
