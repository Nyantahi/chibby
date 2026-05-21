use crate::engine::models::{
    AuditResult, CommitLintResult, ContainerScanResult, GatesConfig, GatesResult, IacScanResult,
    LicenseCheckResult, SastResult, SecretScanResult,
};
use crate::engine::gates;
use std::path::Path;
use tokio::task;

// All long-running gates shell out to external scanners (gitleaks / trivy /
// semgrep / npm audit / cargo audit / pip-audit / cargo-license /
// license-checker) and can take 30s to several minutes — especially on first
// run when scanners download policy databases. Running them on Tauri's main
// thread freezes the webview ("OS spinner"). We route every scanner through
// `spawn_blocking` so the GUI stays responsive.
async fn off_main<F, T>(f: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    task::spawn_blocking(f)
        .await
        .map_err(|e| format!("scanner task panicked: {e}"))?
}

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
pub async fn run_gates(repo_path: String) -> Result<GatesResult, String> {
    off_main(move || gates::run_gates(Path::new(&repo_path)).map_err(|e| e.to_string())).await
}

/// Run secret scanning only.
#[tauri::command]
pub async fn run_secret_scan(repo_path: String) -> Result<SecretScanResult, String> {
    off_main(move || {
        let config = gates::load_gates_config(Path::new(&repo_path)).map_err(|e| e.to_string())?;
        gates::run_secret_scan(Path::new(&repo_path), &config).map_err(|e| e.to_string())
    })
    .await
}

/// Run dependency/CVE audit only.
#[tauri::command]
pub async fn run_dependency_audit(repo_path: String) -> Result<AuditResult, String> {
    off_main(move || {
        let config = gates::load_gates_config(Path::new(&repo_path)).map_err(|e| e.to_string())?;
        gates::run_dependency_audit(Path::new(&repo_path), &config).map_err(|e| e.to_string())
    })
    .await
}

/// Run commit message linting only.
#[tauri::command]
pub async fn run_commit_lint(repo_path: String) -> Result<CommitLintResult, String> {
    off_main(move || {
        let config = gates::load_gates_config(Path::new(&repo_path)).map_err(|e| e.to_string())?;
        gates::run_commit_lint(Path::new(&repo_path), &config).map_err(|e| e.to_string())
    })
    .await
}

/// Create a secret scan baseline (marks existing findings as acknowledged).
#[tauri::command]
pub async fn create_secret_scan_baseline(repo_path: String) -> Result<String, String> {
    off_main(move || {
        gates::create_secret_scan_baseline(Path::new(&repo_path)).map_err(|e| e.to_string())
    })
    .await
}

/// Run SAST (static analysis) only — wraps semgrep.
#[tauri::command]
pub async fn run_sast(repo_path: String) -> Result<SastResult, String> {
    off_main(move || {
        let config = gates::load_gates_config(Path::new(&repo_path)).map_err(|e| e.to_string())?;
        gates::run_sast(Path::new(&repo_path), &config).map_err(|e| e.to_string())
    })
    .await
}

/// Run container image scanning only — wraps `trivy image`.
#[tauri::command]
pub async fn run_container_scan(repo_path: String) -> Result<ContainerScanResult, String> {
    off_main(move || {
        let config = gates::load_gates_config(Path::new(&repo_path)).map_err(|e| e.to_string())?;
        gates::run_container_scan(Path::new(&repo_path), &config).map_err(|e| e.to_string())
    })
    .await
}

/// Run IaC scanning only — wraps `trivy config`.
#[tauri::command]
pub async fn run_iac_scan(repo_path: String) -> Result<IacScanResult, String> {
    off_main(move || {
        let config = gates::load_gates_config(Path::new(&repo_path)).map_err(|e| e.to_string())?;
        gates::run_iac_scan(Path::new(&repo_path), &config).map_err(|e| e.to_string())
    })
    .await
}

/// Run license compliance check only.
#[tauri::command]
pub async fn run_license_check(repo_path: String) -> Result<LicenseCheckResult, String> {
    off_main(move || {
        let config = gates::load_gates_config(Path::new(&repo_path)).map_err(|e| e.to_string())?;
        gates::run_license_check(Path::new(&repo_path), &config).map_err(|e| e.to_string())
    })
    .await
}
