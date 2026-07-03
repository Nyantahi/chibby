//! Security gate configuration, findings, and result types.

#[allow(unused_imports)]
use super::*;
use serde::{Deserialize, Serialize};

/// Enforcement mode for a security/quality gate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum GateMode {
    /// Block the pipeline on findings.
    Block,
    /// Report findings but continue.
    Warn,
    /// Disable the gate entirely.
    Off,
}

impl Default for GateMode {
    fn default() -> Self {
        Self::Off
    }
}

/// Top-level security and quality gates config (stored in .chibby/gates.toml).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatesConfig {
    /// Secret scanning mode.
    #[serde(default)]
    pub secret_scanning: GateMode,

    /// Dependency/CVE scanning mode.
    #[serde(default)]
    pub dependency_scanning: GateMode,

    /// Commit message linting mode.
    #[serde(default)]
    pub commit_lint: GateMode,

    /// SAST (semgrep) mode.
    #[serde(default)]
    pub sast: GateMode,

    /// Container image scanning (trivy image) mode.
    #[serde(default)]
    pub container_scan: GateMode,

    /// Infrastructure-as-Code scanning (trivy config) mode.
    #[serde(default)]
    pub iac_scan: GateMode,

    /// License compliance mode.
    #[serde(default)]
    pub license_check: GateMode,

    /// Paths to exclude from secret scanning (glob patterns).
    #[serde(default)]
    pub secret_scan_allowlist: Vec<String>,

    /// CVE IDs or package names to ignore in dependency scanning.
    #[serde(default)]
    pub audit_allowlist: Vec<String>,

    /// Severity threshold for dependency scanning: block on this level and above.
    /// One of "critical", "high", "medium", "low". Default: "high".
    #[serde(default = "default_severity_threshold")]
    pub audit_severity_threshold: String,

    /// Whether to use baseline mode for secret scanning (ignore existing findings).
    #[serde(default)]
    pub secret_scan_baseline: bool,

    /// SAST severity threshold: block on this level and above.
    #[serde(default = "default_severity_threshold")]
    pub sast_severity_threshold: String,

    /// SAST rule IDs to ignore (e.g. `python.lang.security.audit.dangerous-subprocess-use`).
    #[serde(default)]
    pub sast_allowlist: Vec<String>,

    /// Container scan severity threshold.
    #[serde(default = "default_severity_threshold")]
    pub container_severity_threshold: String,

    /// Explicit image refs to scan (e.g. `ghcr.io/org/app:tag`).
    /// When empty, the scanner falls back to detected Dockerfiles in the repo.
    #[serde(default)]
    pub container_images: Vec<String>,

    /// IaC scan severity threshold.
    #[serde(default = "default_severity_threshold")]
    pub iac_severity_threshold: String,

    /// SPDX license identifiers that are forbidden (e.g. `GPL-3.0`, `AGPL-3.0`).
    /// Default forbids the most viral copyleft licenses.
    #[serde(default = "default_license_denylist")]
    pub license_denylist: Vec<String>,

    /// Package names exempt from license enforcement (escape hatch).
    #[serde(default)]
    pub license_allowlist: Vec<String>,

    /// Commit lint: allowed commit types.
    #[serde(default = "default_commit_types")]
    pub commit_types: Vec<String>,

    /// Commit lint: max subject line length.
    #[serde(default = "default_max_subject_len")]
    pub commit_max_subject_length: usize,

    /// Commit lint: require a scope.
    #[serde(default)]
    pub commit_require_scope: bool,
}

fn default_license_denylist() -> Vec<String> {
    vec![
        "GPL-3.0".into(),
        "GPL-2.0".into(),
        "AGPL-3.0".into(),
        "AGPL-1.0".into(),
    ]
}

fn default_severity_threshold() -> String {
    "high".to_string()
}

fn default_commit_types() -> Vec<String> {
    vec![
        "feat".into(),
        "fix".into(),
        "docs".into(),
        "style".into(),
        "refactor".into(),
        "perf".into(),
        "test".into(),
        "build".into(),
        "ci".into(),
        "chore".into(),
    ]
}

fn default_max_subject_len() -> usize {
    72
}

impl Default for GatesConfig {
    fn default() -> Self {
        Self {
            secret_scanning: GateMode::Off,
            dependency_scanning: GateMode::Off,
            commit_lint: GateMode::Off,
            sast: GateMode::Off,
            container_scan: GateMode::Off,
            iac_scan: GateMode::Off,
            license_check: GateMode::Off,
            secret_scan_allowlist: Vec::new(),
            audit_allowlist: Vec::new(),
            audit_severity_threshold: default_severity_threshold(),
            secret_scan_baseline: false,
            sast_severity_threshold: default_severity_threshold(),
            sast_allowlist: Vec::new(),
            container_severity_threshold: default_severity_threshold(),
            container_images: Vec::new(),
            iac_severity_threshold: default_severity_threshold(),
            license_denylist: default_license_denylist(),
            license_allowlist: Vec::new(),
            commit_types: default_commit_types(),
            commit_max_subject_length: default_max_subject_len(),
            commit_require_scope: false,
        }
    }
}

/// A single secret scanning finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretFinding {
    /// File path relative to repo root.
    pub file: String,
    /// Line number where the secret was found.
    pub line: u32,
    /// Rule that matched (e.g. "aws-access-key", "generic-api-key").
    pub rule: String,
    /// Redacted preview of the match.
    pub preview: String,
}

/// Result of running secret scanning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretScanResult {
    /// Whether the scan passed (no blocking findings).
    pub passed: bool,
    /// Findings from the scan.
    pub findings: Vec<SecretFinding>,
    /// Whether gitleaks was used (vs built-in scanner).
    pub scanner: String,
    /// Human-readable summary.
    pub message: String,
}

/// Severity level for a dependency vulnerability.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
#[serde(rename_all = "lowercase")]
pub enum VulnSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// A single dependency audit finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditFinding {
    /// Package name.
    pub package: String,
    /// Installed version.
    pub installed_version: String,
    /// Fixed version (if available).
    pub fixed_version: Option<String>,
    /// CVE or advisory identifier.
    pub advisory_id: String,
    /// Severity level.
    pub severity: VulnSeverity,
    /// Short description.
    pub description: String,
    /// Suggested upgrade command (if applicable).
    pub upgrade_command: Option<String>,
}

/// Result of running dependency/CVE scanning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditResult {
    /// Whether the scan passed (no findings at or above threshold).
    pub passed: bool,
    /// Findings from the scan.
    pub findings: Vec<AuditFinding>,
    /// Which scanner was used (e.g. "cargo audit", "npm audit").
    pub scanner: String,
    /// Human-readable summary.
    pub message: String,
}

/// A single commit lint violation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitLintViolation {
    /// Git commit hash (short).
    pub hash: String,
    /// The original commit message subject.
    pub subject: String,
    /// What rule was violated.
    pub rule: String,
    /// Explanation of the expected format.
    pub expected: String,
}

/// Result of running commit message linting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitLintResult {
    /// Whether all commits passed.
    pub passed: bool,
    /// Violations found.
    pub violations: Vec<CommitLintViolation>,
    /// Total commits checked.
    pub commits_checked: u32,
    /// Human-readable summary.
    pub message: String,
}

/// A single SAST finding (semgrep / generic linter).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SastFinding {
    /// File path relative to repo root.
    pub file: String,
    /// 1-based line number.
    pub line: u32,
    /// Rule identifier (e.g. `python.lang.security.audit.dangerous-subprocess-use`).
    pub rule: String,
    /// Severity bucket.
    pub severity: VulnSeverity,
    /// Short description / message from the scanner.
    pub message: String,
}

/// Result of running SAST.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SastResult {
    pub passed: bool,
    pub findings: Vec<SastFinding>,
    /// Scanner used (e.g. "semgrep").
    pub scanner: String,
    pub message: String,
}

/// A single container scan finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerFinding {
    /// Image scanned (e.g. `ghcr.io/org/app:tag` or `Dockerfile`).
    pub target: String,
    /// Package name (OS package or app dependency).
    pub package: String,
    pub installed_version: String,
    pub fixed_version: Option<String>,
    pub advisory_id: String,
    pub severity: VulnSeverity,
    pub description: String,
}

/// Result of container scanning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerScanResult {
    pub passed: bool,
    pub findings: Vec<ContainerFinding>,
    pub scanner: String,
    /// What was scanned (image refs, or detected Dockerfiles).
    pub targets: Vec<String>,
    pub message: String,
}

/// A single IaC misconfiguration finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IacFinding {
    pub file: String,
    pub line: Option<u32>,
    /// Rule / check ID (e.g. `DS002`, `AVD-DS-0002`).
    pub rule: String,
    pub severity: VulnSeverity,
    pub message: String,
    /// Suggested remediation, if the scanner provided one.
    pub resolution: Option<String>,
}

/// Result of IaC scanning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IacScanResult {
    pub passed: bool,
    pub findings: Vec<IacFinding>,
    pub scanner: String,
    pub message: String,
}

/// A single license-compliance finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseFinding {
    pub package: String,
    pub version: String,
    /// SPDX identifier or raw license string from the manifest.
    pub license: String,
    /// Reason this was flagged ("denylisted", "unknown-license", etc.).
    pub reason: String,
}

/// Result of license-compliance check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseCheckResult {
    pub passed: bool,
    pub findings: Vec<LicenseFinding>,
    pub scanner: String,
    pub message: String,
}

/// Combined result of running all enabled gates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatesResult {
    /// Whether all gates passed.
    pub passed: bool,
    /// Secret scanning result (None if gate is off).
    pub secret_scan: Option<SecretScanResult>,
    /// Dependency audit result (None if gate is off).
    pub dependency_audit: Option<AuditResult>,
    /// Commit lint result (None if gate is off).
    pub commit_lint: Option<CommitLintResult>,
    /// SAST result (None if gate is off).
    #[serde(default)]
    pub sast: Option<SastResult>,
    /// Container scan result (None if gate is off).
    #[serde(default)]
    pub container_scan: Option<ContainerScanResult>,
    /// IaC scan result (None if gate is off).
    #[serde(default)]
    pub iac_scan: Option<IacScanResult>,
    /// License check result (None if gate is off).
    #[serde(default)]
    pub license_check: Option<LicenseCheckResult>,
}

// ---------------------------------------------------------------------------
// Unit Tests
// ---------------------------------------------------------------------------
