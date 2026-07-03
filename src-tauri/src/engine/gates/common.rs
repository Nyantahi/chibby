//! Shared helpers used across gate runners.

#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::engine::models::{
    AuditFinding, AuditResult, CommitLintResult, CommitLintViolation, ContainerFinding,
    ContainerScanResult, GateMode, GatesConfig, GatesResult, IacFinding, IacScanResult,
    LicenseCheckResult, LicenseFinding, SastFinding, SastResult, SecretFinding, SecretScanResult,
    VulnSeverity,
};

pub(crate) fn parse_severity(s: &str) -> VulnSeverity {
    match s.to_lowercase().as_str() {
        "critical" => VulnSeverity::Critical,
        "high" => VulnSeverity::High,
        "medium" => VulnSeverity::Medium,
        _ => VulnSeverity::Low,
    }
}

pub(crate) fn command_exists(cmd: &str) -> bool {
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

// ===========================================================================
// Phase 2 gates: SAST, Container scan, IaC scan, License check
// ===========================================================================
//
// Each gate wraps an external tool. If the tool isn't installed, the gate
// returns a non-failing result with `scanner: "(missing)"` and a message
// pointing at the install command — same friendly contract as the existing
// dependency_audit module.

pub(crate) fn meets_threshold(finding: &VulnSeverity, threshold: &str) -> bool {
    let t = parse_severity(threshold);
    finding >= &t
}

// ----- SAST (semgrep) ------------------------------------------------------

pub(crate) fn severity_filter(threshold: &str) -> String {
    match parse_severity(threshold) {
        VulnSeverity::Critical => "CRITICAL".into(),
        VulnSeverity::High => "HIGH,CRITICAL".into(),
        VulnSeverity::Medium => "MEDIUM,HIGH,CRITICAL".into(),
        VulnSeverity::Low => "LOW,MEDIUM,HIGH,CRITICAL".into(),
    }
}
