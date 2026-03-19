use crate::engine::models::{
    LatestJsonResult, TauriLatestJson, UpdateKeyResult, UpdatePublishResult, UpdateSignResult,
    UpdaterConfig,
};
use crate::engine::{artifacts, updater};
use std::path::Path;

// ---------------------------------------------------------------------------
// Updater config commands
// ---------------------------------------------------------------------------

/// Load updater config from .chibby/updater.toml.
#[tauri::command]
pub fn load_updater_config(repo_path: String) -> Result<UpdaterConfig, String> {
    updater::load_updater_config(Path::new(&repo_path)).map_err(|e| e.to_string())
}

/// Save updater config to .chibby/updater.toml.
#[tauri::command]
pub fn save_updater_config(repo_path: String, config: UpdaterConfig) -> Result<(), String> {
    updater::save_updater_config(Path::new(&repo_path), &config).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Key management commands
// ---------------------------------------------------------------------------

/// Generate a Tauri update key pair. Private key goes to OS keychain.
#[tauri::command]
pub fn generate_update_keys(repo_path: String) -> Result<UpdateKeyResult, String> {
    let result = updater::generate_update_keys(&repo_path).map_err(|e| e.to_string())?;

    // Also save the public key to the updater config
    let mut config =
        updater::load_updater_config(Path::new(&repo_path)).map_err(|e| e.to_string())?;
    config.public_key = Some(result.public_key.clone());
    updater::save_updater_config(Path::new(&repo_path), &config).map_err(|e| e.to_string())?;

    Ok(result)
}

/// Import an existing Tauri update private key into the OS keychain.
#[tauri::command]
pub fn import_update_private_key(
    repo_path: String,
    private_key: String,
) -> Result<(), String> {
    updater::set_update_private_key(&repo_path, &private_key).map_err(|e| e.to_string())
}

/// Check if the Tauri update private key exists in the OS keychain.
#[tauri::command]
pub fn has_update_key(repo_path: String) -> bool {
    updater::has_update_private_key(&repo_path)
}

/// Rotate the update key pair (generate new, update config).
#[tauri::command]
pub fn rotate_update_keys(repo_path: String) -> Result<UpdateKeyResult, String> {
    updater::rotate_update_keys(Path::new(&repo_path), &repo_path).map_err(|e| e.to_string())
}

/// Delete the update private key from the OS keychain.
#[tauri::command]
pub fn delete_update_key(repo_path: String) -> Result<(), String> {
    updater::delete_update_private_key(&repo_path).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Preflight command
// ---------------------------------------------------------------------------

/// Run updater preflight checks.
#[tauri::command]
pub fn updater_preflight(repo_path: String) -> Vec<String> {
    updater::updater_preflight(Path::new(&repo_path), &repo_path)
}

// ---------------------------------------------------------------------------
// Signing command
// ---------------------------------------------------------------------------

/// Sign an update bundle with the Tauri update key.
#[tauri::command]
pub fn sign_update_bundle(
    repo_path: String,
    file_path: String,
) -> Result<UpdateSignResult, String> {
    updater::sign_update_bundle(Path::new(&file_path), &repo_path).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// latest.json commands
// ---------------------------------------------------------------------------

/// Generate a Tauri-compatible latest.json from the latest artifact manifest.
#[tauri::command]
pub fn generate_latest_json(
    repo_path: String,
    version: String,
    notes: Option<String>,
) -> Result<LatestJsonResult, String> {
    let artifact_config =
        artifacts::load_artifact_config(Path::new(&repo_path)).map_err(|e| e.to_string())?;

    // Find the manifest for the requested version
    let manifests =
        artifacts::list_artifact_manifests(Path::new(&repo_path), &artifact_config)
            .map_err(|e| e.to_string())?;

    let manifest = manifests
        .into_iter()
        .find(|m| m.version == version)
        .ok_or_else(|| format!("No artifact manifest found for version {version}"))?;

    updater::generate_latest_json(Path::new(&repo_path), &repo_path, &manifest, notes)
        .map_err(|e| e.to_string())
}

/// Merge a per-platform latest.json fragment into an existing latest.json.
#[tauri::command]
pub fn merge_latest_json(
    target_path: String,
    fragment: TauriLatestJson,
) -> Result<TauriLatestJson, String> {
    updater::merge_latest_json(Path::new(&target_path), &fragment).map_err(|e| e.to_string())
}

/// Check if the Tauri CLI is available.
#[tauri::command]
pub fn check_tauri_cli() -> Result<String, String> {
    let cli = updater::check_tauri_cli().map_err(|e| e.to_string())?;
    Ok(cli.join(" "))
}

// ---------------------------------------------------------------------------
// Publish commands
// ---------------------------------------------------------------------------

/// Publish update artifacts and latest.json to the configured target.
#[tauri::command]
pub fn publish_update(
    repo_path: String,
    version: String,
    dry_run: bool,
) -> Result<UpdatePublishResult, String> {
    updater::publish_update(Path::new(&repo_path), &version, dry_run).map_err(|e| e.to_string())
}
