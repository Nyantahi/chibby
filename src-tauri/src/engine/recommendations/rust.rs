//! Rust recommendations.

use crate::engine::models::{FileRecommendation, RecommendationCategory, RecommendationPriority};
use std::path::Path;

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
        exists: repo_path.join("Cargo.lock").exists(),
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
        exists: repo_path.join("rustfmt.toml").exists() || repo_path.join(".rustfmt.toml").exists(),
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
        exists: repo_path.join("clippy.toml").exists() || repo_path.join(".clippy.toml").exists(),
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
        exists: repo_path.join("rust-toolchain.toml").exists()
            || repo_path.join("rust-toolchain").exists(),
        template_hint: Some("[toolchain]\nchannel = \"stable\"".to_string()),
    });
}
