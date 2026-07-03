//! Container image scanning gate.

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

/// Scan container images for vulnerabilities. Image refs come from config;
/// when empty, fall back to Dockerfiles detected in the repo (best-effort).
pub fn run_container_scan(repo_path: &Path, config: &GatesConfig) -> Result<ContainerScanResult> {
    if !command_exists("trivy") {
        return Ok(ContainerScanResult {
            passed: true,
            findings: Vec::new(),
            scanner: "(missing)".into(),
            targets: Vec::new(),
            message: "trivy not installed. Install with `brew install trivy`.".into(),
        });
    }

    let mut targets: Vec<String> = config.container_images.clone();
    if targets.is_empty() {
        // Best-effort fallback: list Dockerfiles in the repo (top-level + 1 dir deep).
        targets = discover_dockerfiles(repo_path);
    }

    if targets.is_empty() {
        return Ok(ContainerScanResult {
            passed: true,
            findings: Vec::new(),
            scanner: "trivy".into(),
            targets,
            message: "No images configured and no Dockerfiles detected — nothing to scan.".into(),
        });
    }

    let mut findings = Vec::new();
    for target in &targets {
        // Only scan image refs (target looks like `repo:tag`, not a path).
        // For Dockerfile paths, skip — `trivy image <Dockerfile>` is not the
        // right command; trivy config covers Dockerfile misconfig already.
        if target.starts_with('/') || target.contains("Dockerfile") {
            continue;
        }
        let output = std::process::Command::new("trivy")
            .args([
                "image",
                "--format",
                "json",
                "--severity",
                &severity_filter(&config.container_severity_threshold),
                "--quiet",
                target,
            ])
            .current_dir(repo_path)
            .output()
            .context("Failed to run trivy image")?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        findings.extend(parse_trivy_image_json(&stdout, target));
    }

    let passed = findings.is_empty();
    let message = format!(
        "{} container vulnerability(ies) across {} target(s)",
        findings.len(),
        targets.len()
    );

    Ok(ContainerScanResult {
        passed,
        findings,
        scanner: "trivy".into(),
        targets,
        message,
    })
}

fn discover_dockerfiles(repo_path: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let top = repo_path.join("Dockerfile");
    if top.exists() {
        out.push(top.to_string_lossy().to_string());
    }
    if let Ok(entries) = std::fs::read_dir(repo_path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let candidate = p.join("Dockerfile");
                if candidate.exists() {
                    out.push(candidate.to_string_lossy().to_string());
                }
            }
        }
    }
    out
}

fn parse_trivy_image_json(json_str: &str, target: &str) -> Vec<ContainerFinding> {
    let mut out = Vec::new();
    let v: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return out,
    };
    let Some(results) = v.get("Results").and_then(|r| r.as_array()) else {
        return out;
    };
    for r in results {
        let Some(vulns) = r.get("Vulnerabilities").and_then(|v| v.as_array()) else {
            continue;
        };
        for vuln in vulns {
            let package = vuln
                .get("PkgName")
                .and_then(|p| p.as_str())
                .unwrap_or("")
                .to_string();
            let installed_version = vuln
                .get("InstalledVersion")
                .and_then(|p| p.as_str())
                .unwrap_or("")
                .to_string();
            let fixed_version = vuln
                .get("FixedVersion")
                .and_then(|p| p.as_str())
                .map(|s| s.to_string());
            let advisory_id = vuln
                .get("VulnerabilityID")
                .and_then(|p| p.as_str())
                .unwrap_or("")
                .to_string();
            let severity = vuln
                .get("Severity")
                .and_then(|p| p.as_str())
                .map(parse_severity)
                .unwrap_or(VulnSeverity::Low);
            let description = vuln
                .get("Title")
                .or_else(|| vuln.get("Description"))
                .and_then(|p| p.as_str())
                .unwrap_or("")
                .to_string();
            out.push(ContainerFinding {
                target: target.to_string(),
                package,
                installed_version,
                fixed_version,
                advisory_id,
                severity,
                description,
            });
        }
    }
    out
}

// ----- IaC scan (trivy config) ---------------------------------------------
