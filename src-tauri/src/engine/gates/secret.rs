//! Secret scanning gate (gitleaks + built-in patterns).

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

/// Built-in secret scanning patterns (used when gitleaks is not installed).
const SECRET_PATTERNS: &[(&str, &str)] = &[
    // AWS
    (r"(?i)AKIA[0-9A-Z]{16}", "aws-access-key-id"),
    (
        r"(?i)aws[_\-]?secret[_\-]?access[_\-]?key\s*[=:]\s*[A-Za-z0-9/+=]{40}",
        "aws-secret-access-key",
    ),
    // Generic API keys / tokens
    (
        r#"(?i)(?:api[_\-]?key|apikey|api[_\-]?token)\s*[=:]\s*['"][A-Za-z0-9\-_.]{16,}['"]"#,
        "generic-api-key",
    ),
    (
        r#"(?i)(?:secret|token|password|passwd|pwd)\s*[=:]\s*['"][^\s'"]{8,}['"]"#,
        "generic-secret",
    ),
    // Private keys
    (
        r"-----BEGIN (?:RSA |EC |DSA |OPENSSH )?PRIVATE KEY-----",
        "private-key",
    ),
    // Database URLs with credentials
    (
        r"(?i)(?:postgres|mysql|mongodb|redis)://[^:\s]+:[^@\s]+@",
        "database-url-with-credentials",
    ),
    // GitHub / GitLab tokens
    (r"(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9_]{36,}", "github-token"),
    (r"glpat-[A-Za-z0-9\-_]{20,}", "gitlab-token"),
    // Slack
    (r"xox[bporas]-[0-9]+-[0-9]+-[A-Za-z0-9]+", "slack-token"),
    // Stripe
    (r"(?:sk|pk)_(?:live|test)_[A-Za-z0-9]{24,}", "stripe-key"),
    // SendGrid
    (
        r"SG\.[A-Za-z0-9\-_]{22}\.[A-Za-z0-9\-_]{43}",
        "sendgrid-api-key",
    ),
    // Twilio
    (r"SK[a-f0-9]{32}", "twilio-api-key"),
    // GCP
    (
        r#"(?i)"type"\s*:\s*"service_account""#,
        "gcp-service-account",
    ),
    // Azure
    (
        r"(?i)(?:azure[_\-]?(?:storage|account)[_\-]?key)\s*[=:]\s*[A-Za-z0-9+/=]{44,}",
        "azure-storage-key",
    ),
    // .env file values that look like secrets
    (
        r"(?i)^[A-Z_]*(?:SECRET|TOKEN|PASSWORD|KEY|CREDENTIAL)[A-Z_]*\s*=\s*\S{8,}$",
        "env-secret-assignment",
    ),
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
        cmd.args(["--config", custom_config.to_str().unwrap_or_default()]);
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
            if allowlist
                .iter()
                .any(|pattern| file.contains(pattern) || rule == *pattern)
            {
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
        .filter_map(|(pattern, rule)| regex::Regex::new(pattern).ok().map(|re| (re, *rule)))
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

        let _ = cmd
            .output()
            .context("Failed to run gitleaks for baseline")?;

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
