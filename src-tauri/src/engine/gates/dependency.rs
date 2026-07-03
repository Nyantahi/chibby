//! Dependency vulnerability audit gate.

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

/// Run dependency audit using the appropriate tool for the project.
pub fn run_dependency_audit(repo_path: &Path, config: &GatesConfig) -> Result<AuditResult> {
    // Detect project type and run the appropriate scanner
    let has_cargo = repo_path.join("Cargo.toml").exists()
        || repo_path.join("src-tauri").join("Cargo.toml").exists();
    let has_npm =
        repo_path.join("package-lock.json").exists() || repo_path.join("package.json").exists();
    let has_pnpm = repo_path.join("pnpm-lock.yaml").exists();
    let has_pip =
        repo_path.join("requirements.txt").exists() || repo_path.join("pyproject.toml").exists();
    let has_go = repo_path.join("go.sum").exists();

    let mut all_findings = Vec::new();
    let mut scanners_used = Vec::new();
    let mut scanners_missing = Vec::new();

    if has_cargo {
        match run_cargo_audit(repo_path, config) {
            Ok(findings) => {
                all_findings.extend(findings);
                scanners_used.push("cargo audit");
            }
            Err(e) => {
                log::warn!("cargo audit unavailable: {e}");
                scanners_missing.push("cargo audit (install: cargo install cargo-audit)");
            }
        }
    }

    if has_npm || has_pnpm {
        let tool = if has_pnpm { "pnpm" } else { "npm" };
        match run_npm_audit(repo_path, tool, config) {
            Ok(findings) => {
                all_findings.extend(findings);
                scanners_used.push(if has_pnpm { "pnpm audit" } else { "npm audit" });
            }
            Err(e) => {
                log::warn!("{tool} audit unavailable: {e}");
                scanners_missing.push(if has_pnpm { "pnpm audit" } else { "npm audit" });
            }
        }
    }

    if has_pip {
        match run_pip_audit(repo_path, config) {
            Ok(findings) => {
                all_findings.extend(findings);
                scanners_used.push("pip-audit");
            }
            Err(e) => {
                log::warn!("pip-audit unavailable: {e}");
                scanners_missing.push("pip-audit (install: pip install pip-audit)");
            }
        }
    }

    if has_go {
        match run_govulncheck(repo_path, config) {
            Ok(findings) => {
                all_findings.extend(findings);
                scanners_used.push("govulncheck");
            }
            Err(e) => {
                log::warn!("govulncheck unavailable: {e}");
                scanners_missing.push(
                    "govulncheck (install: go install golang.org/x/vuln/cmd/govulncheck@latest)",
                );
            }
        }
    }

    // Filter by allowlist
    all_findings.retain(|f| {
        !config
            .audit_allowlist
            .iter()
            .any(|allowed| f.advisory_id == *allowed || f.package == *allowed)
    });

    // Filter by severity threshold
    let threshold = parse_severity(&config.audit_severity_threshold);
    let blocking_findings: Vec<&AuditFinding> = all_findings
        .iter()
        .filter(|f| f.severity >= threshold)
        .collect();

    let passed = blocking_findings.is_empty();
    let scanner_str = if scanners_used.is_empty() {
        "none".to_string()
    } else {
        scanners_used.join(", ")
    };

    let mut message = if passed {
        format!("Dependency audit passed (scanners: {scanner_str}).")
    } else {
        format!(
            "Dependency audit found {} vulnerability(ies) at or above {} severity.",
            blocking_findings.len(),
            config.audit_severity_threshold
        )
    };

    if !scanners_missing.is_empty() {
        message.push_str(&format!(
            " Missing scanners: {}",
            scanners_missing.join(", ")
        ));
    }

    Ok(AuditResult {
        passed,
        findings: all_findings,
        scanner: scanner_str,
        message,
    })
}

/// Run `cargo audit` and parse results.
fn run_cargo_audit(repo_path: &Path, _config: &GatesConfig) -> Result<Vec<AuditFinding>> {
    if !command_exists("cargo-audit") {
        anyhow::bail!("cargo-audit not installed");
    }

    let output = std::process::Command::new("cargo")
        .args(["audit", "--json"])
        .current_dir(repo_path)
        .output()
        .context("Failed to run cargo audit")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_cargo_audit_json(&stdout)
}

fn parse_cargo_audit_json(json_str: &str) -> Result<Vec<AuditFinding>> {
    let parsed: serde_json::Value =
        serde_json::from_str(json_str).context("Failed to parse cargo audit JSON")?;

    let mut findings = Vec::new();

    if let Some(vulns) = parsed
        .get("vulnerabilities")
        .and_then(|v| v.get("list"))
        .and_then(|v| v.as_array())
    {
        for vuln in vulns {
            let advisory = vuln.get("advisory").unwrap_or(vuln);
            let package_info = vuln.get("package");

            let package = package_info
                .and_then(|p| p.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let installed_version = package_info
                .and_then(|p| p.get("version"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let advisory_id = advisory
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let description = advisory
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let fixed = vuln
                .get("versions")
                .and_then(|v| v.get("patched"))
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_str())
                .map(String::from);

            findings.push(AuditFinding {
                package: package.clone(),
                installed_version,
                fixed_version: fixed.clone(),
                advisory_id,
                severity: VulnSeverity::High, // cargo audit doesn't always provide severity
                description,
                upgrade_command: fixed.map(|v| format!("cargo update -p {package}@{v}")),
            });
        }
    }

    Ok(findings)
}

/// Run `npm audit` or `pnpm audit` and parse results.
fn run_npm_audit(repo_path: &Path, tool: &str, _config: &GatesConfig) -> Result<Vec<AuditFinding>> {
    if !command_exists(tool) {
        anyhow::bail!("{tool} not installed");
    }

    let output = std::process::Command::new(tool)
        .args(["audit", "--json"])
        .current_dir(repo_path)
        .output()
        .with_context(|| format!("Failed to run {tool} audit"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_npm_audit_json(&stdout, tool)
}

fn parse_npm_audit_json(json_str: &str, tool: &str) -> Result<Vec<AuditFinding>> {
    let parsed: serde_json::Value =
        serde_json::from_str(json_str).context("Failed to parse npm audit JSON")?;

    let mut findings = Vec::new();

    // npm audit v2 format (advisories under "vulnerabilities")
    if let Some(vulns) = parsed.get("vulnerabilities").and_then(|v| v.as_object()) {
        for (package, info) in vulns {
            let severity_str = info
                .get("severity")
                .and_then(|v| v.as_str())
                .unwrap_or("medium");

            let severity = parse_severity(severity_str);

            let fix_available = info
                .get("fixAvailable")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let via = info
                .get("via")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first());

            let advisory_id = via
                .and_then(|v| v.get("url"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let description = via
                .and_then(|v| v.get("title"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let range = info
                .get("range")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            findings.push(AuditFinding {
                package: package.clone(),
                installed_version: range,
                fixed_version: None,
                advisory_id,
                severity,
                description,
                upgrade_command: if fix_available {
                    Some(format!("{tool} audit fix"))
                } else {
                    None
                },
            });
        }
    }

    Ok(findings)
}

/// Run `pip-audit` and parse results.
fn run_pip_audit(repo_path: &Path, _config: &GatesConfig) -> Result<Vec<AuditFinding>> {
    if !command_exists("pip-audit") {
        anyhow::bail!("pip-audit not installed");
    }

    let output = std::process::Command::new("pip-audit")
        .args(["--format", "json", "--output", "-"])
        .current_dir(repo_path)
        .output()
        .context("Failed to run pip-audit")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_pip_audit_json(&stdout)
}

fn parse_pip_audit_json(json_str: &str) -> Result<Vec<AuditFinding>> {
    let parsed: serde_json::Value =
        serde_json::from_str(json_str).context("Failed to parse pip-audit JSON")?;

    let mut findings = Vec::new();

    if let Some(deps) = parsed.get("dependencies").and_then(|v| v.as_array()) {
        for dep in deps {
            let vulns = match dep.get("vulns").and_then(|v| v.as_array()) {
                Some(v) if !v.is_empty() => v,
                _ => continue,
            };

            let package = dep
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let installed = dep
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            for vuln in vulns {
                let advisory_id = vuln
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                let description = vuln
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let fixed = vuln
                    .get("fix_versions")
                    .and_then(|v| v.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|v| v.as_str())
                    .map(String::from);

                findings.push(AuditFinding {
                    package: package.clone(),
                    installed_version: installed.clone(),
                    fixed_version: fixed.clone(),
                    advisory_id,
                    severity: VulnSeverity::High,
                    description,
                    upgrade_command: fixed.map(|v| format!("pip install {}=={}", package, v)),
                });
            }
        }
    }

    Ok(findings)
}

/// Run `govulncheck` and parse results.
fn run_govulncheck(repo_path: &Path, _config: &GatesConfig) -> Result<Vec<AuditFinding>> {
    if !command_exists("govulncheck") {
        anyhow::bail!("govulncheck not installed");
    }

    let output = std::process::Command::new("govulncheck")
        .args(["-json", "./..."])
        .current_dir(repo_path)
        .output()
        .context("Failed to run govulncheck")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_govulncheck_json(&stdout)
}

fn parse_govulncheck_json(json_str: &str) -> Result<Vec<AuditFinding>> {
    // govulncheck outputs newline-delimited JSON messages
    let mut findings = Vec::new();

    for line in json_str.lines() {
        let parsed: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if let Some(finding) = parsed.get("finding") {
            let osv = finding
                .get("osv")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            // Extract module info from trace
            let trace = finding
                .get("trace")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first());

            let module = trace
                .and_then(|t| t.get("module"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let version = trace
                .and_then(|t| t.get("version"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let fixed = finding
                .get("fixed_version")
                .and_then(|v| v.as_str())
                .map(String::from);

            findings.push(AuditFinding {
                package: module.clone(),
                installed_version: version,
                fixed_version: fixed.clone(),
                advisory_id: osv,
                severity: VulnSeverity::High,
                description: String::new(),
                upgrade_command: fixed.map(|v| format!("go get {}@{}", module, v)),
            });
        }
    }

    Ok(findings)
}

// ---------------------------------------------------------------------------
// Commit message linting
// ---------------------------------------------------------------------------
