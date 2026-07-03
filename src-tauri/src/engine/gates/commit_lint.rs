//! Commit message lint gate.

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
    let commit_type = type_re.captures(subject)?.get(1)?.as_str();

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
