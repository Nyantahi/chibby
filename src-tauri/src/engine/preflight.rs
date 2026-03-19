use crate::engine::models::{Backend, EnvironmentsConfig, Pipeline, SecretsConfig};
use crate::engine::secrets;
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Result of preflight validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightResult {
    pub passed: bool,
    pub errors: Vec<PreflightError>,
    pub warnings: Vec<String>,
}

/// A single preflight validation error.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "detail")]
pub enum PreflightError {
    MissingSecret { name: String, environment: String },
    MissingSshHost { stage: String },
    MissingEnvironment { name: String },
    SshConnectivityFailed { host: String, error: String },
    SshNotAvailable,
}

/// Run preflight validation for a pipeline against a target environment.
pub async fn validate_preflight(
    pipeline: &Pipeline,
    project_path: &str,
    env_name: &str,
    environments_config: &EnvironmentsConfig,
    secrets_config: &SecretsConfig,
) -> Result<PreflightResult> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // 1. Check that the target environment exists.
    let env = environments_config
        .environments
        .iter()
        .find(|e| e.name == env_name);

    let env = match env {
        Some(e) => e,
        None => {
            errors.push(PreflightError::MissingEnvironment {
                name: env_name.to_string(),
            });
            return Ok(PreflightResult {
                passed: false,
                errors,
                warnings,
            });
        }
    };

    // 2. Check SSH stages have a host configured.
    let has_ssh_stages = pipeline
        .stages
        .iter()
        .any(|s| s.backend == Backend::Ssh);

    if has_ssh_stages {
        if env.ssh_host.is_none() {
            for stage in &pipeline.stages {
                if stage.backend == Backend::Ssh {
                    errors.push(PreflightError::MissingSshHost {
                        stage: stage.name.clone(),
                    });
                }
            }
        }

        // Check that ssh binary is available.
        if !ssh_available().await {
            errors.push(PreflightError::SshNotAvailable);
        }
    }

    // 3. Check secrets are set in the keychain.
    let statuses = secrets::check_secrets_status(project_path, env_name, secrets_config);
    for status in &statuses {
        if !status.is_set {
            errors.push(PreflightError::MissingSecret {
                name: status.name.clone(),
                environment: env_name.to_string(),
            });
        }
    }

    // 4. Optional SSH connectivity test (only if host is set and no prior SSH errors).
    if has_ssh_stages && errors.is_empty() {
        if let Some(ref host) = env.ssh_host {
            if let Err(e) = test_ssh_connectivity(host, env.ssh_port).await {
                errors.push(PreflightError::SshConnectivityFailed {
                    host: host.clone(),
                    error: e.to_string(),
                });
            }
        }
    }

    // 5. Warn about stages without health checks on SSH deploys.
    for stage in &pipeline.stages {
        if stage.backend == Backend::Ssh && stage.health_check.is_none() {
            let has_deploy_cmd = stage
                .commands
                .iter()
                .any(|c| c.contains("docker compose up") || c.contains("deploy"));
            if has_deploy_cmd {
                warnings.push(format!(
                    "Stage '{}' deploys without a health check — consider adding one",
                    stage.name
                ));
            }
        }
    }

    let passed = errors.is_empty();
    Ok(PreflightResult {
        passed,
        errors,
        warnings,
    })
}

/// Check if the ssh binary is available on PATH.
async fn ssh_available() -> bool {
    #[cfg(target_os = "windows")]
    let result = tokio::process::Command::new("where")
        .arg("ssh")
        .output()
        .await;

    #[cfg(not(target_os = "windows"))]
    let result = tokio::process::Command::new("which")
        .arg("ssh")
        .output()
        .await;

    match result {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

/// Test SSH connectivity to a host.
pub async fn test_ssh_connectivity(host: &str, port: Option<u16>) -> Result<String> {
    let mut cmd = tokio::process::Command::new("ssh");
    cmd.arg("-o").arg("BatchMode=yes")
        .arg("-o").arg("ConnectTimeout=5")
        .arg("-o").arg("StrictHostKeyChecking=accept-new");

    if let Some(p) = port {
        cmd.arg("-p").arg(p.to_string());
    }

    cmd.arg(host).arg("echo chibby-ok");

    let output = cmd.output().await?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("SSH connection failed: {}", stderr.trim())
    }
}
