use crate::engine::models::{
    AuditFinding, AuditResult, CommitLintResult, CommitLintViolation, GateMode, GatesConfig,
    GatesResult, SecretFinding, SecretScanResult, VulnSeverity,
};
use anyhow::{Context, Result};
use std::path::Path;

// ---------------------------------------------------------------------------
// Config persistence (.chibby/gates.toml)
// ---------------------------------------------------------------------------

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

    // Determine overall pass/fail based on gate modes
    let passed = check_gate_passed(&config.secret_scanning, secret_scan.as_ref().map(|r| r.passed))
        && check_gate_passed(
            &config.dependency_scanning,
            dependency_audit.as_ref().map(|r| r.passed),
        )
        && check_gate_passed(&config.commit_lint, commit_lint.as_ref().map(|r| r.passed));

    Ok(GatesResult {
        passed,
        secret_scan,
        dependency_audit,
        commit_lint,
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

/// Built-in secret scanning patterns (used when gitleaks is not installed).
const SECRET_PATTERNS: &[(&str, &str)] = &[
    // AWS
    (r"(?i)AKIA[0-9A-Z]{16}", "aws-access-key-id"),
    (r"(?i)aws[_\-]?secret[_\-]?access[_\-]?key\s*[=:]\s*[A-Za-z0-9/+=]{40}", "aws-secret-access-key"),
    // Generic API keys / tokens
    (r#"(?i)(?:api[_\-]?key|apikey|api[_\-]?token)\s*[=:]\s*['"][A-Za-z0-9\-_.]{16,}['"]"#, "generic-api-key"),
    (r#"(?i)(?:secret|token|password|passwd|pwd)\s*[=:]\s*['"][^\s'"]{8,}['"]"#, "generic-secret"),
    // Private keys
    (r"-----BEGIN (?:RSA |EC |DSA |OPENSSH )?PRIVATE KEY-----", "private-key"),
    // Database URLs with credentials
    (r"(?i)(?:postgres|mysql|mongodb|redis)://[^:\s]+:[^@\s]+@", "database-url-with-credentials"),
    // GitHub / GitLab tokens
    (r"(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9_]{36,}", "github-token"),
    (r"glpat-[A-Za-z0-9\-_]{20,}", "gitlab-token"),
    // Slack
    (r"xox[bporas]-[0-9]+-[0-9]+-[A-Za-z0-9]+", "slack-token"),
    // Stripe
    (r"(?:sk|pk)_(?:live|test)_[A-Za-z0-9]{24,}", "stripe-key"),
    // SendGrid
    (r"SG\.[A-Za-z0-9\-_]{22}\.[A-Za-z0-9\-_]{43}", "sendgrid-api-key"),
    // Twilio
    (r"SK[a-f0-9]{32}", "twilio-api-key"),
    // GCP
    (r#"(?i)"type"\s*:\s*"service_account""#, "gcp-service-account"),
    // Azure
    (r"(?i)(?:azure[_\-]?(?:storage|account)[_\-]?key)\s*[=:]\s*[A-Za-z0-9+/=]{44,}", "azure-storage-key"),
    // .env file values that look like secrets
    (r"(?i)^[A-Z_]*(?:SECRET|TOKEN|PASSWORD|KEY|CREDENTIAL)[A-Z_]*\s*=\s*\S{8,}$", "env-secret-assignment"),
];

/// Default file patterns to exclude from secret scanning.
const DEFAULT_SCAN_EXCLUDES: &[&str] = &[
    "*.lock",
    "*.sum",
    "*.min.js",
    "*.min.css",
    "*.map",
    "*.wasm",
    "*.png",
    "*.jpg",
    "*.gif",
    "*.ico",
    "*.svg",
    "*.woff",
    "*.woff2",
    "*.ttf",
    "*.eot",
    "node_modules/",
    ".git/",
    "target/",
    "dist/",
    "build/",
    ".chibby/artifacts/",
];

/// Run secret scanning on a repository.
pub fn run_secret_scan(repo_path: &Path, config: &GatesConfig) -> Result<SecretScanResult> {
    // Try gitleaks first
    if command_exists("gitleaks") {
        return run_gitleaks(repo_path, config);
    }

    // Fall back to built-in regex scanner
    log::info!("gitleaks not found, using built-in secret scanner");
    run_builtin_secret_scan(repo_path, config)
}

/// Run gitleaks if available.
fn run_gitleaks(repo_path: &Path, config: &GatesConfig) -> Result<SecretScanResult> {
    let mut cmd = std::process::Command::new("gitleaks");
    cmd.args(["detect", "--source", "."]);
    cmd.args(["--report-format", "json"]);
    cmd.args(["--report-path", "/dev/stdout"]);

    if config.secret_scan_baseline {
        let baseline_path = repo_path.join(".chibby").join("gitleaks-baseline.json");
        if baseline_path.exists() {
            cmd.args([
                "--baseline-path",
                baseline_path.to_str().unwrap_or_default(),
            ]);
        }
    }

    // Custom config if present
    let custom_config = repo_path.join(".chibby").join("gitleaks.toml");
    if custom_config.exists() {
        cmd.args([
            "--config",
            custom_config.to_str().unwrap_or_default(),
        ]);
    }

    cmd.current_dir(repo_path);
    let output = cmd.output().context("Failed to run gitleaks")?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // gitleaks exits with 1 if findings exist, 0 if clean
    let findings = parse_gitleaks_json(&stdout, &config.secret_scan_allowlist);
    let passed = findings.is_empty();

    let message = if passed {
        "Secret scan passed — no secrets detected.".to_string()
    } else {
        format!(
            "Secret scan found {} potential secret(s). Review findings below.",
            findings.len()
        )
    };

    Ok(SecretScanResult {
        passed,
        findings,
        scanner: "gitleaks".to_string(),
        message,
    })
}

/// Parse gitleaks JSON output into findings.
fn parse_gitleaks_json(json_str: &str, allowlist: &[String]) -> Vec<SecretFinding> {
    let parsed: Vec<serde_json::Value> = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    parsed
        .into_iter()
        .filter_map(|entry| {
            let file = entry.get("File")?.as_str()?.to_string();
            let line = entry.get("StartLine")?.as_u64()? as u32;
            let rule = entry
                .get("RuleID")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let secret = entry.get("Secret").and_then(|v| v.as_str()).unwrap_or("");
            let preview = redact_secret(secret);

            // Check allowlist
            if allowlist.iter().any(|pattern| {
                file.contains(pattern) || rule == *pattern
            }) {
                return None;
            }

            Some(SecretFinding {
                file,
                line,
                rule,
                preview,
            })
        })
        .collect()
}

/// Built-in regex-based secret scanner.
fn run_builtin_secret_scan(repo_path: &Path, config: &GatesConfig) -> Result<SecretScanResult> {
    let mut findings = Vec::new();

    // Compile patterns
    let compiled: Vec<(regex::Regex, &str)> = SECRET_PATTERNS
        .iter()
        .filter_map(|(pattern, rule)| {
            regex::Regex::new(pattern).ok().map(|re| (re, *rule))
        })
        .collect();

    // Walk the repo
    scan_directory(repo_path, repo_path, &compiled, config, &mut findings)?;

    let passed = findings.is_empty();
    let message = if passed {
        "Built-in secret scan passed — no secrets detected.".to_string()
    } else {
        format!(
            "Built-in secret scan found {} potential secret(s). Install gitleaks for more accurate scanning.",
            findings.len()
        )
    };

    Ok(SecretScanResult {
        passed,
        findings,
        scanner: "built-in".to_string(),
        message,
    })
}

/// Recursively scan a directory for secrets.
fn scan_directory(
    base_path: &Path,
    dir: &Path,
    patterns: &[(regex::Regex, &str)],
    config: &GatesConfig,
    findings: &mut Vec<SecretFinding>,
) -> Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let rel_path = path
            .strip_prefix(base_path)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        // Check exclusions
        if should_exclude(&rel_path, &config.secret_scan_allowlist) {
            continue;
        }

        if path.is_dir() {
            scan_directory(base_path, &path, patterns, config, findings)?;
        } else if path.is_file() {
            scan_file(base_path, &path, patterns, findings)?;
        }
    }

    Ok(())
}

/// Check if a path should be excluded from scanning.
fn should_exclude(rel_path: &str, allowlist: &[String]) -> bool {
    // Check default excludes
    for exclude in DEFAULT_SCAN_EXCLUDES {
        if exclude.ends_with('/') {
            let dir_name = &exclude[..exclude.len() - 1];
            if rel_path.starts_with(dir_name) || rel_path.contains(&format!("/{dir_name}/")) {
                return true;
            }
        } else if let Some(ext) = exclude.strip_prefix("*.") {
            if rel_path.ends_with(ext) {
                return true;
            }
        }
    }

    // Check user allowlist
    for pattern in allowlist {
        if rel_path.contains(pattern) {
            return true;
        }
    }

    false
}

/// Scan a single file for secret patterns.
fn scan_file(
    base_path: &Path,
    file_path: &Path,
    patterns: &[(regex::Regex, &str)],
    findings: &mut Vec<SecretFinding>,
) -> Result<()> {
    // Skip binary files (simple heuristic: check first 512 bytes)
    let content = match std::fs::read(file_path) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };

    let check_bytes = &content[..content.len().min(512)];
    if check_bytes.iter().any(|&b| b == 0) {
        return Ok(()); // likely binary
    }

    let text = match String::from_utf8(content) {
        Ok(t) => t,
        Err(_) => return Ok(()),
    };

    let rel_path = file_path
        .strip_prefix(base_path)
        .unwrap_or(file_path)
        .to_string_lossy()
        .to_string();

    for (line_num, line) in text.lines().enumerate() {
        for (re, rule) in patterns {
            if re.is_match(line) {
                let preview = redact_secret(line.trim());
                findings.push(SecretFinding {
                    file: rel_path.clone(),
                    line: (line_num + 1) as u32,
                    rule: rule.to_string(),
                    preview,
                });
                break; // one finding per line is enough
            }
        }
    }

    Ok(())
}

/// Redact a secret string, keeping first 4 and last 2 chars visible.
fn redact_secret(s: &str) -> String {
    if s.len() <= 12 {
        return "***REDACTED***".to_string();
    }
    let visible_start = &s[..4];
    let visible_end = &s[s.len() - 2..];
    format!("{}...{}", visible_start, visible_end)
}

/// Create a baseline file from current findings (marks them as acknowledged).
pub fn create_secret_scan_baseline(repo_path: &Path) -> Result<String> {
    if command_exists("gitleaks") {
        let baseline_path = repo_path.join(".chibby").join("gitleaks-baseline.json");
        std::fs::create_dir_all(repo_path.join(".chibby"))?;

        let mut cmd = std::process::Command::new("gitleaks");
        cmd.args(["detect", "--source", "."]);
        cmd.args(["--report-format", "json"]);
        cmd.args(["--report-path", baseline_path.to_str().unwrap_or_default()]);
        cmd.current_dir(repo_path);

        let _ = cmd.output().context("Failed to run gitleaks for baseline")?;

        log::info!("Created gitleaks baseline at {}", baseline_path.display());
        Ok(baseline_path.display().to_string())
    } else {
        anyhow::bail!(
            "Baseline mode requires gitleaks. Install it with: brew install gitleaks (macOS) or see https://github.com/gitleaks/gitleaks"
        )
    }
}

// ---------------------------------------------------------------------------
// Dependency / CVE scanning
// ---------------------------------------------------------------------------

/// Run dependency audit using the appropriate tool for the project.
pub fn run_dependency_audit(repo_path: &Path, config: &GatesConfig) -> Result<AuditResult> {
    // Detect project type and run the appropriate scanner
    let has_cargo = repo_path.join("Cargo.toml").exists()
        || repo_path.join("src-tauri").join("Cargo.toml").exists();
    let has_npm = repo_path.join("package-lock.json").exists()
        || repo_path.join("package.json").exists();
    let has_pnpm = repo_path.join("pnpm-lock.yaml").exists();
    let has_pip = repo_path.join("requirements.txt").exists()
        || repo_path.join("pyproject.toml").exists();
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
                scanners_missing.push(if has_pnpm {
                    "pnpm audit"
                } else {
                    "npm audit"
                });
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
                scanners_missing.push("govulncheck (install: go install golang.org/x/vuln/cmd/govulncheck@latest)");
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

fn parse_severity(s: &str) -> VulnSeverity {
    match s.to_lowercase().as_str() {
        "critical" => VulnSeverity::Critical,
        "high" => VulnSeverity::High,
        "medium" => VulnSeverity::Medium,
        _ => VulnSeverity::Low,
    }
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
fn run_npm_audit(
    repo_path: &Path,
    tool: &str,
    _config: &GatesConfig,
) -> Result<Vec<AuditFinding>> {
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
                    upgrade_command: fixed
                        .map(|v| format!("pip install {}=={}", package, v)),
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

/// Run commit message linting on recent commits.
pub fn run_commit_lint(repo_path: &Path, config: &GatesConfig) -> Result<CommitLintResult> {
    let commits = get_recent_commits(repo_path)?;

    let mut violations = Vec::new();

    for (hash, subject) in &commits {
        if let Some(violation) = lint_commit_message(hash, subject, config) {
            violations.push(violation);
        }
    }

    let passed = violations.is_empty();
    let checked = commits.len() as u32;

    let message = if passed {
        format!("Commit lint passed — all {checked} commit(s) follow Conventional Commits format.")
    } else {
        format!(
            "Commit lint found {} violation(s) in {checked} commit(s). Expected format: <type>(<scope>): <description>",
            violations.len()
        )
    };

    Ok(CommitLintResult {
        passed,
        violations,
        commits_checked: checked,
        message,
    })
}

/// Get commits since last tag (or last 20 commits if no tag).
fn get_recent_commits(repo_path: &Path) -> Result<Vec<(String, String)>> {
    // Try to get the last tag
    let tag_output = std::process::Command::new("git")
        .args(["describe", "--tags", "--abbrev=0"])
        .current_dir(repo_path)
        .output();

    let range = if let Ok(output) = tag_output {
        if output.status.success() {
            let tag = String::from_utf8_lossy(&output.stdout).trim().to_string();
            format!("{tag}..HEAD")
        } else {
            "HEAD~20..HEAD".to_string()
        }
    } else {
        "HEAD~20..HEAD".to_string()
    };

    let output = std::process::Command::new("git")
        .args(["log", &range, "--format=%h|%s", "--no-merges"])
        .current_dir(repo_path)
        .output()
        .context("Failed to run git log")?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    let commits: Vec<(String, String)> = stdout
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let (hash, subject) = line.split_once('|')?;
            Some((hash.to_string(), subject.to_string()))
        })
        .collect();

    Ok(commits)
}

/// Lint a single commit message against Conventional Commits rules.
fn lint_commit_message(
    hash: &str,
    subject: &str,
    config: &GatesConfig,
) -> Option<CommitLintViolation> {
    // Check max subject length
    if subject.len() > config.commit_max_subject_length {
        return Some(CommitLintViolation {
            hash: hash.to_string(),
            subject: subject.to_string(),
            rule: "subject-max-length".to_string(),
            expected: format!(
                "Subject must be {} characters or fewer (currently {}).",
                config.commit_max_subject_length,
                subject.len()
            ),
        });
    }

    // Parse conventional commit format: type(scope): description
    let re = regex::Regex::new(r"^([a-z]+)(\([a-zA-Z0-9_\-./]+\))?!?:\s+.+").ok()?;

    if !re.is_match(subject) {
        return Some(CommitLintViolation {
            hash: hash.to_string(),
            subject: subject.to_string(),
            rule: "conventional-commit-format".to_string(),
            expected: format!(
                "Expected format: <type>(<scope>): <description>. Allowed types: {}",
                config.commit_types.join(", ")
            ),
        });
    }

    // Extract the type
    let type_re = regex::Regex::new(r"^([a-z]+)").ok()?;
    let commit_type = type_re
        .captures(subject)?
        .get(1)?
        .as_str();

    // Check if type is allowed
    if !config.commit_types.iter().any(|t| t == commit_type) {
        return Some(CommitLintViolation {
            hash: hash.to_string(),
            subject: subject.to_string(),
            rule: "type-enum".to_string(),
            expected: format!(
                "Type '{}' is not allowed. Allowed types: {}",
                commit_type,
                config.commit_types.join(", ")
            ),
        });
    }

    // Check if scope is required
    if config.commit_require_scope && !subject.contains('(') {
        return Some(CommitLintViolation {
            hash: hash.to_string(),
            subject: subject.to_string(),
            rule: "scope-required".to_string(),
            expected: "A scope is required. Format: <type>(<scope>): <description>".to_string(),
        });
    }

    None
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn command_exists(cmd: &str) -> bool {
    let check = if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    };
    std::process::Command::new(check)
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
