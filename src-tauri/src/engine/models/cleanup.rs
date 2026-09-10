//! Cleanup configuration and result types.

#[allow(unused_imports)]
use super::*;
use serde::{Deserialize, Serialize};

/// Cleanup configuration (stored in .chibby/cleanup.toml).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupConfig {
    /// Max artifact versions to keep per project.
    #[serde(default = "default_retention")]
    pub artifact_retention: u32,
    /// Max full run records to keep **per project**. Older records lose their
    /// logs, not their history: the run index keeps a summary of every run
    /// under the far longer `index_retention_days` / `index_max_entries`
    /// bounds, so trends outlive the logs they were computed from.
    #[serde(default = "default_run_retention")]
    pub run_retention: u32,
    /// Max age of a run *summary* in the index. 0 disables the bound.
    #[serde(default = "default_index_retention_days")]
    pub index_retention_days: u32,
    /// Max summaries the index keeps. 0 disables the bound. 5000 summaries is
    /// roughly 2 MB — cheap enough that history is worth keeping.
    #[serde(default = "default_index_max_entries")]
    pub index_max_entries: u32,
    /// Whether to prune Docker images on remote deploy targets.
    #[serde(default)]
    pub prune_remote_docker: bool,
}

fn default_run_retention() -> u32 {
    200
}

fn default_index_retention_days() -> u32 {
    180
}

fn default_index_max_entries() -> u32 {
    5_000
}

impl Default for CleanupConfig {
    fn default() -> Self {
        Self {
            artifact_retention: default_retention(),
            run_retention: default_run_retention(),
            index_retention_days: default_index_retention_days(),
            index_max_entries: default_index_max_entries(),
            prune_remote_docker: false,
        }
    }
}

/// Result of a cleanup operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupResult {
    /// Number of artifact versions removed.
    pub artifacts_removed: u32,
    /// Number of full run records removed (their index summaries survive).
    pub runs_removed: u32,
    /// Bytes freed.
    pub bytes_freed: u64,
    /// Run summaries dropped from the index by its own retention bounds.
    #[serde(default)]
    pub index_entries_pruned: u32,
    /// Summaries remaining in the index afterwards.
    #[serde(default)]
    pub index_entries: u32,
    /// Size of `runs-index.json` on disk.
    #[serde(default)]
    pub index_bytes: u64,
    /// Details of what was cleaned.
    pub details: Vec<String>,
}

// ---------------------------------------------------------------------------
// CI/CD Recommendations
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cleanup_config_defaults() {
        let config = CleanupConfig::default();

        assert_eq!(config.artifact_retention, 5);
        assert_eq!(config.run_retention, 200);
        assert_eq!(config.index_retention_days, 180);
        assert_eq!(config.index_max_entries, 5_000);
        assert!(!config.prune_remote_docker);
    }
}
