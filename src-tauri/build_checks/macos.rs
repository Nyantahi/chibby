//! macOS-specific build validation checks.
//!
//! # Adding new checks
//!
//! 1. Add your check function below
//! 2. Call it from `validate()`
//! 3. Use `super::common::warn()` to emit warnings
//!
//! # Available checks
//!
//! - `check_create_dmg`: Verifies create-dmg is installed for DMG bundling
//! - `check_code_signing`: Verifies signing identity is available
//! - (more to come)

use super::common::{self, TauriConfig};

/// Run all macOS-specific validation checks.
pub fn validate(config: &TauriConfig) {
    check_create_dmg(config);
    check_code_signing_identity();
    // Add new checks here
}

/// Check if create-dmg is installed when DMG bundling is enabled.
fn check_create_dmg(config: &TauriConfig) {
    if config.has_bundle_target("dmg") && !common::command_exists("create-dmg") {
        common::warn("Bundle targets include DMG but 'create-dmg' is not installed.");
        common::warn("Install with: brew install create-dmg");
        common::warn("Or build without DMG: npm run tauri build -- --bundles app");
    }
}

/// Check if a code signing identity is available.
///
/// This is a soft check - many developers build unsigned locally.
fn check_code_signing_identity() {
    // Only warn if we're likely doing a release build
    if std::env::var("PROFILE").unwrap_or_default() != "release" {
        return;
    }

    // Check for signing identity in environment
    let has_signing_env = std::env::var("APPLE_SIGNING_IDENTITY").is_ok()
        || std::env::var("TAURI_SIGNING_IDENTITY").is_ok();

    if !has_signing_env {
        // Check if any signing identities exist
        let output = std::process::Command::new("security")
            .args(["find-identity", "-v", "-p", "codesigning"])
            .output();

        match output {
            Ok(result) if result.status.success() => {
                let stdout = String::from_utf8_lossy(&result.stdout);
                if stdout.contains("0 valid identities found") {
                    common::warn(
                        "No code signing identities found. The app will be unsigned.",
                    );
                    common::warn(
                        "For distribution, you'll need an Apple Developer certificate.",
                    );
                }
            }
            _ => {
                // security command failed, skip this check
            }
        }
    }
}

// Future checks to add:
// - check_notarytool_credentials: Verify notarization credentials in keychain
// - check_xcode_tools: Verify Xcode command line tools are installed
// - check_bundle_resources: Verify icons and resources exist
