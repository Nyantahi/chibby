//! Tauri build script.
//!
//! This script runs during `cargo build` and performs:
//! - Platform-specific validation checks (see `build_checks/` modules)
//! - Tauri build setup
//!
//! To add new checks, see the appropriate platform module in `build_checks/`.

mod build_checks;

fn main() {
    // Run pre-build validation checks
    build_checks::validate();

    tauri_build::build()
}

