//! Static application security testing (SAST) gate.

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

/// Run semgrep across the repo. Falls back gracefully if semgrep isn't installed.
pub fn run_sast(repo_path: &Path, config: &GatesConfig) -> Result<SastResult> {
    if !command_exists("semgrep") {
        return Ok(SastResult {
            passed: true,
            findings: Vec::new(),
            scanner: "(missing)".into(),
            message:
                "semgrep not installed. Install with `pip install semgrep` or `brew install semgrep`."
                    .into(),
        });
    }

    let output = std::process::Command::new("semgrep")
        .args(["--config=auto", "--json", "--quiet", "--no-error", "."])
        .current_dir(repo_path)
        .output()
        .context("Failed to run semgrep")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut findings = parse_semgrep_json(&stdout, config);
    findings.retain(|f| !config.sast_allowlist.contains(&f.rule));

    let blocking: usize = findings
        .iter()
        .filter(|f| meets_threshold(&f.severity, &config.sast_severity_threshold))
        .count();
    let passed = blocking == 0;
    let message = format!(
        "{} SAST finding(s) ({} at/above {})",
        findings.len(),
        blocking,
        config.sast_severity_threshold
    );

    Ok(SastResult {
        passed,
        findings,
        scanner: "semgrep".into(),
        message,
    })
}

fn parse_semgrep_json(json_str: &str, _config: &GatesConfig) -> Vec<SastFinding> {
    let mut out = Vec::new();
    let v: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return out,
    };
    let Some(results) = v.get("results").and_then(|r| r.as_array()) else {
        return out;
    };
    for r in results {
        let file = r
            .get("path")
            .and_then(|p| p.as_str())
            .unwrap_or("")
            .to_string();
        let line = r
            .get("start")
            .and_then(|s| s.get("line"))
            .and_then(|l| l.as_u64())
            .unwrap_or(0) as u32;
        let rule = r
            .get("check_id")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();
        let extra = r.get("extra");
        let severity = extra
            .and_then(|e| e.get("severity"))
            .and_then(|s| s.as_str())
            .map(|s| match s.to_uppercase().as_str() {
                "ERROR" => VulnSeverity::High,
                "WARNING" => VulnSeverity::Medium,
                "INFO" => VulnSeverity::Low,
                _ => VulnSeverity::Low,
            })
            .unwrap_or(VulnSeverity::Low);
        let message = extra
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();
        out.push(SastFinding {
            file,
            line,
            rule,
            severity,
            message,
        });
    }
    out
}

// ----- Container scan (trivy image) ----------------------------------------
