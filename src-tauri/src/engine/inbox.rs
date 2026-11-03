//! Notification inbox: one JSON file per entry under the data directory.
//!
//! The CLI and the app are separate processes, and in-process locks cannot
//! protect a shared file between them. One file per entry means concurrent
//! writers never touch the same data.

use crate::engine::models::{
    NotificationEntry, NotificationKind, NotificationSeverity, NotificationSummary, PipelineRun,
    RunStatus, StageStatus,
};
use crate::engine::notify::{self, DeliveryPlan};
use crate::engine::persistence;
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Duration, Utc};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Pruning starts above this many entries...
const MAX_ENTRIES: usize = 550;
/// ...and removes the oldest down to this many.
const KEEP_ENTRIES: usize = 500;
/// Read entries older than this are dropped when the app starts.
const READ_RETENTION_DAYS: i64 = 30;

/// Serializes read-modify-write within this process.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

pub fn inbox_dir() -> Result<PathBuf> {
    let dir = persistence::data_dir()?.join("notifications");
    std::fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    Ok(dir)
}

/// Ids arrive from the frontend and become file names.
fn entry_path(dir: &Path, id: &str) -> Result<PathBuf> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        bail!("Invalid notification id '{id}'");
    }
    Ok(dir.join(format!("{id}.json")))
}

/// Write via a temp file and rename, so a reader never sees half an entry.
fn write_entry(dir: &Path, entry: &NotificationEntry) -> Result<()> {
    let path = entry_path(dir, &entry.id)?;
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&tmp, serde_json::to_string_pretty(entry)?)
        .with_context(|| format!("Failed to write {}", tmp.display()))?;
    std::fs::rename(&tmp, &path).with_context(|| format!("Failed to replace {}", path.display()))
}

/// Entry files, oldest first: names start with creation millis.
fn entry_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    Ok(files)
}

/// Entries newest first. Unreadable files are skipped: one bad entry must not
/// hide the rest.
pub fn list() -> Result<Vec<NotificationEntry>> {
    let mut entries: Vec<NotificationEntry> = entry_files(&inbox_dir()?)?
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .filter_map(|content| serde_json::from_str(&content).ok())
        .collect();
    entries.reverse();
    Ok(entries)
}

pub fn summarize(entries: &[NotificationEntry]) -> NotificationSummary {
    let unread = entries.iter().filter(|e| e.read_at.is_none());
    NotificationSummary {
        unread: unread.clone().count() as u32,
        unread_failures: unread
            .filter(|e| e.severity == NotificationSeverity::Failure)
            .count() as u32,
    }
}

/// Add an entry to the inbox.
pub fn record(entry: &NotificationEntry) -> Result<()> {
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = inbox_dir()?;
    remove_superseded(&dir, entry)?;
    write_entry(&dir, entry)?;
    prune_to_cap(&dir)
}

/// Record, logging on failure: the inbox must never fail the work it reports.
pub fn record_best_effort(entry: &NotificationEntry) {
    if let Err(e) = record(entry) {
        log::warn!("Failed to record notification '{}': {e}", entry.title);
    }
}

/// A busy file watch would otherwise bury failures under unread successes, so
/// an unattended success replaces the previous unread one from its trigger.
fn remove_superseded(dir: &Path, entry: &NotificationEntry) -> Result<()> {
    let unattended = entry.run_kind.is_some_and(|k| k.is_unattended());
    if !unattended || entry.severity != NotificationSeverity::Success || entry.trigger_id.is_none()
    {
        return Ok(());
    }

    for old in list()? {
        if old.read_at.is_none()
            && old.severity == NotificationSeverity::Success
            && old.trigger_id == entry.trigger_id
            && old.repo_path == entry.repo_path
        {
            let _ = std::fs::remove_file(entry_path(dir, &old.id)?);
        }
    }
    Ok(())
}

fn prune_to_cap(dir: &Path) -> Result<()> {
    let files = entry_files(dir)?;
    if files.len() <= MAX_ENTRIES {
        return Ok(());
    }
    for path in &files[..files.len() - KEEP_ENTRIES] {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

/// Drop read entries past their retention. Unread ones stay until seen.
pub fn prune_expired(now: DateTime<Utc>) -> Result<()> {
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = inbox_dir()?;
    let cutoff = now - Duration::days(READ_RETENTION_DAYS);
    for entry in list()? {
        if entry.read_at.is_some_and(|read_at| read_at < cutoff) {
            let _ = std::fs::remove_file(entry_path(&dir, &entry.id)?);
        }
    }
    Ok(())
}

/// Mark every unread entry matching `predicate` as read.
fn mark_read_where(predicate: impl Fn(&NotificationEntry) -> bool) -> Result<()> {
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = inbox_dir()?;
    let now = Utc::now();
    for mut entry in list()? {
        if entry.read_at.is_none() && predicate(&entry) {
            entry.read_at = Some(now);
            write_entry(&dir, &entry)?;
        }
    }
    Ok(())
}

pub fn mark_read(id: &str) -> Result<()> {
    entry_path(&inbox_dir()?, id)?;
    mark_read_where(|entry| entry.id == id)
}

pub fn mark_all_read() -> Result<()> {
    mark_read_where(|_| true)
}

pub fn mark_run_read(run_id: &str) -> Result<()> {
    mark_read_where(|entry| entry.run_id.as_deref() == Some(run_id))
}

pub fn delete(id: &str) -> Result<()> {
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = entry_path(&inbox_dir()?, id)?;
    if path.exists() {
        std::fs::remove_file(&path).with_context(|| format!("Failed to delete {id}"))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Entries
// ---------------------------------------------------------------------------

/// The name the user gave the project, else its directory.
fn project_name(repo_path: &str) -> String {
    persistence::load_projects()
        .ok()
        .and_then(|projects| projects.into_iter().find(|p| p.path == repo_path))
        .map(|project| project.name)
        .or_else(|| {
            Path::new(repo_path)
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| repo_path.to_string())
}

/// The entry for a run that reached a terminal state.
pub fn finished_run_entry(run: &PipelineRun, plan: &DeliveryPlan) -> NotificationEntry {
    let (severity, outcome) = match run.status {
        RunStatus::Success => (NotificationSeverity::Success, "succeeded"),
        RunStatus::Failed => (NotificationSeverity::Failure, "failed"),
        RunStatus::Cancelled => (NotificationSeverity::Info, "cancelled"),
        _ => (NotificationSeverity::Info, "completed"),
    };
    let project = project_name(&run.repo_path);
    let failed_stage = run
        .stage_results
        .iter()
        .find(|stage| stage.status == StageStatus::Failed);

    let mut body = match failed_stage {
        Some(stage) => format!("Failed at stage '{}'", stage.stage_name),
        None => format!("Pipeline '{}' {outcome}", run.pipeline_name),
    };
    body.push_str(&notify::rollback_suffix(run.rollback_outcome));
    if let Some(ms) = run.duration_ms {
        body.push_str(&format!(" ({}s)", ms / 1000));
    }

    NotificationEntry {
        repo_path: Some(run.repo_path.clone()),
        run_id: Some(run.id.clone()),
        run_kind: Some(run.run_kind),
        trigger_id: run.trigger_id.clone(),
        pipeline_file: run.pipeline_file.clone(),
        alert: plan.alert,
        alert_delivered: !plan.gui_alerts,
        ..NotificationEntry::new(
            NotificationKind::RunFinished,
            severity,
            format!("{project}: {} {outcome}", run.pipeline_name),
            body,
        )
    }
    .with_project(project)
}

/// The entry for a run that never produced a run record.
pub fn pre_run_failure_entry(
    repo_path: &Path,
    run_kind: crate::engine::models::RunKind,
    trigger_id: Option<String>,
    pipeline_file: Option<String>,
    error: &str,
    plan: &DeliveryPlan,
) -> NotificationEntry {
    let repo_path = repo_path.to_string_lossy().to_string();
    let project = project_name(&repo_path);

    NotificationEntry {
        repo_path: Some(repo_path),
        run_kind: Some(run_kind),
        trigger_id,
        pipeline_file,
        alert: plan.alert,
        alert_delivered: !plan.gui_alerts,
        ..NotificationEntry::new(
            NotificationKind::PreRunFailure,
            NotificationSeverity::Failure,
            format!("{project}: run could not start"),
            error.to_string(),
        )
    }
    .with_project(project)
}

/// The entry for a run found still `Running` at startup. No OS alert: the app
/// window is opening at that moment, and the badge carries it.
pub fn interrupted_run_entry(run: &PipelineRun) -> NotificationEntry {
    let project = project_name(&run.repo_path);

    NotificationEntry {
        repo_path: Some(run.repo_path.clone()),
        run_id: Some(run.id.clone()),
        run_kind: Some(run.run_kind),
        trigger_id: run.trigger_id.clone(),
        pipeline_file: run.pipeline_file.clone(),
        ..NotificationEntry::new(
            NotificationKind::RunInterrupted,
            NotificationSeverity::Failure,
            format!("{project}: {} was interrupted", run.pipeline_name),
            "Chibby stopped while this run was in progress.".to_string(),
        )
    }
    .with_project(project)
}

impl NotificationEntry {
    fn with_project(mut self, project: String) -> Self {
        self.project_name = Some(project);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::models::{AlertPolicy, RollbackOutcome, RunKind};
    use crate::engine::persistence::scoped_test_data_dir;

    fn entry(severity: NotificationSeverity) -> NotificationEntry {
        NotificationEntry::new(
            NotificationKind::RunFinished,
            severity,
            "shop: deploy".to_string(),
            "body".to_string(),
        )
    }

    fn watch_success(trigger: &str) -> NotificationEntry {
        NotificationEntry {
            run_kind: Some(RunKind::Watch),
            trigger_id: Some(trigger.to_string()),
            repo_path: Some("/repo".to_string()),
            ..entry(NotificationSeverity::Success)
        }
    }

    #[test]
    fn test_round_trip_lists_newest_first() {
        let (_data, _guard) = scoped_test_data_dir();
        let mut older = entry(NotificationSeverity::Failure);
        older.id = format!("0000000000001-{}", older.id);
        let mut newer = entry(NotificationSeverity::Success);
        newer.id = format!("0000000000002-{}", newer.id);

        record(&older).unwrap();
        record(&newer).unwrap();

        let ids: Vec<String> = list().unwrap().into_iter().map(|e| e.id).collect();
        assert_eq!(ids, vec![newer.id, older.id]);
    }

    #[test]
    fn test_list_ignores_temp_and_corrupt_files() {
        let (_data, _guard) = scoped_test_data_dir();
        record(&entry(NotificationSeverity::Info)).unwrap();
        let dir = inbox_dir().unwrap();
        std::fs::write(dir.join("000.tmp-123"), "{").unwrap();
        std::fs::write(dir.join("001.json"), "not json").unwrap();

        assert_eq!(list().unwrap().len(), 1);
    }

    #[test]
    fn test_summary_counts_only_unread() {
        let mut read = entry(NotificationSeverity::Failure);
        read.read_at = Some(Utc::now());

        let summary = summarize(&[
            read,
            entry(NotificationSeverity::Failure),
            entry(NotificationSeverity::Success),
        ]);

        assert_eq!(
            summary,
            NotificationSummary {
                unread: 2,
                unread_failures: 1
            }
        );
    }

    #[test]
    fn test_mark_read_variants() {
        let (_data, _guard) = scoped_test_data_dir();
        let one = entry(NotificationSeverity::Failure);
        let mut two = entry(NotificationSeverity::Failure);
        two.run_id = Some("run-2".to_string());
        let three = entry(NotificationSeverity::Failure);
        for e in [&one, &two, &three] {
            record(e).unwrap();
        }
        let unread = || summarize(&list().unwrap()).unread;

        mark_read(&one.id).unwrap();
        assert_eq!(unread(), 2);
        mark_run_read("run-2").unwrap();
        assert_eq!(unread(), 1);
        mark_all_read().unwrap();
        assert_eq!(unread(), 0);
    }

    #[test]
    fn test_ids_cannot_escape_the_inbox() {
        let (data, _guard) = scoped_test_data_dir();
        let outside = data.path().join("settings.json");
        std::fs::write(&outside, "{}").unwrap();

        for id in ["../settings", "a/b", "a\\b", "..", ""] {
            assert!(delete(id).is_err(), "{id}");
            assert!(mark_read(id).is_err(), "{id}");
        }
        assert!(outside.exists());
    }

    #[test]
    fn test_delete_removes_entry_and_tolerates_missing() {
        let (_data, _guard) = scoped_test_data_dir();
        let one = entry(NotificationSeverity::Info);
        record(&one).unwrap();

        delete(&one.id).unwrap();
        delete(&one.id).unwrap();

        assert!(list().unwrap().is_empty());
    }

    #[test]
    fn test_cap_removes_oldest_files() {
        let (_data, _guard) = scoped_test_data_dir();
        let dir = inbox_dir().unwrap();
        for i in 0..MAX_ENTRIES {
            let mut old = entry(NotificationSeverity::Info);
            old.id = format!("{i:013}-old");
            write_entry(&dir, &old).unwrap();
        }

        let newest = entry(NotificationSeverity::Failure);
        record(&newest).unwrap();

        let entries = list().unwrap();
        assert_eq!(entries.len(), KEEP_ENTRIES);
        assert_eq!(entries[0].id, newest.id);
        assert!(!entries.iter().any(|e| e.id == "0000000000000-old"));
    }

    #[test]
    fn test_expiry_drops_only_old_read_entries() {
        let (_data, _guard) = scoped_test_data_dir();
        let now = Utc::now();
        let mut expired = entry(NotificationSeverity::Info);
        expired.read_at = Some(now - Duration::days(READ_RETENTION_DAYS + 1));
        let mut recent = entry(NotificationSeverity::Info);
        recent.read_at = Some(now - Duration::days(1));
        let unread = entry(NotificationSeverity::Info);
        for e in [&expired, &recent, &unread] {
            record(e).unwrap();
        }

        prune_expired(now).unwrap();

        let ids: Vec<String> = list().unwrap().into_iter().map(|e| e.id).collect();
        assert_eq!(ids.len(), 2);
        assert!(!ids.contains(&expired.id));
    }

    #[test]
    fn test_unattended_success_replaces_previous_unread_success() {
        let (_data, _guard) = scoped_test_data_dir();
        record(&watch_success("watch:a")).unwrap();
        let latest = watch_success("watch:a");
        record(&latest).unwrap();

        let entries = list().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, latest.id);
    }

    #[test]
    fn test_replacement_keeps_failures_read_entries_and_other_triggers() {
        let (_data, _guard) = scoped_test_data_dir();
        let failure = NotificationEntry {
            severity: NotificationSeverity::Failure,
            ..watch_success("watch:a")
        };
        let mut read = watch_success("watch:a");
        read.read_at = Some(Utc::now());
        for e in [&failure, &read, &watch_success("watch:b")] {
            record(e).unwrap();
        }

        record(&watch_success("watch:a")).unwrap();

        assert_eq!(list().unwrap().len(), 4);
    }

    #[test]
    fn test_attended_successes_are_all_kept() {
        let (_data, _guard) = scoped_test_data_dir();
        for _ in 0..2 {
            record(&NotificationEntry {
                run_kind: Some(RunKind::Normal),
                ..watch_success("hook:pre-push")
            })
            .unwrap();
        }

        assert_eq!(list().unwrap().len(), 2);
    }

    #[test]
    fn test_finished_run_entry_reports_outcome_and_links_run() {
        let (_data, _guard) = scoped_test_data_dir();
        let mut run = PipelineRun::new("deploy", "/repos/shop", None);
        run.status = RunStatus::Failed;
        run.rollback_outcome = Some(RollbackOutcome::Succeeded);
        run.duration_ms = Some(2500);

        let built = finished_run_entry(
            &run,
            &DeliveryPlan {
                alert: AlertPolicy::WhenAway,
                webhooks: Vec::new(),
                gui_alerts: true,
            },
        );

        assert_eq!(built.title, "shop: deploy failed");
        assert_eq!(
            built.body,
            "Pipeline 'deploy' failed — auto-rolled back, healthy again (2s)"
        );
        assert_eq!(built.severity, NotificationSeverity::Failure);
        assert_eq!(built.run_id.as_deref(), Some(run.id.as_str()));
        assert_eq!(built.project_name.as_deref(), Some("shop"));
        assert!(!built.alert_delivered);
    }

    #[test]
    fn test_interrupted_entry_never_alerts() {
        let (_data, _guard) = scoped_test_data_dir();
        let run = PipelineRun::new("deploy", "/repos/shop", None);

        let built = interrupted_run_entry(&run);

        assert_eq!(built.alert, AlertPolicy::None);
        assert_eq!(built.severity, NotificationSeverity::Failure);
    }
}
