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
    TimedOut,
}

impl StageStatus {
    /// Whether this status counts as a stage failure (drives fail-fast, run
    /// status, retry eligibility and "first failed stage" lookups).
    pub fn is_failure(&self) -> bool {
        matches!(self, StageStatus::Failed | StageStatus::TimedOut)
    }
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
    /// How many attempts the stage took (None when it never executed).
    #[serde(default)]
    pub attempts: Option<u32>,
    /// Why the stage was skipped, when it was skipped by a `when` condition.
    #[serde(default)]
    pub skip_reason: Option<String>,
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

/// What started this run.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RunKind {
    Normal,
    Retry,
    Rollback,
    /// Fired by a cron schedule.
    Scheduled,
    /// Fired by a file watch.
    Watch,
    /// Fired by a git hook (pre-push / pre-commit).
    Hook,
}

impl Default for RunKind {
    fn default() -> Self {
        Self::Normal
    }
}

impl RunKind {
    /// No human is watching — failures must escalate.
    ///
    /// `Hook` is deliberately excluded: someone typed `git push` and is
    /// staring at the terminal output right now.
    pub fn is_unattended(&self) -> bool {
        matches!(self, Self::Scheduled | Self::Watch)
    }
}

/// How an automatic rollback for a failed run turned out.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RollbackOutcome {
    Succeeded,
    Failed,
    /// Policy declined to roll back (mode off, no target, throttled, ...).
    Skipped,
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
    /// Set when a stage failed specifically because its health check failed.
    #[serde(default)]
    pub health_failure_stage: Option<String>,
    /// The auto-rollback run spawned for this run.
    #[serde(default)]
    pub rollback_run_id: Option<String>,
    /// How that auto-rollback turned out.
    #[serde(default)]
    pub rollback_outcome: Option<RollbackOutcome>,
    /// Why auto-rollback did not run, when a policy was configured but a guard
    /// refused it. Without this a throttled rollback is indistinguishable from
    /// no policy at all, while the bad release is still live.
    #[serde(default)]
    pub rollback_skip_reason: Option<String>,
    /// On a rollback run: the failed run that caused it.
    #[serde(default)]
    pub auto_rollback_of: Option<String>,
    /// Which trigger started this run (`scheduled:<id>`, `watch:<id>`,
    /// `hook:pre-push`). None for a run a human started directly.
    #[serde(default)]
    pub trigger_id: Option<String>,
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
            health_failure_stage: None,
            rollback_run_id: None,
            rollback_outcome: None,
            rollback_skip_reason: None,
            auto_rollback_of: None,
            trigger_id: None,
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
            (StageStatus::TimedOut, "timedout"),
        ];

        for (status, expected) in statuses {
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(json, format!("\"{}\"", expected));
        }
    }

    #[test]
    fn test_stage_status_is_failure() {
        assert!(StageStatus::Failed.is_failure());
        assert!(StageStatus::TimedOut.is_failure());
        assert!(!StageStatus::Success.is_failure());
        assert!(!StageStatus::Skipped.is_failure());
        assert!(!StageStatus::Running.is_failure());
        assert!(!StageStatus::Pending.is_failure());
    }

    #[test]
    fn test_rollback_outcome_serialization() {
        assert_eq!(
            serde_json::to_string(&RollbackOutcome::Succeeded).unwrap(),
            r#""succeeded""#
        );
        let parsed: RollbackOutcome = serde_json::from_str(r#""skipped""#).unwrap();
        assert_eq!(parsed, RollbackOutcome::Skipped);
    }

    /// Run records written before auto-rollback existed have none of the four
    /// new keys and must still load.
    #[test]
    fn test_legacy_run_json_without_rollback_fields_deserializes() {
        let json = r#"{
            "id": "abc123",
            "pipeline_name": "deploy",
            "repo_path": "/tmp/repo",
            "environment": "prod",
            "branch": "main",
            "commit": "deadbee",
            "status": "failed",
            "stage_results": [],
            "started_at": "2024-01-01T00:00:00Z",
            "finished_at": null,
            "duration_ms": null
        }"#;

        let run: PipelineRun = serde_json::from_str(json).unwrap();

        assert_eq!(run.id, "abc123");
        assert_eq!(run.run_kind, RunKind::Normal);
        assert!(run.health_failure_stage.is_none());
        assert!(run.rollback_run_id.is_none());
        assert!(run.rollback_outcome.is_none());
        assert!(run.auto_rollback_of.is_none());
        assert!(run.trigger_id.is_none());
    }

    #[test]
    fn test_run_kind_round_trips_the_trigger_variants() {
        for (kind, expected) in [
            (RunKind::Scheduled, "\"scheduled\""),
            (RunKind::Watch, "\"watch\""),
            (RunKind::Hook, "\"hook\""),
        ] {
            assert_eq!(serde_json::to_string(&kind).unwrap(), expected);
            assert_eq!(serde_json::from_str::<RunKind>(expected).unwrap(), kind);
        }
    }

    /// Escalation policy: only the kinds nobody is watching count.
    #[test]
    fn test_only_scheduled_and_watch_are_unattended() {
        assert!(RunKind::Scheduled.is_unattended());
        assert!(RunKind::Watch.is_unattended());
        assert!(!RunKind::Hook.is_unattended());
        assert!(!RunKind::Normal.is_unattended());
        assert!(!RunKind::Retry.is_unattended());
        assert!(!RunKind::Rollback.is_unattended());
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
