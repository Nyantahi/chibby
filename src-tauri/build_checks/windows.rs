//! Windows-specific build validation checks.
//!
//! # Adding new checks
//!
//! 1. Add your check function below
//! 2. Call it from `validate()`
//! 3. Use `super::common::warn()` to emit warnings
//!
//! # Available checks
//!
//! - `check_webview2`: Verifies WebView2 runtime or SDK is available
//! - `check_vbscript`: Verifies VBSCRIPT for MSI builds
//! - `check_nsis`: Verifies NSIS for NSIS installer builds
//! - `check_signtool`: Verifies signtool for code signing

use super::common::{self, TauriConfig};

/// Run all Windows-specific validation checks.
pub fn validate(config: &TauriConfig) {
    check_webview2();
    check_vbscript(config);
    check_nsis(config);
    check_signtool();
    // Add new checks here
}

/// Check if WebView2 runtime is available.
///
/// WebView2 is required for Tauri apps on Windows.
fn check_webview2() {
    // WebView2 is usually available on Windows 10/11, but we can check the registry
    // For now, just check if we're on a supported Windows version
    let output = std::process::Command::new("reg")
        .args([
            "query",
            r"HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
            "/v",
            "pv",
        ])
        .output();

    match output {
        Ok(result) if result.status.success() => {
            // WebView2 found
        }
        _ => {
            common::warn("WebView2 runtime may not be installed.");
            common::warn("Download from: https://developer.microsoft.com/microsoft-edge/webview2");
            common::warn("The app will prompt users to install WebView2 if missing.");
        }
    }
}

/// Check if VBSCRIPT/WMI is available for MSI builds.
///
/// MSI generation on Windows requires VBSCRIPT which is being deprecated.
fn check_vbscript(config: &TauriConfig) {
    if !config.has_bundle_target("msi") {
        return;
    }

    // Check if cscript is available
    let output = std::process::Command::new("where")
        .arg("cscript")
        .output();

    match output {
        Ok(result) if result.status.success() => {
            // VBSCRIPT available
        }
        _ => {
            common::warn("VBSCRIPT (cscript) not found. MSI bundling may fail.");
            common::warn("On newer Windows versions, VBSCRIPT may be disabled by default.");
            common::warn("Enable via: Settings > Apps > Optional Features > Add > VBSCRIPT");
        }
    }
}

/// Check if NSIS is installed for NSIS installer builds.
fn check_nsis(config: &TauriConfig) {
    if !config.has_bundle_target("nsis") {
        return;
    }

    // Check for NSIS in PATH
    if !common::command_exists("makensis") {
        // Check common install locations
        let nsis_paths = [
            r"C:\Program Files (x86)\NSIS\makensis.exe",
            r"C:\Program Files\NSIS\makensis.exe",
        ];

        let found = nsis_paths.iter().any(|p| std::path::Path::new(p).exists());

        if !found {
            common::warn("NSIS not found. NSIS installer bundling will fail.");
            common::warn("Download from: https://nsis.sourceforge.io/Download");
            common::warn("Or install via: choco install nsis");
        }
    }
}

/// Check if signtool is available for code signing.
fn check_signtool() {
    // Only check in release mode
    if std::env::var("PROFILE").unwrap_or_default() != "release" {
        return;
    }

    // Check for signing environment variables
    let has_signing_env = std::env::var("TAURI_SIGNING_PRIVATE_KEY").is_ok()
        || std::env::var("WINDOWS_CERTIFICATE").is_ok();

    if has_signing_env && !common::command_exists("signtool") {
        common::warn("Signing credentials found but signtool is not in PATH.");
        common::warn("Install Windows SDK to get signtool.");
    }
}

// Future checks to add:
// - check_windows_sdk: Verify Windows SDK is installed
// - check_visual_studio: Verify MSVC build tools are available
// - check_rustup_target: Verify correct Rust targets are installed
