//! Go recommendations.

use super::exists_nested;
use crate::engine::models::{FileRecommendation, RecommendationCategory, RecommendationPriority};
use std::path::Path;

/// Add Go specific recommendations.
pub(super) fn add_go_recommendations(repo_path: &Path, recs: &mut Vec<FileRecommendation>) {
    // go.sum
    recs.push(FileRecommendation {
        file_name: "go.sum".to_string(),
        title: "Go Checksum File".to_string(),
        description: "Cryptographic checksums for module dependencies.".to_string(),
        priority: RecommendationPriority::Critical,
        category: RecommendationCategory::Dependencies,
        docs_url: Some("https://go.dev/ref/mod#go-sum-files".to_string()),
        exists: exists_nested(repo_path, &["go.sum"]),
        template_hint: Some("Run 'go mod tidy' to generate".to_string()),
    });

    // golangci-lint
    recs.push(FileRecommendation {
        file_name: ".golangci.yml".to_string(),
        title: "GolangCI-Lint Config".to_string(),
        description: "Comprehensive Go linting with multiple linters.".to_string(),
        priority: RecommendationPriority::High,
        category: RecommendationCategory::CodeQuality,
        docs_url: Some("https://golangci-lint.run/usage/configuration/".to_string()),
        exists: exists_nested(repo_path, &[".golangci.yml", ".golangci.yaml"]),
        template_hint: Some("Enable staticcheck, gosec, errcheck".to_string()),
    });
}
