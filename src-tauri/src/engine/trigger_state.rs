//! Per-trigger bookkeeping: when each trigger last fired, and why it didn't.
//!
//! Stored in `<data_dir>/trigger_state.json` (not in the repo) because it is
//! machine-local runtime state, and shared by the GUI and the CLI. Mirrors the
//! `mutate_projects` + static-Mutex pattern in `persistence`.

use crate::engine::persistence::data_dir;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// Process-wide lock serializing read-modify-write of `trigger_state.json`.
static TRIGGER_STATE_LOCK: Mutex<()> = Mutex::new(());

/// What happened last time a trigger was evaluated.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TriggerStateEntry {
    #[serde(default)]
    pub last_fired_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub last_run_id: Option<String>,
    #[serde(default)]
    pub last_skip_reason: Option<String>,
}

/// Keyed `"<repo_path>|<trigger_id>"`.
pub type TriggerStateMap = HashMap<String, TriggerStateEntry>;

/// The key one trigger is stored under.
pub fn state_key(repo_path: &str, trigger_id: &str) -> String {
    format!("{repo_path}|{trigger_id}")
}

fn state_file() -> Result<PathBuf> {
    Ok(data_dir()?.join("trigger_state.json"))
}

/// Load all trigger state. A corrupt file reads as empty rather than blocking
/// every trigger on the machine.
pub fn load_trigger_state() -> Result<TriggerStateMap> {
    let path = state_file()?;
    if !path.exists() {
        return Ok(TriggerStateMap::new());
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    Ok(serde_json::from_str(&content).unwrap_or_default())
}

/// Overwrite all trigger state.
pub fn save_trigger_state(state: &TriggerStateMap) -> Result<()> {
    let path = state_file()?;
    std::fs::write(&path, serde_json::to_string_pretty(state)?)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

/// Atomically load, mutate and persist trigger state under the process lock.
pub fn mutate_trigger_state<F, R>(f: F) -> Result<R>
where
    F: FnOnce(&mut TriggerStateMap) -> R,
{
    let _guard = TRIGGER_STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut state = load_trigger_state()?;
    let result = f(&mut state);
    save_trigger_state(&state)?;
    Ok(result)
}

/// State for one trigger.
pub fn get_trigger_state(repo_path: &str, trigger_id: &str) -> Result<Option<TriggerStateEntry>> {
    Ok(load_trigger_state()?
        .get(&state_key(repo_path, trigger_id))
        .cloned())
}

/// Every entry belonging to one repo, keyed by trigger id.
pub fn trigger_state_for_repo(repo_path: &str) -> Result<HashMap<String, TriggerStateEntry>> {
    let prefix = format!("{repo_path}|");
    Ok(load_trigger_state()?
        .into_iter()
        .filter_map(|(key, entry)| key.strip_prefix(&prefix).map(|id| (id.to_string(), entry)))
        .collect())
}

/// Record a fire time. Called *before* the run starts, so a crash mid-run
/// cannot re-fire the same occurrence on restart.
pub fn record_fired(repo_path: &str, trigger_id: &str, at: DateTime<Utc>) -> Result<()> {
    mutate_trigger_state(|state| {
        let entry = state.entry(state_key(repo_path, trigger_id)).or_default();
        entry.last_fired_at = Some(at);
        entry.last_skip_reason = None;
    })
}

/// Attach the run id once the run has one.
pub fn record_run_id(repo_path: &str, trigger_id: &str, run_id: &str) -> Result<()> {
    mutate_trigger_state(|state| {
        state
            .entry(state_key(repo_path, trigger_id))
            .or_default()
            .last_run_id = Some(run_id.to_string());
    })
}

/// Move the baseline forward without running, recording why.
pub fn record_skip(
    repo_path: &str,
    trigger_id: &str,
    at: DateTime<Utc>,
    reason: &str,
) -> Result<()> {
    mutate_trigger_state(|state| {
        let entry = state.entry(state_key(repo_path, trigger_id)).or_default();
        entry.last_fired_at = Some(at);
        entry.last_skip_reason = Some(reason.to_string());
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::persistence::scoped_test_data_dir;

    const REPO: &str = "/tmp/chibby-trigger-state";

    #[test]
    fn test_missing_state_reads_as_empty() {
        let (_dir, _guard) = scoped_test_data_dir();

        assert!(load_trigger_state().unwrap().is_empty());
        assert!(get_trigger_state(REPO, "nightly").unwrap().is_none());
    }

    #[test]
    fn test_fired_then_skip_round_trips() {
        let (_dir, _guard) = scoped_test_data_dir();
        let at = Utc::now();

        record_fired(REPO, "nightly", at).unwrap();
        record_run_id(REPO, "nightly", "run-1").unwrap();

        let entry = get_trigger_state(REPO, "nightly").unwrap().unwrap();
        assert_eq!(entry.last_fired_at, Some(at));
        assert_eq!(entry.last_run_id.as_deref(), Some("run-1"));
        assert!(entry.last_skip_reason.is_none());

        record_skip(REPO, "nightly", at, "busy").unwrap();
        let entry = get_trigger_state(REPO, "nightly").unwrap().unwrap();
        assert_eq!(entry.last_skip_reason.as_deref(), Some("busy"));
        // The run id survives a skip so the UI can still link the last run.
        assert_eq!(entry.last_run_id.as_deref(), Some("run-1"));
    }

    #[test]
    fn test_state_is_scoped_per_repo_and_trigger() {
        let (_dir, _guard) = scoped_test_data_dir();
        let at = Utc::now();

        record_fired(REPO, "nightly", at).unwrap();
        record_fired(REPO, "hourly", at).unwrap();
        record_fired("/tmp/other-repo", "nightly", at).unwrap();

        let mine = trigger_state_for_repo(REPO).unwrap();

        assert_eq!(mine.len(), 2);
        assert!(mine.contains_key("nightly"));
        assert!(mine.contains_key("hourly"));
    }

    #[test]
    fn test_corrupt_state_file_reads_as_empty() {
        let (_dir, _guard) = scoped_test_data_dir();
        std::fs::write(state_file().unwrap(), "{ not json").unwrap();

        assert!(load_trigger_state().unwrap().is_empty());
    }
}
