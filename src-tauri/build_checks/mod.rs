//! Build-time validation checks for Tauri applications.
//!
//! This module provides platform-specific checks that run during `cargo build`
//! to catch common configuration issues early. Each platform has its own module
//! that contributors can extend without touching unrelated code.
//!
//! # Adding new checks
//!
//! 1. Find the appropriate platform module (`macos.rs`, `windows.rs`, `linux.rs`)
//! 2. Add your check function
//! 3. Call it from the platform's `validate()` function
//! 4. Use `common::warn()` to emit warnings
//!
//! # Example
//!
//! ```ignore
//! // In macos.rs
//! pub fn check_my_tool(config: &TauriConfig) {
//!     if !common::command_exists("my-tool") {
//!         common::warn("my-tool is not installed. Install with: brew install my-tool");
//!     }
//! }
//! ```

pub mod common;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

use common::TauriConfig;

/// Run all platform-appropriate validation checks.
///
/// This is the main entry point called from `build.rs`.
pub fn validate() {
    let config = match TauriConfig::load() {
        Some(c) => c,
        None => return, // Warnings already emitted by load()
    };

    // Run cross-platform checks
    common::validate_identifier(&config);
    common::validate_version_sync();

    // Run platform-specific checks
    #[cfg(target_os = "macos")]
    macos::validate(&config);

    #[cfg(target_os = "windows")]
    windows::validate(&config);

    #[cfg(target_os = "linux")]
    linux::validate(&config);
}
