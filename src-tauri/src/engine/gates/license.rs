//! License compliance gate.

#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::engine::models::{
    AuditFinding, AuditResult, CommitLintResult, CommitLintViolation, ContainerFinding,
    ContainerScanResult, GateMode, GatesConfig, GatesResult, IacFinding, IacScanResult,
    LicenseCheckResult, LicenseFinding, SastFinding, SastResult, SecretFinding, SecretScanResult,
    VulnSeverity,
};
use anyhow::{Context, Result};
use std::path::Path;

/// Check dependency licenses across detected languages. Cargo and npm only
/// (Python licensing is harder to enumerate accurately and is left for later).
pub fn run_license_check(repo_path: &Path, config: &GatesConfig) -> Result<LicenseCheckResult> {
    let mut findings: Vec<LicenseFinding> = Vec::new();
    let mut scanners: Vec<&str> = Vec::new();
    let mut missing: Vec<String> = Vec::new();

    // Cargo
    if repo_path.join("Cargo.toml").exists() {
        if command_exists("cargo-license") {
            findings.extend(run_cargo_license(repo_path, config)?);
            scanners.push("cargo-license");
        } else {
            missing.push("cargo-license (install: cargo install cargo-license)".into());
        }
    }

    // npm — detected via the presence of a package-lock or package.json at root
    if repo_path.join("package.json").exists() {
        if command_exists("license-checker") {
            findings.extend(run_license_checker(repo_path, config)?);
            scanners.push("license-checker");
        } else {
            missing.push("license-checker (install: npm i -g license-checker)".into());
        }
    }

    let passed = findings.is_empty();
    let scanner_str = if scanners.is_empty() {
        "(missing)".into()
    } else {
        scanners.join(", ")
    };
    let mut message = if scanners.is_empty() && missing.is_empty() {
        "No Cargo.toml or package.json at repo root — nothing to scan.".into()
    } else {
        format!(
            "{} license violation(s) across {} scanner(s)",
            findings.len(),
            scanners.len()
        )
    };
    if !missing.is_empty() {
        message.push_str(" — missing: ");
        message.push_str(&missing.join("; "));
    }

    Ok(LicenseCheckResult {
        passed,
        findings,
        scanner: scanner_str,
        message,
    })
}

fn run_cargo_license(repo_path: &Path, config: &GatesConfig) -> Result<Vec<LicenseFinding>> {
    let output = std::process::Command::new("cargo")
        .args(["license", "--json"])
        .current_dir(repo_path)
        .output()
        .context("Failed to run cargo license")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap_or(serde_json::Value::Null);
    let arr = v.as_array().cloned().unwrap_or_default();
    let mut out = Vec::new();
    for entry in arr {
        let name = entry
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string();
        if config.license_allowlist.contains(&name) {
            continue;
        }
        let version = entry
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let license = entry
            .get("license")
            .and_then(|l| l.as_str())
            .unwrap_or("")
            .to_string();
        if let Some(reason) = license_finding_reason(&license, &config.license_denylist) {
            out.push(LicenseFinding {
                package: name,
                version,
                license,
                reason,
            });
        }
    }
    Ok(out)
}

fn run_license_checker(repo_path: &Path, config: &GatesConfig) -> Result<Vec<LicenseFinding>> {
    let output = std::process::Command::new("license-checker")
        .args(["--production", "--json"])
        .current_dir(repo_path)
        .output()
        .context("Failed to run license-checker")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap_or(serde_json::Value::Null);
    let Some(map) = v.as_object() else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for (pkg_at_version, entry) in map {
        // package-name@version
        let (name, version) = match pkg_at_version.rsplit_once('@') {
            Some((n, v)) => (n.to_string(), v.to_string()),
            None => (pkg_at_version.clone(), String::new()),
        };
        if config.license_allowlist.contains(&name) {
            continue;
        }
        let license = entry
            .get("licenses")
            .and_then(|l| l.as_str())
            .unwrap_or("")
            .to_string();
        if let Some(reason) = license_finding_reason(&license, &config.license_denylist) {
            out.push(LicenseFinding {
                package: name,
                version,
                license,
                reason,
            });
        }
    }
    Ok(out)
}

fn license_finding_reason(license: &str, denylist: &[String]) -> Option<String> {
    if license.is_empty() || license.eq_ignore_ascii_case("UNKNOWN") {
        return Some("unknown-license".into());
    }
    // Naive match — license fields are usually SPDX ids but sometimes compound
    // like "(MIT OR Apache-2.0)". Substring match is conservative.
    for forbidden in denylist {
        if license.to_lowercase().contains(&forbidden.to_lowercase()) {
            return Some(format!("denylisted: {}", forbidden));
        }
    }
    None
}
