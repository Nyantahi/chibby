//! Auto-updater configuration and result types.

#[allow(unused_imports)]
use super::*;
use serde::{Deserialize, Serialize};

/// Hosting target for update publishing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum UpdatePublishTarget {
    /// AWS S3 or S3-compatible (e.g. Cloudflare R2).
    S3,
    /// GitHub Releases.
    GithubRelease,
    /// SCP to a static file server.
    Scp,
    /// Local directory (for self-hosted or LAN distribution).
    Local,
}

/// Tauri updater configuration (stored in .chibby/updater.toml).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdaterConfig {
    /// Whether the updater integration is enabled.
    #[serde(default)]
    pub enabled: bool,

    /// Public key for Tauri update verification (stored in config, safe to commit).
    #[serde(default)]
    pub public_key: Option<String>,

    /// Base URL where update artifacts will be hosted.
    /// Used to construct download URLs in latest.json.
    #[serde(default)]
    pub base_url: Option<String>,

    /// Publish target type.
    #[serde(default)]
    pub publish_target: Option<UpdatePublishTarget>,

    /// S3 bucket name (for S3/R2 targets).
    #[serde(default)]
    pub s3_bucket: Option<String>,

    /// S3 region (for S3 targets).
    #[serde(default)]
    pub s3_region: Option<String>,

    /// S3 endpoint URL (for S3-compatible like R2).
    #[serde(default)]
    pub s3_endpoint: Option<String>,

    /// GitHub owner/repo (for GitHub Releases target).
    #[serde(default)]
    pub github_repo: Option<String>,

    /// SCP destination (user@host:/path) for SCP target.
    #[serde(default)]
    pub scp_dest: Option<String>,

    /// Local directory path (for local target).
    #[serde(default)]
    pub local_dir: Option<String>,
}

impl Default for UpdaterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            public_key: None,
            base_url: None,
            publish_target: None,
            s3_bucket: None,
            s3_region: None,
            s3_endpoint: None,
            github_repo: None,
            scp_dest: None,
            local_dir: None,
        }
    }
}

/// A per-platform entry in the Tauri latest.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePlatformEntry {
    /// Download URL for this platform's update bundle.
    pub url: String,
    /// Base64-encoded Ed25519 signature of the update bundle.
    pub signature: String,
}

/// Tauri-compatible latest.json structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TauriLatestJson {
    /// Version string (semver).
    pub version: String,
    /// Release notes (from changelog).
    #[serde(default)]
    pub notes: Option<String>,
    /// Publication date (RFC 3339).
    pub pub_date: String,
    /// Per-platform update entries keyed by Tauri platform identifiers
    /// (e.g. "darwin-aarch64", "darwin-x86_64", "linux-x86_64", "windows-x86_64").
    pub platforms: std::collections::HashMap<String, UpdatePlatformEntry>,
}

/// Result of generating a Tauri update key pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateKeyResult {
    /// The public key string.
    pub public_key: String,
    /// Whether the private key was stored in the OS keychain.
    pub private_key_stored: bool,
    /// Human-readable status message.
    pub message: String,
}

/// Result of generating latest.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestJsonResult {
    /// Path to the generated latest.json file.
    pub path: String,
    /// The generated JSON content (for preview).
    pub content: TauriLatestJson,
    /// Whether schema validation passed.
    pub valid: bool,
}

/// Result of signing an update bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSignResult {
    /// Path to the file that was signed.
    pub file_path: String,
    /// Base64-encoded signature.
    pub signature: String,
    /// Whether local verification passed.
    pub verified: bool,
}

/// Result of publishing an update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePublishResult {
    /// Whether the publish succeeded.
    pub success: bool,
    /// Target that was published to.
    pub target: UpdatePublishTarget,
    /// Files that were uploaded.
    pub uploaded_files: Vec<String>,
    /// Human-readable status message.
    pub message: String,
}

// ---------------------------------------------------------------------------
// Phase 5.8: Security and Quality Gates
// ---------------------------------------------------------------------------
