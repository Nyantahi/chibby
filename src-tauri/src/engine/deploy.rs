//! Deploy-backend helpers split out of `executor`: SSH command building, health
//! checks, and Docker Compose service verification. The pipeline loop in
//! `executor::run_pipeline` calls into these; local-command building and shell
//! selection stay in `executor` since they're shared with the agent chokepoint.

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::engine::executor::{build_local_command, LogCallback};
use crate::engine::models::{Backend, Environment, HealthCheck};
use crate::engine::redact::Redactor;

/// One stage's log sink, redacting every line before it leaves the process.
///
/// Health checks stream through the same callback as stage commands, so they
/// need the same "redact at ingest" guarantee: a check like
/// `curl -sv -H "Authorization: Bearer $API_TOKEN" ...` echoes the resolved
/// token on both the command line and in the response headers.
pub(crate) struct StageLog<'a> {
    on_log: &'a Option<LogCallback>,
    redactor: &'a Redactor,
    stage_name: &'a str,
}

impl<'a> StageLog<'a> {
    pub(crate) fn new(
        on_log: &'a Option<LogCallback>,
        redactor: &'a Redactor,
        stage_name: &'a str,
    ) -> Self {
        Self {
            on_log,
            redactor,
            stage_name,
        }
    }

    pub(crate) fn emit(&self, kind: &str, line: &str) {
        if let Some(ref cb) = self.on_log {
            cb(self.stage_name, kind, &self.redactor.redact_log(line));
        }
    }
}

/// Build an SSH command that executes a command string on a remote host.
pub(crate) fn build_ssh_command(
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

    // Validate SSH host to prevent option injection (e.g. "-o ProxyCommand=...")
    if host.starts_with('-') || host.contains(' ') || host.contains('\n') {
        anyhow::bail!(
            "Invalid ssh_host value '{}': must not start with '-' or contain spaces",
            host
        );
    }

    // Build the remote command string with env exports and cd.
    let mut remote_parts = Vec::new();

    // Export environment variables on the remote side.
    for (key, value) in env_vars {
        if !is_valid_env_var_name(key) {
            anyhow::bail!(
                "Invalid environment variable name '{}': must match [A-Za-z_][A-Za-z0-9_]*",
                key
            );
        }
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
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_health_check(
    health_check: &HealthCheck,
    backend: &Backend,
    environment: Option<&Environment>,
    repo_path: &Path,
    working_dir: &Option<String>,
    env_vars: &HashMap<String, String>,
    log: &StageLog<'_>,
) -> bool {
    for attempt in 1..=health_check.retries {
        log.emit(
            "info",
            &format!(
                "Health check attempt {}/{}: {}",
                attempt, health_check.retries, health_check.command
            ),
        );

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
                        log.emit("stdout", &line);
                    }
                }
                if let Some(stderr) = child.stderr.take() {
                    let reader = BufReader::new(stderr);
                    let mut lines = reader.lines();
                    while let Some(line) = lines.next_line().await.unwrap_or(None) {
                        log.emit("stderr", &line);
                    }
                }

                if let Ok(status) = child.wait().await {
                    if status.success() {
                        log.emit("info", "Health check passed");
                        return true;
                    }
                }
            }
            Err(e) => {
                log.emit("error", &format!("Health check error: {e}"));
            }
        }

        if attempt < health_check.retries {
            log.emit(
                "info",
                &format!("Retrying in {} seconds...", health_check.delay_secs),
            );
            tokio::time::sleep(std::time::Duration::from_secs(
                health_check.delay_secs as u64,
            ))
            .await;
        }
    }

    false
}

/// Auto-check docker compose services after a `docker compose up` command.
pub(crate) async fn check_docker_compose_services(
    environment: Option<&Environment>,
    working_dir: &Option<String>,
    env_vars: &HashMap<String, String>,
    log: &StageLog<'_>,
) -> bool {
    log.emit("info", "Checking Docker Compose service status...");

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
                    log.emit("stdout", &line);
                }
            }
            if let Some(stderr) = child.stderr.take() {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Some(line) = lines.next_line().await.unwrap_or(None) {
                    log.emit("stderr", &line);
                }
            }

            match child.wait().await {
                Ok(status) if status.success() => {
                    // Check for unhealthy or exited services in the output.
                    let has_issues = output.contains("\"exited\"")
                        || output.contains("\"dead\"")
                        || output.contains("\"restarting\"");
                    if has_issues {
                        log.emit("error", "Some Docker Compose services are not healthy");
                        return false;
                    }
                    log.emit("info", "All Docker Compose services running");
                    true
                }
                _ => {
                    log.emit("error", "Failed to check Docker Compose service status");
                    false
                }
            }
        }
        Err(e) => {
            log.emit("error", &format!("Docker Compose check error: {e}"));
            false
        }
    }
}

/// Shell-escape a value for safe embedding in a remote command.
fn shell_escape(s: &str) -> String {
    // Use single quotes with escaped single quotes.
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Validate that an environment variable name is safe for shell use.
fn is_valid_env_var_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let first = name.as_bytes()[0];
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return false;
    }
    name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
