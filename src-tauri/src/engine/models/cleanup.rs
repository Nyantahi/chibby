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
    /// Max run history entries to keep.
    #[serde(default = "default_run_retention")]
    pub run_retention: u32,
    /// Whether to prune Docker images on remote deploy targets.
    #[serde(default)]
    pub prune_remote_docker: bool,
}

fn default_run_retention() -> u32 {
    50
}

impl Default for CleanupConfig {
    fn default() -> Self {
        Self {
            artifact_retention: default_retention(),
            run_retention: default_run_retention(),
            prune_remote_docker: false,
        }
    }
}

/// Result of a cleanup operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupResult {
    /// Number of artifact versions removed.
    pub artifacts_removed: u32,
    /// Number of run history entries removed.
    pub runs_removed: u32,
    /// Bytes freed.
    pub bytes_freed: u64,
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
        assert_eq!(config.run_retention, 50);
        assert!(!config.prune_remote_docker);
    }
}
