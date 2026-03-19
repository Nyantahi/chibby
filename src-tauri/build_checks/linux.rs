//! Linux-specific build validation checks.
//!
//! # Adding new checks
//!
//! 1. Add your check function below
//! 2. Call it from `validate()`
//! 3. Use `super::common::warn()` to emit warnings
//!
//! # Available checks
//!
//! - `check_webkit2gtk`: Verifies libwebkit2gtk-4.1-dev is installed
//! - `check_appindicator`: Verifies libappindicator3-dev is installed
//! - `check_patchelf`: Verifies patchelf for AppImage builds
//! - `check_glibc`: Warns about GLIBC version for AppImage compatibility

use super::common::{self, TauriConfig};

/// Run all Linux-specific validation checks.
pub fn validate(config: &TauriConfig) {
    check_webkit2gtk();
    check_appindicator();
    check_patchelf(config);
    check_glibc();
    // Add new checks here
}

/// Check if libwebkit2gtk-4.1-dev is installed.
fn check_webkit2gtk() {
    // Check via pkg-config
    let output = std::process::Command::new("pkg-config")
        .args(["--exists", "webkit2gtk-4.1"])
        .status();

    match output {
        Ok(status) if status.success() => {
            // webkit2gtk found
        }
        _ => {
            // Try the older 4.0 version
            let output_40 = std::process::Command::new("pkg-config")
                .args(["--exists", "webkit2gtk-4.0"])
                .status();

            match output_40 {
                Ok(status) if status.success() => {
                    common::warn("Found webkit2gtk-4.0 but Tauri v2 prefers webkit2gtk-4.1");
                    common::warn("On Ubuntu/Debian: sudo apt install libwebkit2gtk-4.1-dev");
                }
                _ => {
                    common::warn("libwebkit2gtk not found. Build will fail.");
                    common::warn("On Ubuntu/Debian: sudo apt install libwebkit2gtk-4.1-dev");
                    common::warn("On Fedora: sudo dnf install webkit2gtk4.1-devel");
                    common::warn("On Arch: sudo pacman -S webkit2gtk-4.1");
                }
            }
        }
    }
}

/// Check if libappindicator3-dev is installed for system tray support.
fn check_appindicator() {
    let output = std::process::Command::new("pkg-config")
        .args(["--exists", "ayatana-appindicator3-0.1"])
        .status();

    let found_ayatana = output.map(|s| s.success()).unwrap_or(false);

    if !found_ayatana {
        let output_legacy = std::process::Command::new("pkg-config")
            .args(["--exists", "appindicator3-0.1"])
            .status();

        if !output_legacy.map(|s| s.success()).unwrap_or(false) {
            common::warn("libappindicator not found. System tray may not work.");
            common::warn("On Ubuntu/Debian: sudo apt install libayatana-appindicator3-dev");
            common::warn("On Fedora: sudo dnf install libappindicator-gtk3-devel");
        }
    }
}

/// Check if patchelf is available for AppImage builds.
fn check_patchelf(config: &TauriConfig) {
    if !config.has_bundle_target("appimage") {
        return;
    }

    if !common::command_exists("patchelf") {
        common::warn("patchelf not found. AppImage bundling may fail.");
        common::warn("On Ubuntu/Debian: sudo apt install patchelf");
        common::warn("On Fedora: sudo dnf install patchelf");
        common::warn("On Arch: sudo pacman -S patchelf");
    }
}

/// Check GLIBC version for AppImage compatibility.
///
/// Building on a system with a newer GLIBC produces AppImages
/// that won't run on older systems.
fn check_glibc() {
    // Only relevant for release builds
    if std::env::var("PROFILE").unwrap_or_default() != "release" {
        return;
    }

    let output = std::process::Command::new("ldd")
        .arg("--version")
        .output();

    if let Ok(result) = output {
        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);
        let version_text = format!("{}{}", stdout, stderr);

        // Parse GLIBC version (e.g., "ldd (GNU libc) 2.35")
        if let Some(version) = extract_glibc_version(&version_text) {
            // GLIBC 2.31+ may cause compatibility issues on older distros
            if version >= (2, 35) {
                common::warn(&format!(
                    "GLIBC {} detected. AppImages built here may not run on older distros.",
                    format!("{}.{}", version.0, version.1)
                ));
                common::warn("Consider building on an older distro or in a container.");
                common::warn("Ubuntu 20.04 (GLIBC 2.31) is a good baseline for compatibility.");
            }
        }
    }
}

fn extract_glibc_version(text: &str) -> Option<(u32, u32)> {
    // Look for version pattern like "2.35" or "2.31"
    for word in text.split_whitespace() {
        if word.contains('.') && word.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            let parts: Vec<&str> = word.split('.').collect();
            if parts.len() >= 2 {
                if let (Ok(major), Ok(minor)) = (parts[0].parse(), parts[1].parse()) {
                    return Some((major, minor));
                }
            }
        }
    }
    None
}

// Future checks to add:
// - check_librsvg: Verify librsvg2-dev for icon rendering
// - check_libssl: Verify libssl-dev for HTTPS support
// - check_squashfs: Verify squashfs-tools for AppImage
