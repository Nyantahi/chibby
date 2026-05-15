use crate::engine::audit;
use crate::engine::bootstrap::{self, ApplyMode, BootstrapReport};
use crate::engine::models::{EnvironmentsConfig, SecretsConfig};
use crate::engine::pipeline;
use crate::engine::preflight;
use crate::engine::secrets;
use std::path::Path;

/// Load environments config from a project's .chibby/environments.toml (committed file only).
#[tauri::command]
pub fn load_environments(repo_path: String) -> Result<EnvironmentsConfig, String> {
    pipeline::load_environments(Path::new(&repo_path)).map_err(|e| e.to_string())
}

/// Load environments with `environments.local.toml` overrides applied.
/// Use this for read-only/run-time views; use `load_environments` for editing the committed file.
#[tauri::command]
pub fn load_environments_layered(repo_path: String) -> Result<EnvironmentsConfig, String> {
    pipeline::load_environments_layered(Path::new(&repo_path)).map_err(|e| e.to_string())
}

/// Load per-developer overrides from `.chibby/environments.local.toml`.
#[tauri::command]
pub fn load_environments_local(repo_path: String) -> Result<EnvironmentsConfig, String> {
    pipeline::load_environments_local(Path::new(&repo_path)).map_err(|e| e.to_string())
}

/// Save the committed environments config.
#[tauri::command]
pub fn save_environments(
    repo_path: String,
    config: EnvironmentsConfig,
) -> Result<(), String> {
    let env_names: Vec<&str> = config.environments.iter().map(|e| e.name.as_str()).collect();
    audit::log_event(
        "save_environments",
        &format!("project={} envs={:?}", repo_path, env_names),
    );
    pipeline::save_environments(Path::new(&repo_path), &config).map_err(|e| e.to_string())
}

/// Save per-developer overrides (auto-adds `.gitignore` entries).
#[tauri::command]
pub fn save_environments_local(
    repo_path: String,
    config: EnvironmentsConfig,
) -> Result<(), String> {
    let env_names: Vec<&str> = config.environments.iter().map(|e| e.name.as_str()).collect();
    audit::log_event(
        "save_environments_local",
        &format!("project={} envs={:?}", repo_path, env_names),
    );
    pipeline::save_environments_local(Path::new(&repo_path), &config).map_err(|e| e.to_string())
}

/// Load secrets config from a project's .chibby/secrets.toml.
#[tauri::command]
pub fn load_secrets_config(repo_path: String) -> Result<SecretsConfig, String> {
    pipeline::load_secrets_config(Path::new(&repo_path)).map_err(|e| e.to_string())
}

/// Save secrets config to a project's .chibby/secrets.toml.
#[tauri::command]
pub fn save_secrets_config(
    repo_path: String,
    config: SecretsConfig,
) -> Result<(), String> {
    pipeline::save_secrets_config(Path::new(&repo_path), &config).map_err(|e| e.to_string())
}

/// Store a secret value in the OS keychain.
#[tauri::command]
pub fn set_secret(
    project_path: String,
    env_name: String,
    secret_name: String,
    value: String,
) -> Result<(), String> {
    audit::log_event(
        "set_secret",
        &format!("project={} env={} secret={}", project_path, env_name, secret_name),
    );
    secrets::set_secret(&project_path, &env_name, &secret_name, &value)
        .map_err(|e| e.to_string())
}

/// Delete a secret from the OS keychain.
#[tauri::command]
pub fn delete_secret(
    project_path: String,
    env_name: String,
    secret_name: String,
) -> Result<(), String> {
    audit::log_event(
        "delete_secret",
        &format!("project={} env={} secret={}", project_path, env_name, secret_name),
    );
    secrets::delete_secret(&project_path, &env_name, &secret_name).map_err(|e| e.to_string())
}

/// Check which secrets are set in the keychain for a given environment.
#[tauri::command]
pub fn check_secrets_status(
    project_path: String,
    env_name: String,
) -> Result<Vec<secrets::SecretStatus>, String> {
    let secrets_config =
        pipeline::load_secrets_config(Path::new(&project_path)).map_err(|e| e.to_string())?;
    Ok(secrets::check_secrets_status(
        &project_path,
        &env_name,
        &secrets_config,
    ))
}

/// Test SSH connectivity to a host.
#[tauri::command]
pub async fn test_ssh_connection(
    host: String,
    port: Option<u16>,
) -> Result<String, String> {
    preflight::test_ssh_connectivity(&host, port)
        .await
        .map_err(|e| e.to_string())
}

/// Scan a project for env/secret references without writing anything.
#[tauri::command]
pub fn scan_bootstrap(repo_path: String) -> Result<BootstrapReport, String> {
    bootstrap::scan_project(Path::new(&repo_path)).map_err(|e| e.to_string())
}

/// Apply a previously-generated `BootstrapReport` to the project.
/// `merge=true` appends missing names to existing configs; `merge=false` is the
/// "Safe" mode that refuses if either config already exists.
#[tauri::command]
pub fn apply_bootstrap(
    repo_path: String,
    report: BootstrapReport,
    merge: bool,
) -> Result<bool, String> {
    let mode = if merge { ApplyMode::Merge } else { ApplyMode::Safe };
    audit::log_event(
        "apply_bootstrap",
        &format!(
            "project={} mode={:?} detected={}",
            repo_path,
            mode,
            report.detected.len()
        ),
    );
    bootstrap::apply_bootstrap(Path::new(&repo_path), &report, mode).map_err(|e| e.to_string())
}

/// Result of `auto_bootstrap_for_project`.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AutoBootstrapOutcome {
    /// Which behaviour was applied this run.
    pub mode: String,
    /// The scan result. `None` when mode is `off`.
    pub report: Option<BootstrapReport>,
    /// `true` if files were written this call. `false` when in confirm mode
    /// (UI should show review), when mode is off, or when nothing was detected.
    pub applied: bool,
}

/// Honor `AppSettings.bootstrap_mode` and either scan-only (Confirm),
/// scan-and-apply (Silent), or do nothing (Off). Called by the Add Project
/// wizard after `add_project` resolves.
#[tauri::command]
pub fn auto_bootstrap_for_project(repo_path: String) -> Result<AutoBootstrapOutcome, String> {
    use crate::engine::app_settings::{load_app_settings, BootstrapMode};
    let settings = load_app_settings().map_err(|e| e.to_string())?;
    let mode_label = match settings.bootstrap_mode {
        BootstrapMode::Confirm => "confirm",
        BootstrapMode::Silent => "silent",
        BootstrapMode::Off => "off",
    };
    if settings.bootstrap_mode == BootstrapMode::Off {
        return Ok(AutoBootstrapOutcome {
            mode: mode_label.to_string(),
            report: None,
            applied: false,
        });
    }
    let report = bootstrap::scan_project(Path::new(&repo_path)).map_err(|e| e.to_string())?;
    if report.detected.is_empty() {
        return Ok(AutoBootstrapOutcome {
            mode: mode_label.to_string(),
            report: Some(report),
            applied: false,
        });
    }
    let applied = if settings.bootstrap_mode == BootstrapMode::Silent {
        bootstrap::apply_bootstrap(Path::new(&repo_path), &report, ApplyMode::Merge)
            .map_err(|e| e.to_string())?
    } else {
        false
    };
    audit::log_event(
        "auto_bootstrap",
        &format!(
            "project={} mode={} detected={} applied={}",
            repo_path,
            mode_label,
            report.detected.len(),
            applied
        ),
    );
    Ok(AutoBootstrapOutcome {
        mode: mode_label.to_string(),
        report: Some(report),
        applied,
    })
}

/// Run preflight validation for a pipeline against an environment.
#[tauri::command]
pub async fn run_preflight(
    repo_path: String,
    environment: String,
) -> Result<preflight::PreflightResult, String> {
    let path = Path::new(&repo_path);
    let pipe = pipeline::load_pipeline(path).map_err(|e| e.to_string())?;
    let envs = pipeline::load_environments(path).map_err(|e| e.to_string())?;
    let secs = pipeline::load_secrets_config(path).map_err(|e| e.to_string())?;

    preflight::validate_preflight(&pipe, &repo_path, &environment, &envs, &secs)
        .await
        .map_err(|e| e.to_string())
}
