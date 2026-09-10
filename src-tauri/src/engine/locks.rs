//! Cross-process run lock and cancel flag.
//!
//! `state.rs` only knows about runs inside the current process. Triggers break
//! that assumption: with the desktop app open and a headless `chibby schedule
//! --once` firing from launchd, the same repo would otherwise run twice at
//! once. The lock lives in the shared data directory, so every Chibby process
//! sees it.
//!
//! Re-exported from `persistence` so callers can keep using
//! `persistence::acquire_run_lock`.

use crate::engine::persistence::data_dir;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Serializes acquisition inside this process; the lock file handles the rest.
static ACQUIRE_LOCK: Mutex<()> = Mutex::new(());

/// On platforms where liveness cannot be probed, a lock older than this is
/// treated as abandoned rather than blocking the repo forever.
const ASSUME_STALE_AFTER_HOURS: i64 = 12;

/// What a lock file records about the process holding it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunLockInfo {
    pub pid: u32,
    pub started_at: DateTime<Utc>,
    #[serde(default)]
    pub run_id: Option<String>,
}

/// A held run lock. Releases on drop.
#[derive(Debug)]
pub struct RunLock {
    path: PathBuf,
    repo_path: String,
}

impl RunLock {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn repo_path(&self) -> &str {
        &self.repo_path
    }

    /// Whether another process has asked this run to stop.
    pub fn cancel_requested(&self) -> bool {
        cancel_path(&self.path).exists()
    }
}

impl Drop for RunLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(cancel_path(&self.path));
        let _ = std::fs::remove_file(&self.path);
    }
}

fn locks_dir() -> Result<PathBuf> {
    let dir = data_dir()?.join("locks");
    std::fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    Ok(dir)
}

/// Lock file for a repo. Hashed so any path becomes a safe file name.
pub fn lock_path(repo_path: &str) -> Result<PathBuf> {
    let mut hasher = Sha256::new();
    hasher.update(repo_path.as_bytes());
    let hash: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok(locks_dir()?.join(format!("{hash}.lock")))
}

fn cancel_path(lock_path: &Path) -> PathBuf {
    lock_path.with_extension("cancel")
}

/// Take the run lock for `repo_path`, or `None` when another live process holds it.
pub fn acquire_run_lock(repo_path: &str) -> Result<Option<RunLock>> {
    acquire_run_lock_with_id(repo_path, None)
}

/// As [`acquire_run_lock`], recording the run id in the lock file.
pub fn acquire_run_lock_with_id(repo_path: &str, run_id: Option<&str>) -> Result<Option<RunLock>> {
    let _guard = ACQUIRE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = lock_path(repo_path)?;

    if let Some(holder) = read_lock(&path) {
        if is_holder_alive(&holder) {
            return Ok(None);
        }
        log::info!(
            "[lock] reclaiming stale lock for {repo_path} (pid {} is gone)",
            holder.pid
        );
        let _ = std::fs::remove_file(cancel_path(&path));
        let _ = std::fs::remove_file(&path);
    }

    let info = RunLockInfo {
        pid: std::process::id(),
        started_at: Utc::now(),
        run_id: run_id.map(str::to_string),
    };
    std::fs::write(&path, serde_json::to_string_pretty(&info)?)
        .with_context(|| format!("Failed to write {}", path.display()))?;

    Ok(Some(RunLock {
        path,
        repo_path: repo_path.to_string(),
    }))
}

/// The process currently running `repo_path`, if any.
pub fn current_holder(repo_path: &str) -> Result<Option<RunLockInfo>> {
    let path = lock_path(repo_path)?;
    Ok(read_lock(&path).filter(is_holder_alive))
}

/// A lock file whose JSON is unreadable is treated as absent — better to
/// reclaim it than to wedge the repo permanently.
fn read_lock(path: &Path) -> Option<RunLockInfo> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn is_holder_alive(info: &RunLockInfo) -> bool {
    // A process from an earlier boot can reuse a pid, so age-guard regardless.
    if Utc::now() - info.started_at > chrono::Duration::hours(ASSUME_STALE_AFTER_HOURS) {
        return false;
    }
    is_pid_alive(info.pid)
}

/// `kill -0` probes for existence without signalling. Shelling out avoids
/// pulling in a libc dependency for one syscall.
#[cfg(unix)]
fn is_pid_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(true)
}

/// Conservative elsewhere: assume the holder is alive and let the age guard
/// above release the lock eventually.
#[cfg(not(unix))]
fn is_pid_alive(_pid: u32) -> bool {
    true
}

// ---------------------------------------------------------------------------
// Cross-process cancellation
// ---------------------------------------------------------------------------

/// Ask whichever process is running `repo_path` to stop.
/// Returns false when nothing is running.
pub fn request_cancel(repo_path: &str) -> Result<bool> {
    let path = lock_path(repo_path)?;
    let Some(holder) = read_lock(&path).filter(is_holder_alive) else {
        return Ok(false);
    };
    std::fs::write(cancel_path(&path), holder.pid.to_string())
        .with_context(|| format!("Failed to write cancel flag for {repo_path}"))?;
    Ok(true)
}

/// Whether a cancel has been requested for `repo_path` by any process.
/// Polled by the executor alongside its in-process cancel flag.
pub fn cancel_requested(repo_path: &str) -> bool {
    lock_path(repo_path).is_ok_and(|p| cancel_path(&p).exists())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::persistence::scoped_test_data_dir;

    const REPO: &str = "/tmp/chibby-lock-test";

    #[test]
    fn test_lock_blocks_a_second_acquisition_and_releases_on_drop() {
        let (_dir, _guard) = scoped_test_data_dir();

        let held = acquire_run_lock(REPO).unwrap().expect("first acquire");
        assert!(acquire_run_lock(REPO).unwrap().is_none(), "double acquire");

        drop(held);
        assert!(acquire_run_lock(REPO).unwrap().is_some(), "not released");
    }

    #[test]
    fn test_different_repos_do_not_contend() {
        let (_dir, _guard) = scoped_test_data_dir();

        let _a = acquire_run_lock("/tmp/repo-a").unwrap().unwrap();
        assert!(acquire_run_lock("/tmp/repo-b").unwrap().is_some());
    }

    /// A crashed run leaves a lock behind; a dead pid must not wedge the repo.
    #[test]
    fn test_stale_pid_is_reclaimed() {
        let (_dir, _guard) = scoped_test_data_dir();
        let path = lock_path(REPO).unwrap();
        let dead = RunLockInfo {
            // Above the default pid_max on every supported platform.
            pid: 4_294_967_000,
            started_at: Utc::now(),
            run_id: Some("crashed".to_string()),
        };
        std::fs::write(&path, serde_json::to_string(&dead).unwrap()).unwrap();

        let lock = acquire_run_lock(REPO).unwrap();

        assert!(lock.is_some(), "stale lock was not reclaimed");
    }

    #[test]
    fn test_ancient_lock_is_reclaimed_even_for_a_live_pid() {
        let (_dir, _guard) = scoped_test_data_dir();
        let path = lock_path(REPO).unwrap();
        let ancient = RunLockInfo {
            pid: std::process::id(),
            started_at: Utc::now() - chrono::Duration::hours(ASSUME_STALE_AFTER_HOURS + 1),
            run_id: None,
        };
        std::fs::write(&path, serde_json::to_string(&ancient).unwrap()).unwrap();

        assert!(acquire_run_lock(REPO).unwrap().is_some());
    }

    #[test]
    fn test_lock_file_records_pid_and_run_id() {
        let (_dir, _guard) = scoped_test_data_dir();

        let lock = acquire_run_lock_with_id(REPO, Some("run-42"))
            .unwrap()
            .unwrap();
        let info = read_lock(lock.path()).unwrap();

        assert_eq!(info.pid, std::process::id());
        assert_eq!(info.run_id.as_deref(), Some("run-42"));
        assert_eq!(
            current_holder(REPO).unwrap().map(|h| h.pid),
            Some(std::process::id())
        );
    }

    #[test]
    fn test_cancel_flag_round_trip() {
        let (_dir, _guard) = scoped_test_data_dir();
        assert!(!cancel_requested(REPO));
        assert!(!request_cancel(REPO).unwrap(), "no run to cancel");

        let lock = acquire_run_lock(REPO).unwrap().unwrap();
        assert!(request_cancel(REPO).unwrap());
        assert!(cancel_requested(REPO));
        assert!(lock.cancel_requested());

        drop(lock);
        assert!(!cancel_requested(REPO), "cancel flag outlived the run");
    }
}
