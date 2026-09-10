//! Stage reliability: what breaks, what times out, and what only passes on
//! the second try.
//!
//! `flaky_passes` is the headline number here. A stage that always ends up
//! green but needs three attempts is not healthy, and it is only detectable
//! because stage results record `attempts`. It is reported separately from
//! `failure_rate` so a flaky stage can't hide behind a 100% pass rate.
//!
//! Health-check failures are counted apart from command failures: the
//! commands succeeded and the service still came up wrong, which is a
//! different problem with a different fix.

use super::stats::{average, percentile_95, rate};
use crate::engine::models::StageStatus;
use crate::engine::run_index::RunSummary;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// Reliability of one stage across the report's scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageReliability {
    pub stage_name: String,
    /// Executions counted (skipped and never-run stages are excluded).
    pub runs: u32,
    /// `Failed` or `TimedOut`.
    pub failures: u32,
    pub timeouts: u32,
    /// `failures / runs`, 0.0 when the stage never executed.
    pub failure_rate: f64,
    /// Executions that succeeded only after a retry (`attempts > 1`).
    pub flaky_passes: u32,
    /// `flaky_passes / runs`.
    pub flaky_rate: f64,
    pub avg_duration_ms: Option<u64>,
    pub p95_duration_ms: Option<u64>,
    /// The run holding this stage's slowest execution.
    pub slowest_run_id: Option<String>,
}

/// Where one project's failures concentrate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureHotspot {
    pub repo_path: String,
    pub project_name: String,
    /// The stage that failed most often, if anything failed at all.
    pub top_failing_stage: Option<String>,
    /// Failures of that stage.
    pub stage_failures: u32,
    /// Failed stage executions across the whole project.
    pub total_failures: u32,
    /// Runs whose commands passed but whose post-deploy health check did not.
    pub health_check_failures: u32,
    pub top_health_failure_stage: Option<String>,
}

/// Failure counts for one project, split by cause.
#[derive(Default)]
struct ProjectFailures {
    /// Failed stage executions, by stage name.
    stages: HashMap<String, u32>,
    /// Health-check failures, by the stage whose check failed.
    health: HashMap<String, u32>,
}

/// Running tally for one stage name.
#[derive(Default)]
struct StageTally {
    runs: u32,
    failures: u32,
    timeouts: u32,
    flaky_passes: u32,
    durations: Vec<u64>,
    slowest: Option<(u64, String)>,
}

/// Whether a stage actually executed and so belongs in the denominator.
fn executed(status: &StageStatus) -> bool {
    matches!(
        status,
        StageStatus::Success | StageStatus::Failed | StageStatus::TimedOut
    )
}

/// Aggregate per-stage reliability, worst failure rate first.
pub fn stage_reliability(summaries: &[RunSummary]) -> Vec<StageReliability> {
    let mut tallies: BTreeMap<String, StageTally> = BTreeMap::new();

    for summary in summaries {
        for stage in &summary.stages {
            if !executed(&stage.status) {
                continue;
            }
            let tally = tallies.entry(stage.name.clone()).or_default();
            tally.runs += 1;

            match stage.status {
                StageStatus::TimedOut => {
                    tally.timeouts += 1;
                    tally.failures += 1;
                }
                StageStatus::Failed => tally.failures += 1,
                // Green, but it took more than one go to get there.
                StageStatus::Success if stage.attempts.unwrap_or(1) > 1 => tally.flaky_passes += 1,
                _ => {}
            }

            if let Some(ms) = stage.duration_ms {
                tally.durations.push(ms);
                if tally.slowest.as_ref().map_or(true, |(max, _)| ms > *max) {
                    tally.slowest = Some((ms, summary.id.clone()));
                }
            }
        }
    }

    let mut stages: Vec<StageReliability> = tallies
        .into_iter()
        .map(|(stage_name, tally)| StageReliability {
            stage_name,
            runs: tally.runs,
            failures: tally.failures,
            timeouts: tally.timeouts,
            failure_rate: rate(tally.failures, tally.runs),
            flaky_passes: tally.flaky_passes,
            flaky_rate: rate(tally.flaky_passes, tally.runs),
            avg_duration_ms: average(&tally.durations),
            p95_duration_ms: percentile_95(&tally.durations),
            slowest_run_id: tally.slowest.map(|(_, id)| id),
        })
        .collect();

    stages.sort_by(|a, b| {
        b.failure_rate
            .partial_cmp(&a.failure_rate)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.flaky_passes.cmp(&a.flaky_passes))
            .then_with(|| a.stage_name.cmp(&b.stage_name))
    });
    stages
}

/// The most common failing stage per project, plus health-check failures.
pub fn failure_hotspots(
    summaries: &[RunSummary],
    project_name: impl Fn(&str) -> String,
) -> Vec<FailureHotspot> {
    let mut by_project: BTreeMap<String, ProjectFailures> = BTreeMap::new();

    for summary in summaries {
        let entry = by_project.entry(summary.repo_path.clone()).or_default();
        for stage in summary.stages.iter().filter(|s| s.status.is_failure()) {
            *entry.stages.entry(stage.name.clone()).or_default() += 1;
        }
        if let Some(stage) = &summary.health_failure_stage {
            *entry.health.entry(stage.clone()).or_default() += 1;
        }
    }

    by_project
        .into_iter()
        .map(|(repo_path, failures)| {
            let top = top_entry(&failures.stages);
            let top_health = top_entry(&failures.health);
            FailureHotspot {
                project_name: project_name(&repo_path),
                repo_path,
                top_failing_stage: top.as_ref().map(|(name, _)| name.clone()),
                stage_failures: top.map(|(_, count)| count).unwrap_or(0),
                total_failures: failures.stages.values().sum(),
                health_check_failures: failures.health.values().sum(),
                top_health_failure_stage: top_health.map(|(name, _)| name),
            }
        })
        .filter(|h| h.total_failures > 0 || h.health_check_failures > 0)
        .collect()
}

/// Highest count, ties broken by name so the output is deterministic.
fn top_entry(counts: &HashMap<String, u32>) -> Option<(String, u32)> {
    counts
        .iter()
        .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)))
        .map(|(name, count)| (name.clone(), *count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::insights::tests_support::{stage, summary_with_stages};
    use crate::engine::models::RunStatus;

    #[test]
    fn test_flaky_passes_count_only_successes_that_needed_a_retry() {
        let runs = vec![
            summary_with_stages(
                "a",
                RunStatus::Success,
                vec![stage("test", StageStatus::Success, Some(3), Some(100))],
            ),
            summary_with_stages(
                "b",
                RunStatus::Success,
                vec![stage("test", StageStatus::Success, Some(1), Some(100))],
            ),
            // A failure with several attempts is a failure, not a flaky pass.
            summary_with_stages(
                "c",
                RunStatus::Failed,
                vec![stage("test", StageStatus::Failed, Some(3), Some(100))],
            ),
        ];

        let reliability = &stage_reliability(&runs)[0];

        assert_eq!(reliability.runs, 3);
        assert_eq!(reliability.flaky_passes, 1);
        assert!((reliability.flaky_rate - 1.0 / 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_timeouts_count_as_failures_and_are_reported_separately() {
        let runs = vec![
            summary_with_stages(
                "a",
                RunStatus::Failed,
                vec![stage("deploy", StageStatus::TimedOut, Some(1), Some(500))],
            ),
            summary_with_stages(
                "b",
                RunStatus::Success,
                vec![stage("deploy", StageStatus::Success, Some(1), Some(100))],
            ),
        ];

        let reliability = &stage_reliability(&runs)[0];

        assert_eq!(reliability.failures, 1);
        assert_eq!(reliability.timeouts, 1);
        assert_eq!(reliability.failure_rate, 0.5);
        assert_eq!(reliability.slowest_run_id.as_deref(), Some("a"));
        assert_eq!(reliability.avg_duration_ms, Some(300));
    }

    #[test]
    fn test_skipped_stages_are_excluded_and_zero_runs_does_not_divide_by_zero() {
        let runs = vec![summary_with_stages(
            "a",
            RunStatus::Success,
            vec![stage("lint", StageStatus::Skipped, None, None)],
        )];

        let reliability = stage_reliability(&runs);

        assert!(reliability.is_empty());
        assert!(stage_reliability(&[]).is_empty());
    }

    #[test]
    fn test_p95_with_one_and_two_samples() {
        let one = vec![summary_with_stages(
            "a",
            RunStatus::Success,
            vec![stage("build", StageStatus::Success, Some(1), Some(700))],
        )];
        assert_eq!(stage_reliability(&one)[0].p95_duration_ms, Some(700));

        let mut two = one;
        two.push(summary_with_stages(
            "b",
            RunStatus::Success,
            vec![stage("build", StageStatus::Success, Some(1), Some(300))],
        ));
        assert_eq!(stage_reliability(&two)[0].p95_duration_ms, Some(700));
    }

    #[test]
    fn test_hotspots_pick_the_top_failing_stage_and_count_health_failures() {
        let mut unhealthy = summary_with_stages(
            "c",
            RunStatus::Failed,
            vec![stage("deploy", StageStatus::Success, Some(1), Some(10))],
        );
        unhealthy.health_failure_stage = Some("deploy".to_string());

        let runs = vec![
            summary_with_stages(
                "a",
                RunStatus::Failed,
                vec![stage("test", StageStatus::Failed, Some(1), Some(10))],
            ),
            summary_with_stages(
                "b",
                RunStatus::Failed,
                vec![
                    stage("test", StageStatus::Failed, Some(1), Some(10)),
                    stage("build", StageStatus::TimedOut, Some(1), Some(10)),
                ],
            ),
            unhealthy,
        ];

        let hotspots = failure_hotspots(&runs, |p| p.to_string());

        assert_eq!(hotspots.len(), 1);
        assert_eq!(hotspots[0].top_failing_stage.as_deref(), Some("test"));
        assert_eq!(hotspots[0].stage_failures, 2);
        assert_eq!(hotspots[0].total_failures, 3);
        assert_eq!(hotspots[0].health_check_failures, 1);
        assert_eq!(
            hotspots[0].top_health_failure_stage.as_deref(),
            Some("deploy")
        );
    }

    #[test]
    fn test_projects_without_failures_are_not_hotspots() {
        let runs = vec![summary_with_stages(
            "a",
            RunStatus::Success,
            vec![stage("test", StageStatus::Success, Some(1), Some(10))],
        )];

        assert!(failure_hotspots(&runs, |p| p.to_string()).is_empty());
    }
}
