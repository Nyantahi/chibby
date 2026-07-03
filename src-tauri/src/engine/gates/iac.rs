//! Infrastructure-as-code scanning gate.

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

/// Scan Dockerfile/Compose/K8s/Terraform for misconfigurations.
pub fn run_iac_scan(repo_path: &Path, config: &GatesConfig) -> Result<IacScanResult> {
    if !command_exists("trivy") {
        return Ok(IacScanResult {
            passed: true,
            findings: Vec::new(),
            scanner: "(missing)".into(),
            message: "trivy not installed. Install with `brew install trivy`.".into(),
        });
    }

    let output = std::process::Command::new("trivy")
        .args([
            "config",
            "--format",
            "json",
            "--severity",
            &severity_filter(&config.iac_severity_threshold),
            "--quiet",
            ".",
        ])
        .current_dir(repo_path)
        .output()
        .context("Failed to run trivy config")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let findings = parse_trivy_config_json(&stdout);
    let passed = findings.is_empty();
    let message = format!("{} IaC misconfiguration(s)", findings.len());

    Ok(IacScanResult {
        passed,
        findings,
        scanner: "trivy".into(),
        message,
    })
}

fn parse_trivy_config_json(json_str: &str) -> Vec<IacFinding> {
    let mut out = Vec::new();
    let v: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return out,
    };
    let Some(results) = v.get("Results").and_then(|r| r.as_array()) else {
        return out;
    };
    for r in results {
        let file = r
            .get("Target")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
        let Some(misconfigs) = r.get("Misconfigurations").and_then(|m| m.as_array()) else {
            continue;
        };
        for m in misconfigs {
            let line = m
                .get("CauseMetadata")
                .and_then(|c| c.get("StartLine"))
                .and_then(|l| l.as_u64())
                .map(|l| l as u32);
            let rule = m
                .get("ID")
                .or_else(|| m.get("AVDID"))
                .and_then(|i| i.as_str())
                .unwrap_or("")
                .to_string();
            let severity = m
                .get("Severity")
                .and_then(|s| s.as_str())
                .map(parse_severity)
                .unwrap_or(VulnSeverity::Low);
            let message = m
                .get("Title")
                .or_else(|| m.get("Description"))
                .and_then(|m| m.as_str())
                .unwrap_or("")
                .to_string();
            let resolution = m
                .get("Resolution")
                .and_then(|r| r.as_str())
                .map(|s| s.to_string());
            out.push(IacFinding {
                file: file.clone(),
                line,
                rule,
                severity,
                message,
                resolution,
            });
        }
    }
    out
}

// ----- License check (cargo-license + license-checker) ---------------------
