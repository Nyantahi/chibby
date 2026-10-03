//! Tauri build script.
//!
//! This script runs during `cargo build` and performs:
//! - Platform-specific validation checks (see `build_checks/` modules)
//! - Tauri build setup
//!
//! To add new checks, see the appropriate platform module in `build_checks/`.

mod build_checks;

fn main() {
    if std::env::var_os("CARGO_FEATURE_GUI").is_some() {
        // Run pre-build validation checks
        build_checks::validate();
        // Statically link the MSVC runtime on Windows so the .exe doesn't require
        // the VC++ redistributable. Set explicitly via WindowsAttributes to avoid
        // the deprecated STATIC_VCRUNTIME env mechanism. No-op on other platforms.
        let attributes = tauri_build::Attributes::new()
            .windows_attributes(tauri_build::WindowsAttributes::new().static_vc_runtime(true));
        tauri_build::try_build(attributes).expect("failed to run tauri-build");
    }
}
