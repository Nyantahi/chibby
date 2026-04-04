use crate::engine::models::{EnvironmentsConfig, SecretsConfig};
use crate::engine::pipeline;
use crate::engine::preflight;
use crate::engine::secrets;
use crate::engine::audit;
use std::path::Path;

/// Load environments config from a project's .chibby/environments.toml.
#[tauri::command]
pub fn load_environments(repo_path: String) -> Result<EnvironmentsConfig, String> {
    pipeline::load_environments(Path::new(&repo_path)).map_err(|e| e.to_string())
}

/// Save environments config to a project's .chibby/environments.toml.
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
