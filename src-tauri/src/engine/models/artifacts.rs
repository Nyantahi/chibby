//! Artifact configuration and manifest types.

#[allow(unused_imports)]
use super::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Artifact naming configuration (stored in .chibby/artifacts.toml).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactConfig {
    /// Output directory for artifacts (relative to repo root).
    #[serde(default = "default_artifact_dir")]
    pub output_dir: String,
    /// How many versions to retain locally.
    #[serde(default = "default_retention")]
    pub retention_count: u32,
    /// Glob patterns to collect as artifacts (e.g. "target/release/*.dmg").
    #[serde(default)]
    pub patterns: Vec<String>,
    /// Optional upload destination (e.g. "s3://bucket/path", "github-release", "scp://host:/path").
    #[serde(default)]
    pub upload_to: Option<String>,
}

fn default_artifact_dir() -> String {
    ".chibby/artifacts".to_string()
}

pub(crate) fn default_retention() -> u32 {
    5
}

impl Default for ArtifactConfig {
    fn default() -> Self {
        Self {
            output_dir: default_artifact_dir(),
            patterns: Vec::new(),
            retention_count: default_retention(),
            upload_to: None,
        }
    }
}

/// A collected artifact with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    /// Original file name.
    pub file_name: String,
    /// Standardized name ({project}-{version}-{platform}-{arch}.{ext}).
    pub canonical_name: String,
    /// Absolute path to the artifact.
    pub path: String,
    /// SHA256 checksum.
    pub sha256: String,
    /// File size in bytes.
    pub size_bytes: u64,
    /// When the artifact was collected.
    pub collected_at: DateTime<Utc>,
}

/// Manifest for a single artifact collection run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactManifest {
    /// Project name.
    pub project: String,
    /// Version that was built.
    pub version: String,
    /// Git commit hash.
    pub commit: Option<String>,
    /// Git branch.
    pub branch: Option<String>,
    /// When the manifest was created.
    pub created_at: DateTime<Utc>,
    /// List of artifacts in this collection.
    pub artifacts: Vec<Artifact>,
}
