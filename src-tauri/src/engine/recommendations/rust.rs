//! Rust recommendations.

use crate::engine::detector::{exists_within, NESTED_SCAN_DEPTH};
use crate::engine::models::{FileRecommendation, RecommendationCategory, RecommendationPriority};
use std::path::Path;

/// Whether any of `names` exists at or below the repo root (within the shallow
/// scan depth). Covers the standard Tauri layout where the Rust crate and its
/// config live in `src-tauri/`, and workspace members — not just the repo root —
/// so these files aren't recommended when they already exist nested.
fn exists_nested(repo_path: &Path, names: &[&str]) -> bool {
    exists_within(repo_path, NESTED_SCAN_DEPTH, &|n: &str| names.contains(&n))
}

/// Add Rust specific recommendations.
pub(super) fn add_rust_recommendations(repo_path: &Path, recs: &mut Vec<FileRecommendation>) {
    // Cargo.lock
    recs.push(FileRecommendation {
        file_name: "Cargo.lock".to_string(),
        title: "Cargo Lock File".to_string(),
        description: "Locks dependency versions for reproducible builds.".to_string(),
        priority: RecommendationPriority::Critical,
        category: RecommendationCategory::Dependencies,
        docs_url: Some(
            "https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html".to_string(),
        ),
        exists: exists_nested(repo_path, &["Cargo.lock"]),
        template_hint: Some("Run 'cargo build' to generate".to_string()),
    });

    // rustfmt
    recs.push(FileRecommendation {
        file_name: "rustfmt.toml".to_string(),
        title: "Rustfmt Config".to_string(),
        description: "Consistent Rust code formatting across the project.".to_string(),
        priority: RecommendationPriority::High,
        category: RecommendationCategory::CodeQuality,
        docs_url: Some("https://rust-lang.github.io/rustfmt/".to_string()),
        exists: exists_nested(repo_path, &["rustfmt.toml", ".rustfmt.toml"]),
        template_hint: Some("edition = \"2021\"".to_string()),
    });

    // clippy
    recs.push(FileRecommendation {
        file_name: "clippy.toml".to_string(),
        title: "Clippy Config".to_string(),
        description: "Rust linting configuration for catching common mistakes.".to_string(),
        priority: RecommendationPriority::Medium,
        category: RecommendationCategory::CodeQuality,
        docs_url: Some("https://doc.rust-lang.org/clippy/configuration.html".to_string()),
        exists: exists_nested(repo_path, &["clippy.toml", ".clippy.toml"]),
        template_hint: Some("Configure lint levels and allow/deny rules".to_string()),
    });

    // rust-toolchain
    recs.push(FileRecommendation {
        file_name: "rust-toolchain.toml".to_string(),
        title: "Rust Toolchain File".to_string(),
        description: "Specifies the Rust version and components for the project.".to_string(),
        priority: RecommendationPriority::Medium,
        category: RecommendationCategory::Dependencies,
        docs_url: Some("https://rust-lang.github.io/rustup/overrides.html".to_string()),
        exists: exists_nested(repo_path, &["rust-toolchain.toml", "rust-toolchain"]),
        template_hint: Some("[toolchain]\nchannel = \"stable\"".to_string()),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_config_detected_in_src_tauri() {
        // Tauri layout: Rust crate + config live in src-tauri/, not the repo root.
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("src-tauri")).unwrap();
        std::fs::write(tmp.path().join("src-tauri/Cargo.lock"), "").unwrap();
        std::fs::write(tmp.path().join("src-tauri/rustfmt.toml"), "").unwrap();

        let mut recs = Vec::new();
        add_rust_recommendations(tmp.path(), &mut recs);
        let find = |name: &str| recs.iter().find(|r| r.file_name == name).unwrap();

        // Present in src-tauri/ -> recognized (no false-positive recommendation).
        assert!(find("Cargo.lock").exists);
        assert!(find("rustfmt.toml").exists);
        // Genuinely missing -> still flagged.
        assert!(!find("clippy.toml").exists);
        assert!(!find("rust-toolchain.toml").exists);
    }
}
