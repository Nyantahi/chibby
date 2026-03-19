//! Common utilities for build-time validation checks.
//!
//! This module provides shared helpers used by platform-specific validation modules.

use std::fs;
use std::path::Path;
use std::process::Command;

/// Parsed Tauri configuration relevant to validation.
#[derive(Debug)]
pub struct TauriConfig {
    /// The bundle identifier (e.g., "com.company.app")
    pub identifier: Option<String>,
    /// Raw config content for additional parsing
    pub raw_content: String,
}

impl TauriConfig {
    /// Load and parse tauri.conf.json from the current directory.
    ///
    /// Returns `None` if the file cannot be read, emitting a warning.
    pub fn load() -> Option<Self> {
        let config_path = Path::new("tauri.conf.json");

        if !config_path.exists() {
            warn("tauri.conf.json not found in src-tauri directory");
            return None;
        }

        let raw_content = match fs::read_to_string(config_path) {
            Ok(content) => content,
            Err(e) => {
                warn(&format!("Failed to read tauri.conf.json: {}", e));
                return None;
            }
        };

        Some(Self {
            identifier: extract_json_string(&raw_content, "identifier"),
            raw_content,
        })
    }

    /// Check if a specific bundle target is enabled.
    pub fn has_bundle_target(&self, target: &str) -> bool {
        self.raw_content.contains(&format!("\"{}\"", target))
            || self.raw_content.contains("\"all\"")
    }
}

/// Emit a cargo build warning that will be displayed during compilation.
pub fn warn(message: &str) {
    println!("cargo:warning={}", message);
}

/// Check if a command exists in PATH.
///
/// Uses `which` on Unix-like systems and `where` on Windows.
pub fn command_exists(cmd: &str) -> bool {
    #[cfg(target_os = "windows")]
    let checker = "where";

    #[cfg(not(target_os = "windows"))]
    let checker = "which";

    Command::new(checker)
        .arg(cmd)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Extract a string value from JSON content.
///
/// This is a simple parser suitable for build scripts without pulling in serde.
pub fn extract_json_string(content: &str, key: &str) -> Option<String> {
    let search = format!("\"{}\"", key);
    if let Some(pos) = content.find(&search) {
        let after_key = &content[pos + search.len()..];
        if let Some(colon_pos) = after_key.find(':') {
            let after_colon = &after_key[colon_pos + 1..];
            if let Some(quote_start) = after_colon.find('"') {
                let value_start = &after_colon[quote_start + 1..];
                if let Some(quote_end) = value_start.find('"') {
                    return Some(value_start[..quote_end].to_string());
                }
            }
        }
    }
    None
}

/// Validate the bundle identifier format.
pub fn validate_identifier(config: &TauriConfig) {
    if let Some(ref identifier) = config.identifier {
        // Check for .app suffix conflict
        if identifier.ends_with(".app") {
            warn(&format!(
                "Bundle identifier '{}' ends with '.app' which conflicts with macOS bundle extension.",
                identifier
            ));
            warn(&format!(
                "This may cause DMG bundling to fail. Consider changing to '{}'",
                identifier.trim_end_matches(".app")
            ));
        }

        // Check for valid reverse-domain format
        if !identifier.contains('.') {
            warn(&format!(
                "Bundle identifier '{}' should use reverse-domain format (e.g., com.company.app)",
                identifier
            ));
        }
    }
}

/// Validate version synchronization across config files.
///
/// Checks that package.json, Cargo.toml, and tauri.conf.json have matching versions.
pub fn validate_version_sync() {
    let tauri_version = read_tauri_version();
    let cargo_version = read_cargo_version();
    let package_version = read_package_json_version();

    // Collect all found versions
    let versions: Vec<(&str, &str)> = [
        ("tauri.conf.json", tauri_version.as_deref()),
        ("Cargo.toml", cargo_version.as_deref()),
        ("package.json", package_version.as_deref()),
    ]
    .iter()
    .filter_map(|(name, ver)| ver.map(|v| (*name, v)))
    .collect();

    if versions.len() >= 2 {
        let first_version = versions[0].1;
        let mismatches: Vec<_> = versions
            .iter()
            .filter(|(_, v)| *v != first_version)
            .collect();

        if !mismatches.is_empty() {
            warn("Version mismatch detected across config files:");
            for (name, version) in &versions {
                warn(&format!("  {}: {}", name, version));
            }
            warn("Consider synchronizing versions before release.");
        }
    }
}

fn read_tauri_version() -> Option<String> {
    let config_path = Path::new("tauri.conf.json");
    let content = fs::read_to_string(config_path).ok()?;
    extract_json_string(&content, "version")
}

fn read_cargo_version() -> Option<String> {
    let cargo_path = Path::new("Cargo.toml");
    let content = fs::read_to_string(cargo_path).ok()?;

    // Simple TOML parsing for version = "x.y.z"
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("version") {
            if let Some(eq_pos) = trimmed.find('=') {
                let value = trimmed[eq_pos + 1..].trim();
                if value.starts_with('"') && value.ends_with('"') {
                    return Some(value[1..value.len() - 1].to_string());
                }
            }
        }
    }
    None
}

fn read_package_json_version() -> Option<String> {
    // package.json is in the parent directory (frontend root)
    let package_path = Path::new("../package.json");
    let content = fs::read_to_string(package_path).ok()?;
    extract_json_string(&content, "version")
}
