//! Security gates: configuration, dispatch, and per-gate runners.
//!
//! Split into per-gate submodules; runner entry points are re-exported so
//! `crate::engine::gates::*` paths remain stable.

#[allow(unused_imports)]
use crate::engine::models::{
    AuditFinding, AuditResult, CommitLintResult, CommitLintViolation, ContainerFinding,
    ContainerScanResult, GateMode, GatesConfig, GatesResult, IacFinding, IacScanResult,
    LicenseCheckResult, LicenseFinding, SastFinding, SastResult, SecretFinding, SecretScanResult,
    VulnSeverity,
};
use anyhow::{Context, Result};
use std::path::Path;

mod commit_lint;
mod common;
mod container;
mod dependency;
mod iac;
mod license;
mod sast;
mod secret;

pub use commit_lint::*;
pub(crate) use common::*;
pub use container::*;
pub use dependency::*;
pub use iac::*;
pub use license::*;
pub use sast::*;
pub use secret::*;

/// Save gates config to .chibby/gates.toml.
pub fn save_gates_config(repo_path: &Path, config: &GatesConfig) -> Result<()> {
    let chibby_dir = repo_path.join(".chibby");
    std::fs::create_dir_all(&chibby_dir)?;

    let toml_str = toml::to_string_pretty(config).context("Failed to serialize gates config")?;

    let file_path = chibby_dir.join("gates.toml");
    std::fs::write(&file_path, &toml_str)?;

    log::info!("Saved gates config to {}", file_path.display());
    Ok(())
}

/// Load gates config from .chibby/gates.toml.
pub fn load_gates_config(repo_path: &Path) -> Result<GatesConfig> {
    let file_path = repo_path.join(".chibby").join("gates.toml");
    if !file_path.exists() {
        return Ok(GatesConfig::default());
    }
    let content = std::fs::read_to_string(&file_path)
        .with_context(|| format!("Failed to read {}", file_path.display()))?;

    let config: GatesConfig = toml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", file_path.display()))?;

    Ok(config)
}

// ---------------------------------------------------------------------------
// Run all enabled gates
// ---------------------------------------------------------------------------

/// Run all enabled security and quality gates.
pub fn run_gates(repo_path: &Path) -> Result<GatesResult> {
    let config = load_gates_config(repo_path)?;

    let secret_scan = if config.secret_scanning != GateMode::Off {
        Some(run_secret_scan(repo_path, &config)?)
    } else {
        None
    };

    let dependency_audit = if config.dependency_scanning != GateMode::Off {
        Some(run_dependency_audit(repo_path, &config)?)
    } else {
        None
    };

    let commit_lint = if config.commit_lint != GateMode::Off {
        Some(run_commit_lint(repo_path, &config)?)
    } else {
        None
    };

    let sast = if config.sast != GateMode::Off {
        Some(run_sast(repo_path, &config)?)
    } else {
        None
    };

    let container_scan = if config.container_scan != GateMode::Off {
        Some(run_container_scan(repo_path, &config)?)
    } else {
        None
    };

    let iac_scan = if config.iac_scan != GateMode::Off {
        Some(run_iac_scan(repo_path, &config)?)
    } else {
        None
    };

    let license_check = if config.license_check != GateMode::Off {
        Some(run_license_check(repo_path, &config)?)
    } else {
        None
    };

    // Determine overall pass/fail based on gate modes
    let passed =
        check_gate_passed(
            &config.secret_scanning,
            secret_scan.as_ref().map(|r| r.passed),
        ) && check_gate_passed(
            &config.dependency_scanning,
            dependency_audit.as_ref().map(|r| r.passed),
        ) && check_gate_passed(&config.commit_lint, commit_lint.as_ref().map(|r| r.passed))
            && check_gate_passed(&config.sast, sast.as_ref().map(|r| r.passed))
            && check_gate_passed(
                &config.container_scan,
                container_scan.as_ref().map(|r| r.passed),
            )
            && check_gate_passed(&config.iac_scan, iac_scan.as_ref().map(|r| r.passed))
            && check_gate_passed(
                &config.license_check,
                license_check.as_ref().map(|r| r.passed),
            );

    Ok(GatesResult {
        passed,
        secret_scan,
        dependency_audit,
        commit_lint,
        sast,
        container_scan,
        iac_scan,
        license_check,
    })
}

/// Check if a gate passed given its mode and scan result.
fn check_gate_passed(mode: &GateMode, scan_passed: Option<bool>) -> bool {
    match (mode, scan_passed) {
        (GateMode::Block, Some(false)) => false,
        _ => true,
    }
}

// ---------------------------------------------------------------------------
// Secret scanning
// ---------------------------------------------------------------------------
