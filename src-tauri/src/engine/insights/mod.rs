//! Metrics computed from the run index — numbers and tables, never charts.
//!
//! Everything here reads [`RunSummary`] only. No stdout, no stderr, no
//! pipeline snapshots: a report over a year of history costs a single JSON
//! parse of `runs-index.json`.
//!
//! Three questions this exists to answer:
//! - what version is live on production ([`environments`])
//! - which stage keeps failing, or only passes on retry ([`failures`])
//! - is this pipeline getting slower ([`trends`])

pub mod environments;
pub mod failures;
pub mod stats;
pub mod trends;

use crate::engine::persistence;
use crate::engine::run_index::{self, RunSummary};
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use environments::EnvironmentStatus;
use failures::{FailureHotspot, StageReliability};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use trends::{DailyCount, StageDelta, TrendComparison};

/// Default window when a caller does not choose one.
pub const DEFAULT_WINDOW_DAYS: u32 = 7;

/// Everything the metrics view renders, in one call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightsReport {
    pub generated_at: DateTime<Utc>,
    pub window_days: u32,
    /// This window against the equal-length window before it.
    pub totals: TrendComparison,
    /// A row per day for the table; not a plot series.
    pub daily: Vec<DailyCount>,
    /// Current deploy state. Ignores the window on purpose — what is live is
    /// live however long ago it shipped.
    pub environments: Vec<EnvironmentStatus>,
    pub stages: Vec<StageReliability>,
    /// Most common failing stage per project, plus health-check failures.
    pub hotspots: Vec<FailureHotspot>,
    /// Per-stage average duration versus the previous window, worst first.
    pub slowest_stages: Vec<StageDelta>,
}

/// Build the report. `repo_path` `None` covers every project (the environment
/// matrix is inherently cross-project); `Some(path)` scopes to one.
///
/// `window_days` drives both windows: the current one is the last `window_days`
/// days, the previous one the `window_days` immediately before that.
pub fn report(repo_path: Option<&str>, window_days: u32) -> Result<InsightsReport> {
    let window_days = window_days.max(1);
    let summaries = match repo_path {
        Some(path) => run_index::load_for_project(path)?,
        None => run_index::load()?,
    };

    let now = Utc::now();
    let current_start = now - Duration::days(window_days as i64);
    let previous_start = current_start - Duration::days(window_days as i64);

    let current = slice_window(&summaries, current_start, None);
    let previous = slice_window(&summaries, previous_start, Some(current_start));

    let names = project_names();
    let name_of = |path: &str| lookup_name(&names, path);

    Ok(InsightsReport {
        generated_at: now,
        window_days,
        totals: trends::compare(&current, &previous),
        daily: trends::daily_counts(&current, window_days, now),
        environments: environments::environment_matrix(&summaries, name_of),
        stages: failures::stage_reliability(&current),
        hotspots: failures::failure_hotspots(&current, name_of),
        slowest_stages: trends::stage_deltas(&current, &previous),
    })
}

/// Runs started in `[from, until)`.
fn slice_window(
    summaries: &[RunSummary],
    from: DateTime<Utc>,
    until: Option<DateTime<Utc>>,
) -> Vec<RunSummary> {
    summaries
        .iter()
        .filter(|s| s.started_at >= from && until.map_or(true, |end| s.started_at < end))
        .cloned()
        .collect()
}

/// Tracked project names by repo path. A missing projects index is not worth
/// failing a report over — the path is a usable label on its own.
fn project_names() -> HashMap<String, String> {
    persistence::load_projects()
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.path, p.name))
        .collect()
}

/// The tracked name for a repo, falling back to its directory name.
fn lookup_name(names: &HashMap<String, String>, repo_path: &str) -> String {
    if let Some(name) = names.get(repo_path) {
        return name.clone();
    }
    std::path::Path::new(repo_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(repo_path)
        .to_string()
}

/// Shared fixtures for the insights unit tests.
#[cfg(test)]
pub(crate) mod tests_support {
    use crate::engine::models::{RunKind, RunStatus, StageStatus};
    use crate::engine::run_index::{RunSummary, StageSummary};
    use chrono::{Duration, Utc};

    pub(crate) const REPO: &str = "/tmp/chibby-insights";

    /// A terminal run `minutes_ago` old with no stages.
    pub(crate) fn summary(id: &str, status: RunStatus, minutes_ago: i64) -> RunSummary {
        RunSummary {
            id: id.to_string(),
            pipeline_name: "ci".to_string(),
            repo_path: REPO.to_string(),
            environment: None,
            branch: Some("main".to_string()),
            commit: Some("deadbee".to_string()),
            status,
            started_at: Utc::now() - Duration::minutes(minutes_ago),
            finished_at: None,
            duration_ms: Some(1_000),
            run_kind: RunKind::Normal,
            trigger_id: None,
            health_failure_stage: None,
            rollback_outcome: None,
            auto_rollback_of: None,
            stages: Vec::new(),
            logs_pruned: false,
        }
    }

    /// A run that targeted an environment.
    pub(crate) fn deployment(
        id: &str,
        environment: &str,
        status: RunStatus,
        minutes_ago: i64,
    ) -> RunSummary {
        let mut run = summary(id, status, minutes_ago);
        run.environment = Some(environment.to_string());
        run
    }

    /// One stage summary.
    pub(crate) fn stage(
        name: &str,
        status: StageStatus,
        attempts: Option<u32>,
        duration_ms: Option<u64>,
    ) -> StageSummary {
        StageSummary {
            name: name.to_string(),
            status,
            duration_ms,
            attempts,
        }
    }

    /// A run carrying the given stages.
    pub(crate) fn summary_with_stages(
        id: &str,
        status: RunStatus,
        stages: Vec<StageSummary>,
    ) -> RunSummary {
        let mut run = summary(id, status, 1);
        run.stages = stages;
        run
    }

    /// A run with a single successful stage of the given duration.
    pub(crate) fn with_stage(mut run: RunSummary, name: &str, duration_ms: u64) -> RunSummary {
        run.stages = vec![stage(
            name,
            StageStatus::Success,
            Some(1),
            Some(duration_ms),
        )];
        run
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::models::{PipelineRun, RunStatus, StageResult, StageStatus};
    use crate::engine::persistence;

    fn saved_run(id: &str, environment: &str, status: RunStatus) -> PipelineRun {
        let mut run = PipelineRun::new_with_id(
            id,
            "ci",
            "/tmp/chibby-report",
            Some(environment.to_string()),
        );
        run.status = status;
        run.duration_ms = Some(2_000);
        run.stage_results = vec![StageResult {
            stage_name: "build".to_string(),
            status: StageStatus::Success,
            exit_code: Some(0),
            stdout: "x".repeat(10_000),
            stderr: String::new(),
            started_at: None,
            finished_at: None,
            duration_ms: Some(1_500),
            health_check_passed: None,
            attempts: Some(2),
            skip_reason: None,
        }];
        persistence::save_run(&run).unwrap();
        run
    }

    #[test]
    fn test_report_covers_totals_environments_and_flaky_stages() {
        let (_dir, _lock) = persistence::scoped_test_data_dir();
        saved_run("ok", "prod", RunStatus::Success);
        saved_run("bad", "prod", RunStatus::Failed);

        let report = report(Some("/tmp/chibby-report"), 7).unwrap();

        assert_eq!(report.window_days, 7);
        assert_eq!(report.totals.current.runs, 2);
        assert_eq!(report.daily.len(), 7);
        assert_eq!(report.environments.len(), 1);
        assert_eq!(report.environments[0].environment, "prod");
        // Both runs' build stage passed on the second attempt.
        assert_eq!(report.stages[0].flaky_passes, 2);
        assert_eq!(report.hotspots.len(), 0);
        assert_eq!(report.slowest_stages[0].stage_name, "build");
    }

    #[test]
    fn test_report_scoped_to_another_project_is_empty_but_valid() {
        let (_dir, _lock) = persistence::scoped_test_data_dir();
        saved_run("ok", "prod", RunStatus::Success);

        let report = report(Some("/tmp/somewhere-else"), 30).unwrap();

        assert_eq!(report.totals.current.runs, 0);
        assert_eq!(report.totals.current.success_rate, 0.0);
        assert!(report.environments.is_empty());
        assert!(report.stages.is_empty());
        assert_eq!(report.daily.len(), 30);
    }

    #[test]
    fn test_zero_window_days_is_clamped_rather_than_dividing_by_zero() {
        let (_dir, _lock) = persistence::scoped_test_data_dir();

        let report = report(None, 0).unwrap();

        assert_eq!(report.window_days, 1);
        assert_eq!(report.daily.len(), 1);
    }
}
