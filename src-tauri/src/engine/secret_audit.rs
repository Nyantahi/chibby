//! Per-secret audit metadata.
//!
//! Distinct from the line-based `audit.rs` log: this module tracks the
//! lifecycle of each individual secret (last-set / last-deleted / set-count
//! / last-provenance). Stored as one JSON file per project under
//! `<chibby_data_dir>/secret_audit/<repo_hash>.json` so the data follows the
//! user's Chibby install, not the project repo.
//!
//! Failures are non-fatal — audit is observability, not gating. Callers
//! should ignore Err and continue with the underlying operation.

use crate::engine::persistence;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Schema version of the per-project audit file. Bump when the layout changes.
const SCHEMA_VERSION: u32 = 1;

/// Where the audit event came from.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    Cli,
    Gui,
    /// e.g. `Import { adapter: "vercel" }`
    Import {
        adapter: String,
    },
    Export,
    Unknown,
}

impl Provenance {
    pub fn label(&self) -> String {
        match self {
            Provenance::Cli => "cli".into(),
            Provenance::Gui => "gui".into(),
            Provenance::Import { adapter } => format!("import:{}", adapter),
            Provenance::Export => "export".into(),
            Provenance::Unknown => "unknown".into(),
        }
    }
}

/// One secret's tracked lifecycle. Key in `ProjectSecretAudit.entries` is
/// `<env_name>|<secret_name>` (matches the keychain account-key shape).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecretAudit {
    pub last_set: Option<DateTime<Utc>>,
    pub last_deleted: Option<DateTime<Utc>>,
    pub set_count: u32,
    pub delete_count: u32,
    pub last_provenance: Option<String>,
}

/// On-disk schema.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectSecretAudit {
    #[serde(default = "default_version")]
    pub version: u32,
    /// `{env}|{name}` -> metadata.
    #[serde(default)]
    pub entries: BTreeMap<String, SecretAudit>,
}

fn default_version() -> u32 {
    SCHEMA_VERSION
}

fn entry_key(env: &str, name: &str) -> String {
    format!("{}|{}", env, name)
}

fn project_hash(project_path: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(project_path.as_bytes());
    let digest = hasher.finalize();
    hex(&digest[..8]) // 16 chars — collision-safe enough for per-user audit
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        write!(s, "{:02x}", b).unwrap();
    }
    s
}

/// Test-only override for the audit base directory. Production code uses
/// `persistence::data_dir()`. A `Mutex` because cargo test runs in parallel.
#[cfg(test)]
static TEST_DATA_DIR: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

#[cfg(test)]
fn data_dir() -> Result<PathBuf> {
    if let Ok(guard) = TEST_DATA_DIR.lock() {
        if let Some(dir) = guard.as_ref() {
            return Ok(dir.clone());
        }
    }
    persistence::data_dir()
}

#[cfg(not(test))]
fn data_dir() -> Result<PathBuf> {
    persistence::data_dir()
}

/// Path to the per-project audit file. Creates parent directory on demand.
fn audit_path(project_path: &str) -> Result<PathBuf> {
    let dir = data_dir()?.join("secret_audit");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create {}", dir.display()))?;
    Ok(dir.join(format!("{}.json", project_hash(project_path))))
}

/// Load the audit file for a project. Returns empty schema if absent or
/// unreadable — failure here must never block the user's operation.
pub fn load_for_project(project_path: &str) -> Result<ProjectSecretAudit> {
    let path = audit_path(project_path)?;
    if !path.exists() {
        return Ok(ProjectSecretAudit {
            version: SCHEMA_VERSION,
            entries: BTreeMap::new(),
        });
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let parsed: ProjectSecretAudit = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(parsed)
}

fn save_for_project(project_path: &str, audit: &ProjectSecretAudit) -> Result<()> {
    let path = audit_path(project_path)?;
    let content =
        serde_json::to_string_pretty(audit).context("Failed to serialize secret audit")?;
    std::fs::write(&path, content)
        .with_context(|| format!("Failed to write {}", path.display()))?;

    // Owner-only on Unix; this includes provenance hints + counts, no values.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        let _ = std::fs::set_permissions(&path, perms);
    }
    Ok(())
}

/// Record a `set` (or rotate) operation for a secret.
pub fn record_set(
    project_path: &str,
    env: &str,
    name: &str,
    provenance: Provenance,
) -> Result<()> {
    let mut audit = load_for_project(project_path)?;
    let entry = audit.entries.entry(entry_key(env, name)).or_default();
    entry.last_set = Some(Utc::now());
    entry.set_count = entry.set_count.saturating_add(1);
    entry.last_provenance = Some(provenance.label());
    save_for_project(project_path, &audit)
}

/// Record a `delete` operation.
pub fn record_delete(
    project_path: &str,
    env: &str,
    name: &str,
    provenance: Provenance,
) -> Result<()> {
    let mut audit = load_for_project(project_path)?;
    let entry = audit.entries.entry(entry_key(env, name)).or_default();
    entry.last_deleted = Some(Utc::now());
    entry.delete_count = entry.delete_count.saturating_add(1);
    entry.last_provenance = Some(provenance.label());
    save_for_project(project_path, &audit)
}

/// Public lookup — caller passes (env, name) and gets the metadata snapshot.
pub fn get(project_path: &str, env: &str, name: &str) -> Result<Option<SecretAudit>> {
    let audit = load_for_project(project_path)?;
    Ok(audit.entries.get(&entry_key(env, name)).cloned())
}

/// Snake-cased filename for tests / docs that want to inspect the data dir.
pub fn audit_filename_for_test(project_path: &str) -> String {
    format!("{}.json", project_hash(project_path))
}

/// Best-effort recorder helpers — log warnings, never panic.
pub fn record_set_quietly(
    project_path: &str,
    env: &str,
    name: &str,
    provenance: Provenance,
) {
    if let Err(e) = record_set(project_path, env, name, provenance) {
        log::warn!(
            "Failed to record secret-set audit for {}/{}: {}",
            env,
            name,
            e
        );
    }
}

pub fn record_delete_quietly(
    project_path: &str,
    env: &str,
    name: &str,
    provenance: Provenance,
) {
    if let Err(e) = record_delete(project_path, env, name, provenance) {
        log::warn!(
            "Failed to record secret-delete audit for {}/{}: {}",
            env,
            name,
            e
        );
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Tests share process-level state (data_dir override + the audit files on
    /// disk), so serialize them with a mutex. We can't put each test in its own
    /// data dir because TEST_DATA_DIR is a process-wide override.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    /// Set up a fresh, isolated data dir, run `f`, then clean up. The `_lock`
    /// guard keeps other test threads waiting until this one is done.
    fn with_data_dir<F: FnOnce()>(f: F) {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        {
            let mut guard = TEST_DATA_DIR.lock().unwrap();
            *guard = Some(tmp.path().to_path_buf());
        }
        f();
        {
            let mut guard = TEST_DATA_DIR.lock().unwrap();
            *guard = None;
        }
        drop(tmp);
    }

    #[test]
    fn record_set_creates_entry_with_provenance() {
        with_data_dir(|| {
            record_set("/tmp/project_a", "production", "STRIPE", Provenance::Cli).unwrap();
            let snap = get("/tmp/project_a", "production", "STRIPE").unwrap().unwrap();
            assert_eq!(snap.set_count, 1);
            assert!(snap.last_set.is_some());
            assert_eq!(snap.last_provenance.as_deref(), Some("cli"));
        });
    }

    #[test]
    fn record_set_increments_count_on_repeat() {
        with_data_dir(|| {
            record_set("/tmp/project_a", "prod", "KEY", Provenance::Cli).unwrap();
            record_set("/tmp/project_a", "prod", "KEY", Provenance::Gui).unwrap();
            record_set("/tmp/project_a", "prod", "KEY", Provenance::Cli).unwrap();
            let snap = get("/tmp/project_a", "prod", "KEY").unwrap().unwrap();
            assert_eq!(snap.set_count, 3);
            assert_eq!(snap.last_provenance.as_deref(), Some("cli"));
        });
    }

    #[test]
    fn record_delete_tracks_separately() {
        with_data_dir(|| {
            record_set("/tmp/project_a", "prod", "K", Provenance::Cli).unwrap();
            record_delete("/tmp/project_a", "prod", "K", Provenance::Cli).unwrap();
            let snap = get("/tmp/project_a", "prod", "K").unwrap().unwrap();
            assert_eq!(snap.set_count, 1);
            assert_eq!(snap.delete_count, 1);
            assert!(snap.last_set.is_some());
            assert!(snap.last_deleted.is_some());
        });
    }

    #[test]
    fn import_provenance_includes_adapter() {
        with_data_dir(|| {
            record_set(
                "/tmp/project_a",
                "prod",
                "K",
                Provenance::Import {
                    adapter: "vercel".to_string(),
                },
            )
            .unwrap();
            let snap = get("/tmp/project_a", "prod", "K").unwrap().unwrap();
            assert_eq!(snap.last_provenance.as_deref(), Some("import:vercel"));
        });
    }

    #[test]
    fn different_projects_isolated() {
        with_data_dir(|| {
            record_set("/tmp/project_a", "prod", "K", Provenance::Cli).unwrap();
            assert!(get("/tmp/project_b", "prod", "K").unwrap().is_none());
        });
    }

    #[test]
    fn missing_entry_returns_none() {
        with_data_dir(|| {
            let snap = get("/tmp/never", "prod", "K").unwrap();
            assert!(snap.is_none());
        });
    }
}
