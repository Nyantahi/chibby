//! Local trigger configuration: schedules, file watches, and git hooks.
//!
//! Triggers live in `.chibby/triggers.toml` rather than `pipeline.toml`
//! deliberately. `pipeline.toml` is committed and shared with the team; a cron
//! entry in there would fire on every teammate's machine. Triggers are
//! per-machine policy, so they layer exactly like environments:
//! `triggers.toml` (committed, optional) as the base and
//! `triggers.local.toml` (gitignored, per-developer) on top.

pub mod hooks;
pub mod runner;
pub mod schedule;
pub mod watch;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const TRIGGERS_FILE: &str = "triggers.toml";
const TRIGGERS_LOCAL_FILE: &str = "triggers.local.toml";

fn default_true() -> bool {
    true
}

fn default_debounce() -> u64 {
    750
}

fn default_min_interval() -> u64 {
    10
}

/// What to do with schedule fire times that elapsed while Chibby was not running.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissedPolicy {
    /// Forget them. A laptop closed over the weekend simply misses the runs.
    #[default]
    Skip,
    /// Run once to catch up — never backfill one run per missed occurrence.
    RunOnce,
}

/// A cron-driven trigger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleTrigger {
    /// Stable key used for trigger state and the UI.
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Cron expression; 5-field (`0 3 * * *`) and 6/7-field forms both parse.
    pub cron: String,
    #[serde(default)]
    pub missed: MissedPolicy,
    #[serde(default)]
    pub pipeline_file: Option<String>,
    #[serde(default)]
    pub environment: Option<String>,
    /// Stage filter, fed straight to the existing stage filter. Empty = all.
    #[serde(default)]
    pub stages: Vec<String>,
}

/// A filesystem-watch trigger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchTrigger {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Globs relative to the repo root; empty means everything.
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Quiet period before a burst of events fires one run.
    #[serde(default = "default_debounce")]
    pub debounce_ms: u64,
    /// Floor between two runs of this trigger, so a build that writes into the
    /// repo cannot hot-loop.
    #[serde(default = "default_min_interval")]
    pub min_interval_secs: u64,
    #[serde(default)]
    pub pipeline_file: Option<String>,
    #[serde(default)]
    pub environment: Option<String>,
    #[serde(default)]
    pub stages: Vec<String>,
}

/// What a generated git hook should run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookSpec {
    #[serde(default)]
    pub stages: Vec<String>,
    #[serde(default)]
    pub pipeline_file: Option<String>,
    #[serde(default)]
    pub environment: Option<String>,
    /// Whether a failing run blocks the git operation.
    #[serde(default = "default_true")]
    pub blocking: bool,
}

impl Default for HookSpec {
    fn default() -> Self {
        Self {
            stages: Vec::new(),
            pipeline_file: None,
            environment: None,
            blocking: true,
        }
    }
}

/// Git hooks Chibby may manage for a repo.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HooksConfig {
    #[serde(default)]
    pub pre_push: Option<HookSpec>,
    #[serde(default)]
    pub pre_commit: Option<HookSpec>,
}

/// Everything in `.chibby/triggers.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TriggersConfig {
    /// Master switch. Off means no schedule ticks and no watchers for this repo.
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub schedules: Vec<ScheduleTrigger>,
    #[serde(default)]
    pub watches: Vec<WatchTrigger>,
    #[serde(default)]
    pub hooks: HooksConfig,
}

/// Path to the committed triggers file.
pub fn triggers_path(repo_path: &Path) -> PathBuf {
    repo_path.join(".chibby").join(TRIGGERS_FILE)
}

/// Path to the per-developer override file.
pub fn triggers_local_path(repo_path: &Path) -> PathBuf {
    repo_path.join(".chibby").join(TRIGGERS_LOCAL_FILE)
}

/// Load one triggers file, returning an empty config when it is absent.
fn load_file(path: &Path) -> Result<TriggersConfig> {
    if !path.exists() {
        return Ok(TriggersConfig::default());
    }
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))
}

/// Load `.chibby/triggers.toml` (committed base only).
pub fn load_triggers(repo_path: &Path) -> Result<TriggersConfig> {
    load_file(&triggers_path(repo_path))
}

/// Load `.chibby/triggers.local.toml` (per-developer overrides only).
pub fn load_triggers_local(repo_path: &Path) -> Result<TriggersConfig> {
    load_file(&triggers_local_path(repo_path))
}

/// Load triggers with per-developer overrides applied.
pub fn load_triggers_layered(repo_path: &Path) -> Result<TriggersConfig> {
    let base = load_triggers(repo_path)?;
    let local = load_triggers_local(repo_path)?;
    Ok(merge_triggers(base, local))
}

/// Pure merge — local wins over base.
///
/// `enabled` is OR'd so a developer can switch triggers on locally without
/// editing the committed file. Schedules and watches merge by `id` (local
/// replaces base wholesale; local-only ids are appended), and each hook slot is
/// taken from local when present.
pub fn merge_triggers(mut base: TriggersConfig, local: TriggersConfig) -> TriggersConfig {
    base.enabled = base.enabled || local.enabled;

    for schedule in local.schedules {
        match base.schedules.iter_mut().find(|s| s.id == schedule.id) {
            Some(existing) => *existing = schedule,
            None => base.schedules.push(schedule),
        }
    }
    for watch in local.watches {
        match base.watches.iter_mut().find(|w| w.id == watch.id) {
            Some(existing) => *existing = watch,
            None => base.watches.push(watch),
        }
    }
    if local.hooks.pre_push.is_some() {
        base.hooks.pre_push = local.hooks.pre_push;
    }
    if local.hooks.pre_commit.is_some() {
        base.hooks.pre_commit = local.hooks.pre_commit;
    }
    base
}

/// Write `.chibby/triggers.toml`.
pub fn save_triggers(repo_path: &Path, config: &TriggersConfig) -> Result<()> {
    write_triggers(&triggers_path(repo_path), repo_path, config)
}

/// Write `.chibby/triggers.local.toml` and keep it out of git.
pub fn save_triggers_local(repo_path: &Path, config: &TriggersConfig) -> Result<()> {
    write_triggers(&triggers_local_path(repo_path), repo_path, config)?;
    crate::engine::pipeline::ensure_gitignore_entries(repo_path)
}

fn write_triggers(path: &Path, repo_path: &Path, config: &TriggersConfig) -> Result<()> {
    let chibby_dir = repo_path.join(".chibby");
    std::fs::create_dir_all(&chibby_dir)
        .with_context(|| format!("Failed to create {}", chibby_dir.display()))?;
    let toml_str = toml::to_string_pretty(config).context("Failed to serialize triggers config")?;
    std::fs::write(path, &toml_str)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    log::info!("Saved triggers to {}", path.display());
    Ok(())
}

/// Trigger ids are used in file names, run tags and CLI arguments, so keep them
/// boring: alphanumerics, dash and underscore.
pub fn validate_trigger_id(id: &str) -> Result<()> {
    if id.is_empty() {
        anyhow::bail!("Trigger id cannot be empty");
    }
    if !id
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        anyhow::bail!(
            "Trigger id '{id}' may only contain alphanumeric characters, dashes and underscores"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn schedule(id: &str, cron: &str) -> ScheduleTrigger {
        ScheduleTrigger {
            id: id.to_string(),
            enabled: true,
            cron: cron.to_string(),
            missed: MissedPolicy::Skip,
            pipeline_file: None,
            environment: None,
            stages: Vec::new(),
        }
    }

    #[test]
    fn test_load_triggers_returns_empty_when_absent() {
        let temp = TempDir::new().unwrap();
        let config = load_triggers_layered(temp.path()).unwrap();

        assert!(!config.enabled);
        assert!(config.schedules.is_empty());
        assert!(config.watches.is_empty());
    }

    #[test]
    fn test_defaults_apply_to_minimal_toml() {
        let config: TriggersConfig = toml::from_str(
            r#"
            enabled = true
            [[schedules]]
            id = "nightly"
            cron = "0 3 * * *"
            [[watches]]
            id = "tests"
            "#,
        )
        .unwrap();

        let sched = &config.schedules[0];
        assert!(sched.enabled);
        assert_eq!(sched.missed, MissedPolicy::Skip);

        let watch = &config.watches[0];
        assert!(watch.enabled);
        assert_eq!(watch.debounce_ms, 750);
        assert_eq!(watch.min_interval_secs, 10);
    }

    #[test]
    fn test_local_overrides_base_by_id_and_appends_new() {
        let base = TriggersConfig {
            enabled: false,
            schedules: vec![schedule("nightly", "0 3 * * *")],
            ..Default::default()
        };
        let local = TriggersConfig {
            enabled: true,
            schedules: vec![
                schedule("nightly", "0 5 * * *"),
                schedule("hourly", "0 * * * *"),
            ],
            ..Default::default()
        };

        let merged = merge_triggers(base, local);

        assert!(merged.enabled, "local should be able to switch triggers on");
        assert_eq!(merged.schedules.len(), 2);
        assert_eq!(merged.schedules[0].cron, "0 5 * * *");
        assert_eq!(merged.schedules[1].id, "hourly");
    }

    #[test]
    fn test_layered_load_round_trips_through_disk() {
        let temp = TempDir::new().unwrap();
        save_triggers(
            temp.path(),
            &TriggersConfig {
                enabled: true,
                schedules: vec![schedule("nightly", "0 3 * * *")],
                ..Default::default()
            },
        )
        .unwrap();
        save_triggers_local(
            temp.path(),
            &TriggersConfig {
                schedules: vec![schedule("nightly", "30 4 * * *")],
                ..Default::default()
            },
        )
        .unwrap();

        let merged = load_triggers_layered(temp.path()).unwrap();

        assert_eq!(merged.schedules.len(), 1);
        assert_eq!(merged.schedules[0].cron, "30 4 * * *");
    }

    #[test]
    fn test_validate_trigger_id_rejects_path_characters() {
        assert!(validate_trigger_id("nightly-deploy_1").is_ok());
        assert!(validate_trigger_id("").is_err());
        assert!(validate_trigger_id("../escape").is_err());
        assert!(validate_trigger_id("has space").is_err());
    }
}
