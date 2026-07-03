//! Pipeline run and stage result types.

#[allow(unused_imports)]
use super::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Status of a single stage execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StageStatus {
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
}

/// Result of executing one stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageResult {
    pub stage_name: String,
    pub status: StageStatus,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<u64>,
    /// Whether the post-stage health check passed (None if no health check configured).
    #[serde(default)]
    pub health_check_passed: Option<bool>,
}

/// Overall run status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    Pending,
    Running,
    Success,
    Failed,
    Cancelled,
}

/// The kind of run (normal, retry, or rollback).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RunKind {
    Normal,
    Retry,
    Rollback,
}

impl Default for RunKind {
    fn default() -> Self {
        Self::Normal
    }
}

/// A single pipeline run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineRun {
    pub id: String,
    pub pipeline_name: String,
    pub repo_path: String,
    pub environment: Option<String>,
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub status: RunStatus,
    pub stage_results: Vec<StageResult>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<u64>,
    /// The exact pipeline definition executed for this run.
    #[serde(default)]
    pub pipeline_snapshot: Option<Pipeline>,
    /// The source pipeline file name used for this run (`pipeline` by default).
    #[serde(default)]
    pub pipeline_file: Option<String>,
    /// What kind of run this is (normal, retry, or rollback).
    #[serde(default)]
    pub run_kind: RunKind,
    /// If this is a retry, the ID of the original run.
    #[serde(default)]
    pub parent_run_id: Option<String>,
    /// If this is a retry, which attempt number (1-based).
    #[serde(default)]
    pub retry_number: Option<u32>,
    /// If this is a rollback, the ID of the run being rolled back to.
    #[serde(default)]
    pub rollback_target_id: Option<String>,
    /// The stage name where retry started from (stages before this were skipped).
    #[serde(default)]
    pub retry_from_stage: Option<String>,
}

impl PipelineRun {
    /// Create a new pending run with a freshly generated id.
    pub fn new(pipeline_name: &str, repo_path: &str, environment: Option<String>) -> Self {
        Self::new_with_id(
            &Uuid::new_v4().to_string(),
            pipeline_name,
            repo_path,
            environment,
        )
    }

    /// Create a new pending run with a caller-supplied id.
    ///
    /// Lets the command layer know the run id up front so log events can be
    /// tagged with it before execution starts.
    pub fn new_with_id(
        id: &str,
        pipeline_name: &str,
        repo_path: &str,
        environment: Option<String>,
    ) -> Self {
        Self {
            id: id.to_string(),
            pipeline_name: pipeline_name.to_string(),
            repo_path: repo_path.to_string(),
            environment,
            branch: None,
            commit: None,
            status: RunStatus::Pending,
            stage_results: Vec::new(),
            started_at: Utc::now(),
            finished_at: None,
            duration_ms: None,
            pipeline_snapshot: None,
            pipeline_file: None,
            run_kind: RunKind::Normal,
            parent_run_id: None,
            retry_number: None,
            rollback_target_id: None,
            retry_from_stage: None,
        }
    }
}

/// Summary of a deployment to a specific environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentRecord {
    /// The run that produced this deployment.
    pub run_id: String,
    /// Pipeline name.
    pub pipeline_name: String,
    /// Environment deployed to.
    pub environment: String,
    /// Run status.
    pub status: RunStatus,
    /// Git branch at deploy time.
    pub branch: Option<String>,
    /// Git commit at deploy time.
    pub commit: Option<String>,
    /// When the deploy started.
    pub started_at: DateTime<Utc>,
    /// Run duration.
    pub duration_ms: Option<u64>,
    /// Whether this was a retry or rollback.
    pub run_kind: RunKind,
}

// ---------------------------------------------------------------------------
// Project (a tracked repo in Chibby)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stage_status_serialization() {
        let statuses = vec![
            (StageStatus::Pending, "pending"),
            (StageStatus::Running, "running"),
            (StageStatus::Success, "success"),
            (StageStatus::Failed, "failed"),
            (StageStatus::Skipped, "skipped"),
        ];

        for (status, expected) in statuses {
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(json, format!("\"{}\"", expected));
        }
    }

    #[test]
    fn test_run_status_serialization() {
        let statuses = vec![
            (RunStatus::Pending, "pending"),
            (RunStatus::Running, "running"),
            (RunStatus::Success, "success"),
            (RunStatus::Failed, "failed"),
            (RunStatus::Cancelled, "cancelled"),
        ];

        for (status, expected) in statuses {
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(json, format!("\"{}\"", expected));
        }
    }
}
