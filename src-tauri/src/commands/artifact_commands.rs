use crate::engine::models::{
    ArtifactConfig, ArtifactManifest, CleanupConfig, CleanupResult, SigningConfig, SigningResult,
};
use crate::engine::{artifacts, cleanup, signing};
use std::path::Path;

// ---------------------------------------------------------------------------
// Artifact commands
// ---------------------------------------------------------------------------

/// Load artifact config from .chibby/artifacts.toml.
#[tauri::command]
pub fn load_artifact_config(repo_path: String) -> Result<ArtifactConfig, String> {
    artifacts::load_artifact_config(Path::new(&repo_path)).map_err(|e| e.to_string())
}

/// Save artifact config to .chibby/artifacts.toml.
#[tauri::command]
pub fn save_artifact_config(repo_path: String, config: ArtifactConfig) -> Result<(), String> {
    artifacts::save_artifact_config(Path::new(&repo_path), &config).map_err(|e| e.to_string())
}

/// Collect artifacts matching configured patterns.
#[tauri::command]
pub fn collect_artifacts(
    repo_path: String,
    project_name: String,
    version: String,
) -> Result<ArtifactManifest, String> {
    let config = artifacts::load_artifact_config(Path::new(&repo_path)).map_err(|e| e.to_string())?;
    artifacts::collect_artifacts(Path::new(&repo_path), &config, &project_name, &version)
        .map_err(|e| e.to_string())
}

/// List all artifact manifests for a project.
#[tauri::command]
pub fn list_artifact_manifests(repo_path: String) -> Result<Vec<ArtifactManifest>, String> {
    let config = artifacts::load_artifact_config(Path::new(&repo_path)).map_err(|e| e.to_string())?;
    artifacts::list_artifact_manifests(Path::new(&repo_path), &config).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Signing commands
// ---------------------------------------------------------------------------

/// Load signing config from .chibby/signing.toml.
#[tauri::command]
pub fn load_signing_config(repo_path: String) -> Result<SigningConfig, String> {
    signing::load_signing_config(Path::new(&repo_path)).map_err(|e| e.to_string())
}

/// Save signing config to .chibby/signing.toml.
#[tauri::command]
pub fn save_signing_config(repo_path: String, config: SigningConfig) -> Result<(), String> {
    signing::save_signing_config(Path::new(&repo_path), &config).map_err(|e| e.to_string())
}

/// Sign an artifact file.
#[tauri::command]
pub fn sign_artifact(repo_path: String, artifact_path: String) -> Result<SigningResult, String> {
    let config = signing::load_signing_config(Path::new(&repo_path)).map_err(|e| e.to_string())?;
    signing::sign_artifact(Path::new(&artifact_path), &config).map_err(|e| e.to_string())
}

/// Check whether signing tools are available on this platform.
#[tauri::command]
pub fn check_signing_tools() -> Vec<String> {
    signing::check_signing_tools()
}

// ---------------------------------------------------------------------------
// Cleanup commands
// ---------------------------------------------------------------------------

/// Load cleanup config from .chibby/cleanup.toml.
#[tauri::command]
pub fn load_cleanup_config(repo_path: String) -> Result<CleanupConfig, String> {
    cleanup::load_cleanup_config(Path::new(&repo_path)).map_err(|e| e.to_string())
}

/// Save cleanup config to .chibby/cleanup.toml.
#[tauri::command]
pub fn save_cleanup_config(repo_path: String, config: CleanupConfig) -> Result<(), String> {
    cleanup::save_cleanup_config(Path::new(&repo_path), &config).map_err(|e| e.to_string())
}

/// Run cleanup (artifact pruning + run history pruning).
#[tauri::command]
pub fn run_cleanup(repo_path: String, dry_run: bool) -> Result<CleanupResult, String> {
    let cleanup_config =
        cleanup::load_cleanup_config(Path::new(&repo_path)).map_err(|e| e.to_string())?;
    let artifact_config =
        artifacts::load_artifact_config(Path::new(&repo_path)).map_err(|e| e.to_string())?;
    cleanup::run_cleanup(Path::new(&repo_path), &cleanup_config, &artifact_config, dry_run)
        .map_err(|e| e.to_string())
}
