use crate::engine::models::{
    AuditResult, CommitLintResult, GatesConfig, GatesResult, SecretScanResult,
};
use crate::engine::gates;
use std::path::Path;

// ---------------------------------------------------------------------------
// Gates config commands
// ---------------------------------------------------------------------------

/// Load gates config from .chibby/gates.toml.
#[tauri::command]
pub fn load_gates_config(repo_path: String) -> Result<GatesConfig, String> {
    gates::load_gates_config(Path::new(&repo_path)).map_err(|e| e.to_string())
}

/// Save gates config to .chibby/gates.toml.
#[tauri::command]
pub fn save_gates_config(repo_path: String, config: GatesConfig) -> Result<(), String> {
    gates::save_gates_config(Path::new(&repo_path), &config).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Run gates
// ---------------------------------------------------------------------------

/// Run all enabled security and quality gates.
#[tauri::command]
pub fn run_gates(repo_path: String) -> Result<GatesResult, String> {
    gates::run_gates(Path::new(&repo_path)).map_err(|e| e.to_string())
}

/// Run secret scanning only.
#[tauri::command]
pub fn run_secret_scan(repo_path: String) -> Result<SecretScanResult, String> {
    let config = gates::load_gates_config(Path::new(&repo_path)).map_err(|e| e.to_string())?;
    gates::run_secret_scan(Path::new(&repo_path), &config).map_err(|e| e.to_string())
}

/// Run dependency/CVE audit only.
#[tauri::command]
pub fn run_dependency_audit(repo_path: String) -> Result<AuditResult, String> {
    let config = gates::load_gates_config(Path::new(&repo_path)).map_err(|e| e.to_string())?;
    gates::run_dependency_audit(Path::new(&repo_path), &config).map_err(|e| e.to_string())
}

/// Run commit message linting only.
#[tauri::command]
pub fn run_commit_lint(repo_path: String) -> Result<CommitLintResult, String> {
    let config = gates::load_gates_config(Path::new(&repo_path)).map_err(|e| e.to_string())?;
    gates::run_commit_lint(Path::new(&repo_path), &config).map_err(|e| e.to_string())
}

/// Create a secret scan baseline (marks existing findings as acknowledged).
#[tauri::command]
pub fn create_secret_scan_baseline(repo_path: String) -> Result<String, String> {
    gates::create_secret_scan_baseline(Path::new(&repo_path)).map_err(|e| e.to_string())
}
