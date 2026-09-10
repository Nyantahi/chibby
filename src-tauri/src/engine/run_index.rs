//! Compact summaries of every run, stored in `<data_dir>/runs-index.json`.
//!
//! A run record on disk carries full stdout/stderr inline, so answering
//! "how often does the deploy stage fail?" by loading `runs/*.json` means
//! parsing megabytes of logs. The index keeps a few hundred bytes per run —
//! everything the metrics views need and nothing they don't.
//!
//! The index is *derived* state: if it is missing, unreadable or behind the
//! runs directory it is rebuilt transparently. A metrics view must never be
//! able to break the app, so nothing here returns an error a caller has to
//! handle to keep running a pipeline.
//!
//! It also outlives the run records. Retention pruning
//! ([`crate::engine::cleanup`]) deletes the heavy JSON but calls
//! [`forget_payload`], which keeps the summary and marks it `logs_pruned`.
//! That is what makes 30-day trends possible under a 50-run history limit.
//! An explicit user delete ([`crate::engine::persistence::delete_run`])
//! removes both, because a deleted run should vanish from metrics too.
//!
//! Stored as a map keyed by run id rather than a list: upsert and delete are
//! then a single `HashMap` operation with no duplicate-id risk, and the map
//! is sorted once on read (which every caller wants anyway).

use crate::engine::models::{PipelineRun, RollbackOutcome, RunKind, RunStatus, StageStatus};
use crate::engine::persistence::{data_dir, load_runs, runs_dir};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// Process-wide lock serializing read-modify-write of `runs-index.json`.
/// Mirrors the `mutate_projects` pattern in `persistence`.
static INDEX_LOCK: Mutex<()> = Mutex::new(());

/// One stage of a run, without its output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageSummary {
    pub name: String,
    pub status: StageStatus,
    pub duration_ms: Option<u64>,
    /// How many attempts the stage took. `> 1` means it was retried, which is
    /// how a stage that only passes on the second try is detected.
    pub attempts: Option<u32>,
}

/// Everything the insights engine knows about one run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSummary {
    pub id: String,
    pub pipeline_name: String,
    pub repo_path: String,
    pub environment: Option<String>,
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub status: RunStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<u64>,
    pub run_kind: RunKind,
    pub trigger_id: Option<String>,
    pub health_failure_stage: Option<String>,
    pub rollback_outcome: Option<RollbackOutcome>,
    pub auto_rollback_of: Option<String>,
    pub stages: Vec<StageSummary>,
    /// The full run record was pruned by retention; only this summary remains.
    /// The UI must not offer a "view logs" link for these.
    #[serde(default)]
    pub logs_pruned: bool,
}

impl RunSummary {
    /// Project a run onto its summary, dropping stdout/stderr and the
    /// pipeline snapshot.
    pub fn from_run(run: &PipelineRun) -> Self {
        Self {
            id: run.id.clone(),
            pipeline_name: run.pipeline_name.clone(),
            repo_path: run.repo_path.clone(),
            environment: run.environment.clone(),
            branch: run.branch.clone(),
            commit: run.commit.clone(),
            status: run.status.clone(),
            started_at: run.started_at,
            finished_at: run.finished_at,
            duration_ms: run.duration_ms,
            run_kind: run.run_kind,
            trigger_id: run.trigger_id.clone(),
            health_failure_stage: run.health_failure_stage.clone(),
            rollback_outcome: run.rollback_outcome,
            auto_rollback_of: run.auto_rollback_of.clone(),
            stages: run
                .stage_results
                .iter()
                .map(|s| StageSummary {
                    name: s.stage_name.clone(),
                    status: s.status.clone(),
                    duration_ms: s.duration_ms,
                    attempts: s.attempts,
                })
                .collect(),
            logs_pruned: false,
        }
    }
}

/// The on-disk index: run id -> summary.
pub type RunIndex = HashMap<String, RunSummary>;

/// Size of the index on disk, for the cleanup card and `chibby doctor`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    pub entries: u32,
    /// Entries whose run JSON has been pruned (summary-only history).
    pub payloads_pruned: u32,
    pub bytes: u64,
}

fn index_file() -> Result<PathBuf> {
    Ok(data_dir()?.join("runs-index.json"))
}

/// Read the index verbatim. `None` when it is missing, unreadable or corrupt —
/// all three mean "rebuild", never "fail".
fn read_index() -> Option<RunIndex> {
    let path = index_file().ok()?;
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

fn write_index(index: &RunIndex) -> Result<()> {
    let path = index_file()?;
    std::fs::write(&path, serde_json::to_string(index)?)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

/// Atomically load, mutate and persist the index under the process lock.
fn mutate_index<F, R>(f: F) -> Result<R>
where
    F: FnOnce(&mut RunIndex) -> R,
{
    let _guard = INDEX_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut index = read_index().unwrap_or_default();
    let result = f(&mut index);
    write_index(&index)?;
    Ok(result)
}

/// How many run records exist on disk.
fn run_file_count() -> Result<usize> {
    let dir = runs_dir()?;
    if !dir.exists() {
        return Ok(0);
    }
    Ok(std::fs::read_dir(&dir)?
        .flatten()
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
        .count())
}

// ---------------------------------------------------------------------------
// Mutation
// ---------------------------------------------------------------------------

/// Record (or refresh) one run's summary.
pub fn upsert(run: &PipelineRun) -> Result<()> {
    let summary = RunSummary::from_run(run);
    mutate_index(|index| {
        index.insert(summary.id.clone(), summary);
    })
}

/// Forget a run entirely — used when the user deletes it.
pub fn remove(run_id: &str) -> Result<()> {
    mutate_index(|index| {
        index.remove(run_id);
    })
}

/// Keep the summary but mark its logs gone — used by retention pruning.
pub fn forget_payload(run_id: &str) -> Result<()> {
    mutate_index(|index| {
        if let Some(entry) = index.get_mut(run_id) {
            entry.logs_pruned = true;
        }
    })
}

/// Drop every summary belonging to one project. Returns how many were removed.
pub fn remove_for_project(repo_path: &str) -> Result<u32> {
    mutate_index(|index| {
        let before = index.len();
        index.retain(|_, s| s.repo_path != repo_path);
        (before - index.len()) as u32
    })
}

// ---------------------------------------------------------------------------
// Reads
// ---------------------------------------------------------------------------

/// Every summary, newest first. Rebuilds transparently when the index is
/// missing, corrupt, or behind the runs directory.
pub fn load() -> Result<Vec<RunSummary>> {
    let index = load_or_rebuild()?;
    let mut summaries: Vec<RunSummary> = index.into_values().collect();
    summaries.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(summaries)
}

/// Summaries for one project, newest first.
pub fn load_for_project(repo_path: &str) -> Result<Vec<RunSummary>> {
    Ok(load()?
        .into_iter()
        .filter(|s| s.repo_path == repo_path)
        .collect())
}

/// Entry count and on-disk size, so the index's growth is visible before it
/// becomes a problem.
pub fn stats() -> Result<IndexStats> {
    let index = load_or_rebuild()?;
    let bytes = index_file()
        .ok()
        .and_then(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .unwrap_or(0);
    Ok(IndexStats {
        entries: index.len() as u32,
        payloads_pruned: index.values().filter(|s| s.logs_pruned).count() as u32,
        bytes,
    })
}

/// The stored index, or a freshly rebuilt one when it can't be trusted.
fn load_or_rebuild() -> Result<RunIndex> {
    let Some(index) = read_index() else {
        log::info!("Run index missing or unreadable — rebuilding from runs/");
        return rebuild_index();
    };

    // Entries whose payload is gone are legitimate history, so only the ones
    // still backed by a file are comparable with the runs directory.
    let backed = index.values().filter(|s| !s.logs_pruned).count();
    let files = run_file_count().unwrap_or(backed);
    if backed >= files {
        return Ok(index);
    }

    log::info!("Run index behind runs/ ({backed} indexed vs {files} files) — rebuilding");
    rebuild_index()
}

// ---------------------------------------------------------------------------
// Rebuild & retention
// ---------------------------------------------------------------------------

/// Regenerate the index from `runs/`. Returns the resulting entry count.
pub fn rebuild() -> Result<usize> {
    Ok(rebuild_index()?.len())
}

/// Union rather than replace: a summary whose run JSON was pruned by retention
/// is real history the runs directory can no longer produce, so a rebuild must
/// keep it (flagged `logs_pruned`) instead of silently shrinking the metrics.
fn rebuild_index() -> Result<RunIndex> {
    let runs = load_runs()?;
    let _guard = INDEX_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut index = read_index().unwrap_or_default();
    for entry in index.values_mut() {
        entry.logs_pruned = true;
    }
    for run in &runs {
        index.insert(run.id.clone(), RunSummary::from_run(run));
    }

    write_index(&index)?;
    log::info!("Rebuilt run index: {} entries", index.len());
    Ok(index)
}

/// Apply the index's own retention bounds. Both are "0 means unlimited", and
/// whichever bites first wins: entries older than `retention_days` go, then
/// the oldest entries beyond `max_entries`. Returns how many were dropped.
///
/// An entry leaving the index takes any surviving run record with it. The
/// index bounds are far wider than run retention, so in practice the record
/// is long gone — but keeping "every run file has an index entry" true is
/// what lets [`load`] detect drift by counting instead of parsing.
pub fn prune(retention_days: u32, max_entries: u32) -> Result<u32> {
    let dropped = mutate_index(|index| drop_beyond_bounds(index, retention_days, max_entries))?;

    for id in &dropped {
        if let Err(e) = crate::engine::persistence::remove_run_file(id) {
            log::warn!("Failed to remove pruned run record {id}: {e}");
        }
    }

    Ok(dropped.len() as u32)
}

/// Remove out-of-bounds entries from `index`, returning their ids.
fn drop_beyond_bounds(index: &mut RunIndex, retention_days: u32, max_entries: u32) -> Vec<String> {
    let mut dropped = Vec::new();

    if retention_days > 0 {
        let cutoff = Utc::now() - Duration::days(retention_days as i64);
        index.retain(|id, summary| {
            let keep = summary.started_at >= cutoff;
            if !keep {
                dropped.push(id.clone());
            }
            keep
        });
    }

    if max_entries > 0 && index.len() > max_entries as usize {
        let mut by_age: Vec<(String, DateTime<Utc>)> = index
            .iter()
            .map(|(id, s)| (id.clone(), s.started_at))
            .collect();
        // Newest first, so everything past the limit is the oldest.
        by_age.sort_by(|a, b| b.1.cmp(&a.1));
        for (id, _) in by_age.iter().skip(max_entries as usize) {
            index.remove(id);
            dropped.push(id.clone());
        }
    }

    dropped
}

/// How many entries [`prune`] would drop, without touching anything.
pub fn prune_preview(retention_days: u32, max_entries: u32) -> Result<u32> {
    let index = load_or_rebuild()?;
    let total = index.len();

    let dropped_by_age = match retention_days {
        0 => 0,
        days => {
            let cutoff = Utc::now() - Duration::days(days as i64);
            index.values().filter(|s| s.started_at < cutoff).count()
        }
    };

    // The cap always removes the oldest of whatever the age bound left.
    let remaining = total - dropped_by_age;
    let dropped_by_cap = match max_entries {
        0 => 0,
        cap => remaining.saturating_sub(cap as usize),
    };

    Ok((dropped_by_age + dropped_by_cap) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::models::StageResult;
    use crate::engine::persistence::{self, scoped_test_data_dir};

    const REPO: &str = "/tmp/chibby-index";

    fn run_with_logs(id: &str, repo_path: &str, minutes_ago: i64) -> PipelineRun {
        let mut run = PipelineRun::new_with_id(id, "ci", repo_path, Some("prod".to_string()));
        run.status = RunStatus::Success;
        run.started_at = Utc::now() - Duration::minutes(minutes_ago);
        run.duration_ms = Some(1_234);
        run.stage_results = vec![StageResult {
            stage_name: "test".to_string(),
            status: StageStatus::Success,
            exit_code: Some(0),
            stdout: "SECRET-LOG-MARKER".repeat(100),
            stderr: "STDERR-MARKER".to_string(),
            started_at: None,
            finished_at: None,
            duration_ms: Some(900),
            health_check_passed: None,
            attempts: Some(2),
            skip_reason: None,
        }];
        run
    }

    fn save(id: &str, minutes_ago: i64) -> PipelineRun {
        let run = run_with_logs(id, REPO, minutes_ago);
        persistence::save_run(&run).unwrap();
        run
    }

    #[test]
    fn test_from_run_keeps_the_shape_and_drops_the_logs() {
        let run = run_with_logs("r1", REPO, 0);

        let summary = RunSummary::from_run(&run);
        let json = serde_json::to_string(&summary).unwrap();

        assert_eq!(summary.id, "r1");
        assert_eq!(summary.environment.as_deref(), Some("prod"));
        assert_eq!(summary.stages.len(), 1);
        assert_eq!(summary.stages[0].attempts, Some(2));
        assert_eq!(summary.stages[0].duration_ms, Some(900));
        assert!(!summary.logs_pruned);
        assert!(!json.contains("SECRET-LOG-MARKER"));
        assert!(!json.contains("STDERR-MARKER"));
    }

    #[test]
    fn test_upsert_remove_and_remove_for_project_round_trip() {
        let (_dir, _lock) = scoped_test_data_dir();
        let a = run_with_logs("a", REPO, 1);
        let b = run_with_logs("b", "/tmp/other", 2);
        upsert(&a).unwrap();
        upsert(&b).unwrap();

        assert_eq!(load().unwrap().len(), 2);
        assert_eq!(load_for_project(REPO).unwrap().len(), 1);

        remove("a").unwrap();
        assert!(load_for_project(REPO).unwrap().is_empty());

        assert_eq!(remove_for_project("/tmp/other").unwrap(), 1);
        assert!(load().unwrap().is_empty());
    }

    #[test]
    fn test_load_sorts_newest_first() {
        let (_dir, _lock) = scoped_test_data_dir();
        save("old", 60);
        save("new", 1);

        let ids: Vec<String> = load().unwrap().into_iter().map(|s| s.id).collect();

        assert_eq!(ids, vec!["new".to_string(), "old".to_string()]);
    }

    #[test]
    fn test_missing_index_is_rebuilt_from_the_runs_directory() {
        let (dir, _lock) = scoped_test_data_dir();
        save("a", 1);
        save("b", 2);
        // Simulate an existing install that predates the index.
        std::fs::remove_file(dir.path().join("runs-index.json")).unwrap();

        let summaries = load().unwrap();

        assert_eq!(summaries.len(), 2);
        assert!(dir.path().join("runs-index.json").exists());
    }

    #[test]
    fn test_corrupt_index_rebuilds_instead_of_erroring() {
        let (dir, _lock) = scoped_test_data_dir();
        save("a", 1);
        std::fs::write(dir.path().join("runs-index.json"), "{not json").unwrap();

        let summaries = load().unwrap();

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, "a");
    }

    #[test]
    fn test_save_run_keeps_the_index_in_step() {
        let (_dir, _lock) = scoped_test_data_dir();
        let mut run = save("a", 1);

        assert_eq!(load().unwrap()[0].status, RunStatus::Success);

        run.status = RunStatus::Failed;
        persistence::save_run(&run).unwrap();

        assert_eq!(load().unwrap()[0].status, RunStatus::Failed);
    }

    #[test]
    fn test_delete_run_removes_both_but_pruning_keeps_the_summary() {
        let (_dir, _lock) = scoped_test_data_dir();
        save("deleted", 1);
        save("pruned", 2);

        persistence::delete_run("deleted").unwrap();
        persistence::prune_run_payload("pruned").unwrap();

        let summaries = load().unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, "pruned");
        assert!(summaries[0].logs_pruned);
        assert!(persistence::load_run("pruned").unwrap().is_none());
    }

    #[test]
    fn test_rebuild_keeps_summaries_whose_run_json_was_pruned() {
        let (_dir, _lock) = scoped_test_data_dir();
        save("kept", 1);
        save("pruned", 2);
        persistence::prune_run_payload("pruned").unwrap();

        let entries = rebuild().unwrap();

        let summaries = load().unwrap();
        assert_eq!(entries, 2);
        assert_eq!(summaries.len(), 2);
        let pruned = summaries.iter().find(|s| s.id == "pruned").unwrap();
        assert!(pruned.logs_pruned);
        let kept = summaries.iter().find(|s| s.id == "kept").unwrap();
        assert!(!kept.logs_pruned);
    }

    #[test]
    fn test_prune_by_max_entries_keeps_the_newest() {
        let (_dir, _lock) = scoped_test_data_dir();
        save("newest", 1);
        save("middle", 10);
        save("oldest", 100);

        assert_eq!(prune_preview(0, 2).unwrap(), 1);
        assert_eq!(prune(0, 2).unwrap(), 1);

        let ids: Vec<String> = load().unwrap().into_iter().map(|s| s.id).collect();
        assert_eq!(ids, vec!["newest".to_string(), "middle".to_string()]);
    }

    #[test]
    fn test_prune_by_retention_days_drops_only_old_entries() {
        let (_dir, _lock) = scoped_test_data_dir();
        save("recent", 60);
        save("ancient", 60 * 24 * 40);

        assert_eq!(prune_preview(30, 0).unwrap(), 1);
        assert_eq!(prune(30, 0).unwrap(), 1);

        let ids: Vec<String> = load().unwrap().into_iter().map(|s| s.id).collect();
        assert_eq!(ids, vec!["recent".to_string()]);
    }

    #[test]
    fn test_prune_with_both_bounds_disabled_keeps_everything() {
        let (_dir, _lock) = scoped_test_data_dir();
        save("a", 1);
        save("b", 60 * 24 * 400);

        assert_eq!(prune_preview(0, 0).unwrap(), 0);
        assert_eq!(prune(0, 0).unwrap(), 0);
        assert_eq!(load().unwrap().len(), 2);
    }

    #[test]
    fn test_stats_report_entries_and_pruned_payloads() {
        let (_dir, _lock) = scoped_test_data_dir();
        save("a", 1);
        save("b", 2);
        persistence::prune_run_payload("b").unwrap();

        let stats = stats().unwrap();

        assert_eq!(stats.entries, 2);
        assert_eq!(stats.payloads_pruned, 1);
        assert!(stats.bytes > 0);
    }
}
