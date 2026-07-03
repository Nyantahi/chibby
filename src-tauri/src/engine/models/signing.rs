//! Code signing configuration and result types.

#[allow(unused_imports)]
use super::*;
use serde::{Deserialize, Serialize};

/// Target platform for code signing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SigningPlatform {
    Macos,
    Windows,
    Linux,
}

/// Configuration for code signing (stored in .chibby/signing.toml).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningConfig {
    /// Whether signing is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// macOS Developer ID identity (e.g. "Developer ID Application: Name (TEAMID)").
    #[serde(default)]
    pub macos_identity: Option<String>,
    /// macOS team ID for notarization.
    #[serde(default)]
    pub macos_team_id: Option<String>,
    /// macOS bundle ID for notarization.
    #[serde(default)]
    pub macos_bundle_id: Option<String>,
    /// Windows certificate file path (relative to repo).
    #[serde(default)]
    pub windows_cert_path: Option<String>,
    /// Linux GPG key ID for package signing.
    #[serde(default)]
    pub linux_gpg_key: Option<String>,
}

impl Default for SigningConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            macos_identity: None,
            macos_team_id: None,
            macos_bundle_id: None,
            windows_cert_path: None,
            linux_gpg_key: None,
        }
    }
}

/// Result of a signing operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningResult {
    /// Whether signing succeeded.
    pub success: bool,
    /// Platform that was signed for.
    pub platform: SigningPlatform,
    /// Path to the signed artifact.
    pub artifact_path: String,
    /// Whether notarization was performed (macOS only).
    pub notarized: bool,
    /// Human-readable status message.
    pub message: String,
}
