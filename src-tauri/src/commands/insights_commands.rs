//! Tauri commands for run metrics and the run index behind them.

use crate::engine::insights::{self, InsightsReport};
use crate::engine::models::CleanupConfig;
use crate::engine::run_index::{self, IndexStats};

/// Metrics for one project, or every project when `repo_path` is omitted.
#[tauri::command]
pub fn get_insights(
    repo_path: Option<String>,
    window_days: Option<u32>,
) -> Result<InsightsReport, String> {
    insights::report(
        repo_path.as_deref(),
        window_days.unwrap_or(insights::DEFAULT_WINDOW_DAYS),
    )
    .map_err(|e| e.to_string())
}

/// Regenerate the run index from `runs/`. Returns the resulting entry count.
#[tauri::command]
pub fn rebuild_run_index() -> Result<usize, String> {
    run_index::rebuild().map_err(|e| e.to_string())
}

/// Apply the index's retention bounds now. Returns how many entries were
/// dropped. Omitted bounds fall back to the cleanup defaults.
#[tauri::command]
pub fn prune_run_index(
    retention_days: Option<u32>,
    max_entries: Option<u32>,
) -> Result<u32, String> {
    let defaults = CleanupConfig::default();
    run_index::prune(
        retention_days.unwrap_or(defaults.index_retention_days),
        max_entries.unwrap_or(defaults.index_max_entries),
    )
    .map_err(|e| e.to_string())
}

/// Entry count and on-disk size of the run index.
#[tauri::command]
pub fn get_run_index_stats() -> Result<IndexStats, String> {
    run_index::stats().map_err(|e| e.to_string())
}
