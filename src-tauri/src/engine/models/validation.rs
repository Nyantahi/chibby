//! Pipeline validation and warning types.

#[allow(unused_imports)]
use super::*;
use serde::{Deserialize, Serialize};

/// Severity level for a pipeline warning.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum WarningSeverity {
    /// May cause issues but could work
    Warning,
    /// Will likely fail
    Error,
}

/// A validation warning or error for a pipeline stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineWarning {
    /// The stage name this warning applies to.
    pub stage_name: String,
    /// The specific command that may fail.
    pub command: String,
    /// Human-readable description of the issue.
    pub message: String,
    /// Suggested fix for the issue.
    pub suggestion: Option<String>,
    /// Severity level.
    pub severity: WarningSeverity,
}

/// Result of validating a pipeline before execution.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PipelineValidation {
    /// List of warnings/errors found.
    pub warnings: Vec<PipelineWarning>,
    /// Detected duplicate or conflicting config files.
    pub file_conflicts: Vec<FileConflict>,
    /// Whether the pipeline is likely to succeed (no errors, only warnings).
    pub is_valid: bool,
}

/// A detected duplicate or conflicting configuration file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileConflict {
    /// Category of the conflict (e.g., "Makefile", "Docker Compose").
    pub category: String,
    /// List of conflicting file names.
    pub files: Vec<String>,
    /// Human-readable description of the issue.
    pub message: String,
    /// Which file will be used (if deterministic).
    pub active_file: Option<String>,
}

// ---------------------------------------------------------------------------
// Phase 5: Versioning, Signing, Artifacts, Notifications, Cleanup
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_warning_severity() {
        let warning: WarningSeverity = serde_json::from_str(r#""warning""#).unwrap();
        let error: WarningSeverity = serde_json::from_str(r#""error""#).unwrap();

        assert_eq!(warning, WarningSeverity::Warning);
        assert_eq!(error, WarningSeverity::Error);
    }
}
