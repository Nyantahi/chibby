//! `chibby insights` — run metrics as numbers and tables.
//!
//! Deliberately no ASCII charts: the user asked for counts, rates, durations
//! and deltas that can be read at a glance or piped through `--json`.

use crate::cli::{self, icons, Printer, StageStatus};
use chibby_lib::engine::insights::{
    environments::EnvironmentStatus, failures::StageReliability, trends::StageDelta, InsightsReport,
};
use chibby_lib::engine::models::CleanupConfig;
use chibby_lib::engine::run_index;
use chibby_lib::engine::{insights, persistence};
use owo_colors::OwoColorize;
use std::path::{Path, PathBuf};

/// How many rows the wider tables show before truncating.
const MAX_ROWS: usize = 12;

pub(crate) fn show_insights(
    printer: &Printer,
    project: Option<&PathBuf>,
    days: u32,
    json: bool,
    rebuild: bool,
    prune: bool,
) -> anyhow::Result<()> {
    if rebuild {
        let entries = run_index::rebuild()?;
        printer.success(&format!("Rebuilt run index — {entries} entries"));
    }
    if prune {
        let defaults = CleanupConfig::default();
        let dropped = run_index::prune(defaults.index_retention_days, defaults.index_max_entries)?;
        printer.success(&format!("Pruned {dropped} run index entries"));
    }

    let scope = project.map(|p| resolve_scope(p)).transpose()?;
    let report = insights::report(scope.as_deref(), days)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    render(printer, &report, scope.as_deref());
    Ok(())
}

/// Canonical repo path for `--project`, matching how runs are recorded.
fn resolve_scope(project: &Path) -> anyhow::Result<String> {
    let path = project
        .canonicalize()
        .unwrap_or_else(|_| project.to_path_buf());
    Ok(path.to_string_lossy().to_string())
}

fn render(printer: &Printer, report: &InsightsReport, scope: Option<&str>) {
    printer.header(&format!("{} Insights", icons::CHART));
    printer.kv("Scope", scope.unwrap_or("all projects"));
    printer.kv("Window", &format!("{} days", report.window_days));

    render_totals(printer, report);
    render_environments(printer, &report.environments);
    render_stages(printer, &report.stages);
    render_slowest(printer, &report.slowest_stages);
    render_hotspots(printer, report);
    render_daily(printer, report);
}

fn render_totals(printer: &Printer, report: &InsightsReport) {
    let current = &report.totals.current;
    let previous = &report.totals.previous;

    printer.subheader("Totals");
    printer.kv(
        "Runs",
        &format!("{} (previous window {})", current.runs, previous.runs),
    );
    printer.kv_colored(
        "Success rate",
        &format!(
            "{}{}",
            percent(current.success_rate),
            delta_suffix(report.totals.success_rate_delta, " pts")
        ),
        rate_status(current.success_rate, current.runs),
    );
    printer.kv(
        "Outcomes",
        &format!(
            "{} ok / {} failed / {} cancelled",
            current.succeeded, current.failed, current.cancelled
        ),
    );
    printer.kv(
        "Avg duration",
        &format!(
            "{}{}",
            duration(current.avg_duration_ms),
            delta_suffix(report.totals.avg_duration_delta_pct, "%")
        ),
    );
    printer.kv("p95 duration", &duration(current.p95_duration_ms));
    printer.kv(
        "Unattended",
        &format!(
            "{} runs, {} failed",
            current.unattended_runs, current.unattended_failures
        ),
    );
}

fn render_environments(printer: &Printer, environments: &[EnvironmentStatus]) {
    printer.subheader("Environments");
    if environments.is_empty() {
        printer.info("No environment-scoped runs recorded.");
        return;
    }

    row(&format!(
        "{:<18} {:<10} {:<10} {:<12} {}",
        "PROJECT", "ENV", "COMMIT", "DEPLOYED", "STATE"
    ));
    for env in environments {
        let state = match (env.current_run_id.is_some(), env.is_stale) {
            (_, true) => format!("{} failed since", env.failed_since)
                .yellow()
                .to_string(),
            (true, false) => "current".green().to_string(),
            (false, false) => "never deployed".bright_black().to_string(),
        };
        row(&format!(
            "{:<18} {:<10} {:<10} {:<12} {}",
            truncate(&env.project_name, 18),
            truncate(&env.environment, 10),
            env.commit
                .as_deref()
                .map(|c| truncate(c, 9))
                .unwrap_or_else(|| "-".to_string()),
            env.deployed_at.map(ago).unwrap_or_else(|| "-".to_string()),
            state
        ));
    }
}

fn render_stages(printer: &Printer, stages: &[StageReliability]) {
    printer.subheader("Stage reliability");
    if stages.is_empty() {
        printer.info("No stages executed in this window.");
        return;
    }

    row(&format!(
        "{:<18} {:>5} {:>6} {:>7} {:>6} {:>9} {:>9}",
        "STAGE", "RUNS", "FAILED", "RATE", "FLAKY", "AVG", "P95"
    ));
    for stage in stages.iter().take(MAX_ROWS) {
        row(&format!(
            "{:<18} {:>5} {:>6} {:>7} {:>6} {:>9} {:>9}",
            truncate(&stage.stage_name, 18),
            stage.runs,
            stage.failures,
            percent(stage.failure_rate),
            stage.flaky_passes,
            duration(stage.avg_duration_ms),
            duration(stage.p95_duration_ms),
        ));
    }

    let flaky: u32 = stages.iter().map(|s| s.flaky_passes).sum();
    if flaky > 0 {
        printer.warn(&format!(
            "{flaky} stage runs passed only after a retry — flaky, not healthy"
        ));
    }
}

fn render_slowest(printer: &Printer, slowest: &[StageDelta]) {
    let comparable: Vec<&StageDelta> = slowest
        .iter()
        .filter(|s| s.delta_pct.is_some())
        .take(MAX_ROWS)
        .collect();
    if comparable.is_empty() {
        return;
    }

    printer.subheader("Duration vs previous window");
    row(&format!(
        "{:<18} {:>10} {:>10} {:>9}",
        "STAGE", "NOW", "BEFORE", "CHANGE"
    ));
    for stage in comparable {
        row(&format!(
            "{:<18} {:>10} {:>10} {:>8}%",
            truncate(&stage.stage_name, 18),
            duration(stage.current_avg_ms),
            duration(stage.previous_avg_ms),
            signed(stage.delta_pct.unwrap_or(0.0)),
        ));
    }
}

fn render_hotspots(printer: &Printer, report: &InsightsReport) {
    if report.hotspots.is_empty() {
        return;
    }

    printer.subheader("Failure hotspots");
    for hotspot in report.hotspots.iter().take(MAX_ROWS) {
        let top = hotspot.top_failing_stage.as_deref().unwrap_or("-");
        printer.kv(
            &truncate(&hotspot.project_name, 18),
            &format!(
                "{} failures, worst stage '{}' ({}), {} health-check failures",
                hotspot.total_failures, top, hotspot.stage_failures, hotspot.health_check_failures
            ),
        );
    }
}

fn render_daily(printer: &Printer, report: &InsightsReport) {
    printer.subheader("Daily");
    row(&format!(
        "{:<12} {:>5} {:>5} {:>6}",
        "DATE", "RUNS", "OK", "FAILED"
    ));
    for day in &report.daily {
        row(&format!(
            "{:<12} {:>5} {:>5} {:>6}",
            day.date, day.runs, day.succeeded, day.failed
        ));
    }
}

/// Index size, surfaced by `chibby doctor` so growth is visible early.
pub(crate) fn print_index_health(printer: &Printer) {
    let Ok(stats) = run_index::stats() else {
        printer.warn("Run index unavailable");
        return;
    };
    printer.kv(
        "Run index",
        &format!(
            "{} summaries ({} KB, {} with pruned logs)",
            stats.entries,
            stats.bytes / 1024,
            stats.payloads_pruned
        ),
    );
    if let Ok(dir) = persistence::data_dir() {
        printer.kv(
            "Index file",
            &dir.join("runs-index.json").display().to_string(),
        );
    }
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

/// One table row, indented to match the printer's key-value rows.
fn row(text: &str) {
    println!("    {text}");
}

fn percent(rate: f64) -> String {
    format!("{:.1}%", rate * 100.0)
}

/// Render a delta as " (+3.0 pts)", or nothing at all when there is no
/// baseline to compare against — an empty previous window is missing data, not
/// a zero, and printing "+40.0 pts" for a first-ever window invents a trend.
fn delta_suffix(delta: Option<f64>, unit: &str) -> String {
    match delta {
        Some(d) => format!(" ({}{})", signed(d), unit),
        None => String::new(),
    }
}

fn signed(value: f64) -> String {
    format!("{value:+.1}")
}

fn duration(ms: Option<u64>) -> String {
    ms.map(cli::format_duration)
        .unwrap_or_else(|| "-".to_string())
}

fn ago(when: chrono::DateTime<chrono::Utc>) -> String {
    let seconds = (chrono::Utc::now() - when).num_seconds().max(0);
    cli::format_relative_time(seconds)
}

fn truncate(text: &str, width: usize) -> String {
    match text.chars().count() > width {
        true => text.chars().take(width - 1).collect::<String>() + "…",
        false => text.to_string(),
    }
}

/// Colour the headline rate: red once failures dominate, green when clean.
fn rate_status(rate: f64, runs: u32) -> StageStatus {
    if runs == 0 {
        return StageStatus::Pending;
    }
    match rate {
        r if r >= 0.9 => StageStatus::Success,
        r if r >= 0.6 => StageStatus::Cancelled,
        _ => StageStatus::Failed,
    }
}
