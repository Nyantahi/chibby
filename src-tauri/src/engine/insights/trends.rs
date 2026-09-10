//! Period aggregates and deltas — the "is this getting worse?" half.
//!
//! Deliberately not a time series: the numbers here render as stat rows and a
//! compact table, and each one is paired with the immediately preceding window
//! of equal length so a direction is visible without a chart.
//!
//! **Which runs count.** Only terminal runs (`Success`, `Failed`, `Cancelled`)
//! are counted; `Pending`/`Running` are excluded so an in-flight run can't
//! move a rate. `Retry` and `Rollback` runs *are* counted here — they executed,
//! took time and can fail, so leaving them out would under-report the machine's
//! real workload. They are excluded from deployment health instead, where a
//! rollback restores an older commit rather than shipping a new one (see
//! [`super::environments`]).

use super::stats::{average, delta_pct, percentile_95, rate};
use crate::engine::models::{RunStatus, StageStatus};
use crate::engine::run_index::RunSummary;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Aggregate outcome of one window.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PeriodStats {
    /// Terminal runs in the window (succeeded + failed + cancelled).
    pub runs: u32,
    pub succeeded: u32,
    pub failed: u32,
    pub cancelled: u32,
    /// `succeeded / (succeeded + failed)`. Cancelled runs are reported but
    /// kept out of the rate: a cancel is a human decision, not an outcome.
    pub success_rate: f64,
    pub avg_duration_ms: Option<u64>,
    pub p95_duration_ms: Option<u64>,
    /// Runs nobody was watching (scheduled + file-watch).
    pub unattended_runs: u32,
    pub unattended_failures: u32,
}

/// One window against the one before it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrendComparison {
    pub current: PeriodStats,
    pub previous: PeriodStats,
    /// Percentage *points*, not percent: 0.80 -> 0.90 is `+10.0`.
    /// `None` when the previous window had no terminal runs — there is nothing
    /// to compare against, and reporting `+40.0` for a first-ever window is a
    /// fabricated improvement.
    pub success_rate_delta: Option<f64>,
    /// Percent change in average duration. `None` when there is no baseline.
    pub avg_duration_delta_pct: Option<f64>,
}

/// One row of the daily table (a row per day, not a plot point).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyCount {
    pub date: NaiveDate,
    pub runs: u32,
    pub succeeded: u32,
    pub failed: u32,
}

/// How one stage's average duration moved between the two windows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageDelta {
    pub stage_name: String,
    pub current_avg_ms: Option<u64>,
    pub previous_avg_ms: Option<u64>,
    /// Percent change; `None` when the stage has no previous-window baseline.
    pub delta_pct: Option<f64>,
    /// Executions counted in the current window.
    pub runs: u32,
}

/// Whether a run has reached a final state and should count toward rates.
fn is_terminal(summary: &RunSummary) -> bool {
    matches!(
        summary.status,
        RunStatus::Success | RunStatus::Failed | RunStatus::Cancelled
    )
}

/// Aggregate one window's runs.
pub fn period_stats(summaries: &[RunSummary]) -> PeriodStats {
    let mut stats = PeriodStats::default();
    let mut durations = Vec::new();

    for summary in summaries.iter().filter(|s| is_terminal(s)) {
        stats.runs += 1;
        match summary.status {
            RunStatus::Success => stats.succeeded += 1,
            RunStatus::Failed => stats.failed += 1,
            _ => stats.cancelled += 1,
        }
        if summary.run_kind.is_unattended() {
            stats.unattended_runs += 1;
            if summary.status == RunStatus::Failed {
                stats.unattended_failures += 1;
            }
        }
        if let Some(ms) = summary.duration_ms {
            durations.push(ms);
        }
    }

    stats.success_rate = rate(stats.succeeded, stats.succeeded + stats.failed);
    stats.avg_duration_ms = average(&durations);
    stats.p95_duration_ms = percentile_95(&durations);
    stats
}

/// Compare a window with the equal-length window immediately before it.
pub fn compare(current: &[RunSummary], previous: &[RunSummary]) -> TrendComparison {
    let current = period_stats(current);
    let previous = period_stats(previous);

    // A window with no terminal runs is not a baseline of "0% success"; it is
    // an absence of data. Deltas against it are meaningless.
    let has_baseline = previous.succeeded + previous.failed > 0;

    TrendComparison {
        success_rate_delta: has_baseline
            .then(|| (current.success_rate - previous.success_rate) * 100.0),
        avg_duration_delta_pct: delta_pct(current.avg_duration_ms, previous.avg_duration_ms),
        current,
        previous,
    }
}

/// A row per calendar day (UTC) for the last `days` days, oldest first.
/// Days with no runs are included so the table has no gaps.
pub fn daily_counts(summaries: &[RunSummary], days: u32, now: DateTime<Utc>) -> Vec<DailyCount> {
    let days = days.max(1);
    let mut tally: HashMap<NaiveDate, (u32, u32, u32)> = HashMap::new();

    for summary in summaries.iter().filter(|s| is_terminal(s)) {
        let entry = tally
            .entry(summary.started_at.date_naive())
            .or_insert((0, 0, 0));
        entry.0 += 1;
        match summary.status {
            RunStatus::Success => entry.1 += 1,
            RunStatus::Failed => entry.2 += 1,
            _ => {}
        }
    }

    let today = now.date_naive();
    (0..days)
        .rev()
        .map(|back| {
            let date = today - Duration::days(back as i64);
            let (runs, succeeded, failed) = tally.get(&date).copied().unwrap_or((0, 0, 0));
            DailyCount {
                date,
                runs,
                succeeded,
                failed,
            }
        })
        .collect()
}

/// Per-stage average duration this window versus the last, biggest slowdown
/// first. Answers "is this pipeline getting slower, and where?" without a
/// series.
pub fn stage_deltas(current: &[RunSummary], previous: &[RunSummary]) -> Vec<StageDelta> {
    let current_durations = stage_durations(current);
    let previous_durations = stage_durations(previous);

    let mut deltas: Vec<StageDelta> = current_durations
        .into_iter()
        .map(|(stage_name, durations)| {
            let current_avg_ms = average(&durations);
            let previous_avg_ms = previous_durations.get(&stage_name).and_then(|d| average(d));
            StageDelta {
                delta_pct: delta_pct(current_avg_ms, previous_avg_ms),
                runs: durations.len() as u32,
                stage_name,
                current_avg_ms,
                previous_avg_ms,
            }
        })
        .collect();

    // Slowing down first; stages with no baseline sort last.
    deltas.sort_by(|a, b| {
        b.delta_pct
            .unwrap_or(f64::MIN)
            .partial_cmp(&a.delta_pct.unwrap_or(f64::MIN))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.stage_name.cmp(&b.stage_name))
    });
    deltas
}

/// Executed stage durations grouped by stage name. Skipped and never-run
/// stages contribute nothing.
fn stage_durations(summaries: &[RunSummary]) -> HashMap<String, Vec<u64>> {
    let mut by_stage: HashMap<String, Vec<u64>> = HashMap::new();
    for stage in summaries.iter().flat_map(|s| s.stages.iter()) {
        let executed = matches!(
            stage.status,
            StageStatus::Success | StageStatus::Failed | StageStatus::TimedOut
        );
        if !executed {
            continue;
        }
        if let Some(ms) = stage.duration_ms {
            by_stage.entry(stage.name.clone()).or_default().push(ms);
        }
    }
    by_stage
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::insights::tests_support::{summary, with_stage};
    use crate::engine::models::RunKind;

    #[test]
    fn test_period_stats_ignores_in_flight_runs_and_guards_division() {
        let empty = period_stats(&[]);
        assert_eq!(empty.runs, 0);
        assert_eq!(empty.success_rate, 0.0);
        assert!(empty.avg_duration_ms.is_none());
        assert!(!empty.success_rate.is_nan());

        let running = summary("r1", RunStatus::Running, 0);
        let stats = period_stats(&[running]);
        assert_eq!(stats.runs, 0);
        assert_eq!(stats.success_rate, 0.0);
    }

    #[test]
    fn test_period_stats_counts_outcomes_and_unattended_failures() {
        let mut scheduled = summary("s1", RunStatus::Failed, 1);
        scheduled.run_kind = RunKind::Scheduled;

        let stats = period_stats(&[
            summary("ok", RunStatus::Success, 1),
            summary("bad", RunStatus::Failed, 2),
            summary("stop", RunStatus::Cancelled, 3),
            scheduled,
        ]);

        assert_eq!(stats.runs, 4);
        assert_eq!((stats.succeeded, stats.failed, stats.cancelled), (1, 2, 1));
        // Cancelled stays out of the rate: 1 success out of 3 outcomes.
        assert!((stats.success_rate - 1.0 / 3.0).abs() < f64::EPSILON);
        assert_eq!(stats.unattended_runs, 1);
        assert_eq!(stats.unattended_failures, 1);
    }

    #[test]
    fn test_compare_reports_signed_deltas() {
        let mut fast = summary("fast", RunStatus::Success, 1);
        fast.duration_ms = Some(1_000);
        let mut slow = summary("slow", RunStatus::Success, 30);
        slow.duration_ms = Some(2_000);

        let comparison = compare(&[fast], &[slow.clone()]);

        // 100% success in both windows.
        assert_eq!(comparison.success_rate_delta, Some(0.0));
        assert_eq!(comparison.avg_duration_delta_pct, Some(-50.0));

        let worse = compare(&[summary("bad", RunStatus::Failed, 1)], &[slow]);
        assert_eq!(worse.success_rate_delta, Some(-100.0));
    }

    #[test]
    fn test_compare_with_no_runs_is_all_zero_and_never_nan() {
        let comparison = compare(&[], &[]);

        assert_eq!(comparison.current.runs, 0);
        assert_eq!(comparison.previous.runs, 0);
        assert_eq!(comparison.success_rate_delta, None);
        assert_eq!(comparison.avg_duration_delta_pct, None);
    }

    /// An empty previous window is an absence of data, not a 0% baseline, so a
    /// first-ever window must not claim an improvement it cannot know about.
    #[test]
    fn test_no_previous_runs_yields_no_delta() {
        let comparison = compare(&[summary("ok", RunStatus::Success, 1)], &[]);

        assert_eq!(comparison.current.success_rate, 1.0);
        assert_eq!(comparison.previous.runs, 0);
        assert_eq!(comparison.success_rate_delta, None);
        assert_eq!(comparison.avg_duration_delta_pct, None);
    }

    #[test]
    fn test_daily_counts_emits_a_row_per_day_including_empty_ones() {
        let now = Utc::now();
        let rows = daily_counts(&[summary("today", RunStatus::Success, 0)], 7, now);

        assert_eq!(rows.len(), 7);
        // Oldest first, newest last.
        assert!(rows[0].date < rows[6].date);
        assert_eq!(rows[6].date, now.date_naive());
        assert_eq!((rows[6].runs, rows[6].succeeded), (1, 1));
        assert_eq!(rows[0].runs, 0);
    }

    #[test]
    fn test_stage_deltas_rank_the_biggest_slowdown_first() {
        let current = vec![
            with_stage(summary("c1", RunStatus::Success, 1), "build", 2_000),
            with_stage(summary("c2", RunStatus::Success, 1), "test", 1_000),
        ];
        let previous = vec![
            with_stage(summary("p1", RunStatus::Success, 30), "build", 1_000),
            with_stage(summary("p2", RunStatus::Success, 30), "test", 1_000),
        ];

        let deltas = stage_deltas(&current, &previous);

        assert_eq!(deltas[0].stage_name, "build");
        assert_eq!(deltas[0].delta_pct, Some(100.0));
        assert_eq!(deltas[1].delta_pct, Some(0.0));
    }

    #[test]
    fn test_stage_delta_without_baseline_has_no_pct() {
        let current = vec![with_stage(summary("c1", RunStatus::Success, 1), "new", 500)];

        let deltas = stage_deltas(&current, &[]);

        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].current_avg_ms, Some(500));
        assert!(deltas[0].previous_avg_ms.is_none());
        assert!(deltas[0].delta_pct.is_none());
    }
}
