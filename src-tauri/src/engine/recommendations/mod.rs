//! CI/CD Recommendations Engine
//!
//! Analyzes a repository and recommends missing CI/CD configuration files
//! based on detected project types and industry best practices.
//!
//! Split into a facade (`analyze_repository`, scoring) plus `detect` for
//! project-type detection and one module per language for the file
//! recommendations, mirroring the `models` re-export layout.

mod detect;
mod dotnet;
mod go;
mod java;
mod node;
mod php;
mod python;
mod ruby;
mod rust;
mod universal;

use crate::engine::models::{
    FileRecommendation, ProjectRecommendations, RecommendationPriority, RecommendationSummary,
};
use detect::detect_project_types;
use std::path::Path;

/// Analyze a repository and generate CI/CD recommendations.
pub fn analyze_repository(repo_path: &Path) -> ProjectRecommendations {
    let project_types = detect_project_types(repo_path);
    let mut recommendations = Vec::new();

    // Add universal recommendations
    universal::add_universal_recommendations(repo_path, &mut recommendations);

    // Add project-type specific recommendations
    for project_type in &project_types {
        match project_type.as_str() {
            "node" | "javascript" | "typescript" => {
                node::add_node_recommendations(repo_path, &mut recommendations);
            }
            "rust" => {
                rust::add_rust_recommendations(repo_path, &mut recommendations);
            }
            "python" => {
                python::add_python_recommendations(repo_path, &mut recommendations);
            }
            "go" => {
                go::add_go_recommendations(repo_path, &mut recommendations);
            }
            "java" | "kotlin" => {
                java::add_java_recommendations(repo_path, &mut recommendations);
            }
            "dotnet" | "csharp" => {
                dotnet::add_dotnet_recommendations(repo_path, &mut recommendations);
            }
            "ruby" => {
                ruby::add_ruby_recommendations(repo_path, &mut recommendations);
            }
            "php" => {
                php::add_php_recommendations(repo_path, &mut recommendations);
            }
            _ => {}
        }
    }

    // Deduplicate recommendations by file_name
    recommendations.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    recommendations.dedup_by(|a, b| a.file_name == b.file_name);

    // Sort by priority (Critical first)
    recommendations.sort_by(|a, b| a.priority.cmp(&b.priority));

    // Calculate summary and readiness score BEFORE filtering
    // (so they reflect overall project health)
    let summary = calculate_summary(&recommendations);
    let readiness_score = calculate_readiness_score(&recommendations);

    // Filter to only include MISSING files (exists == false)
    let missing_recommendations: Vec<FileRecommendation> =
        recommendations.into_iter().filter(|r| !r.exists).collect();

    ProjectRecommendations {
        project_types,
        recommendations: missing_recommendations,
        readiness_score,
        summary,
    }
}

/// Calculate recommendation summary.
fn calculate_summary(recommendations: &[FileRecommendation]) -> RecommendationSummary {
    let mut summary = RecommendationSummary {
        critical_missing: 0,
        high_missing: 0,
        medium_missing: 0,
        low_missing: 0,
        total_recommendations: recommendations.len() as u32,
        total_present: 0,
    };

    for rec in recommendations {
        if rec.exists {
            summary.total_present += 1;
        } else {
            match rec.priority {
                RecommendationPriority::Critical => summary.critical_missing += 1,
                RecommendationPriority::High => summary.high_missing += 1,
                RecommendationPriority::Medium => summary.medium_missing += 1,
                RecommendationPriority::Low => summary.low_missing += 1,
            }
        }
    }

    summary
}

/// Calculate CI/CD readiness score (0-100).
fn calculate_readiness_score(recommendations: &[FileRecommendation]) -> u8 {
    if recommendations.is_empty() {
        return 100;
    }

    let mut score: f32 = 0.0;
    let mut max_score: f32 = 0.0;

    for rec in recommendations {
        let weight = match rec.priority {
            RecommendationPriority::Critical => 4.0,
            RecommendationPriority::High => 3.0,
            RecommendationPriority::Medium => 2.0,
            RecommendationPriority::Low => 1.0,
        };

        max_score += weight;
        if rec.exists {
            score += weight;
        }
    }

    if max_score == 0.0 {
        100
    } else {
        ((score / max_score) * 100.0).round() as u8
    }
}

// ---------------------------------------------------------------------------
// Unit Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_detect_project_types_node() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("package.json"), "{}").unwrap();

        let types = detect_project_types(temp.path());
        assert!(types.contains(&"node".to_string()));
    }

    #[test]
    fn test_detect_project_types_rust() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();

        let types = detect_project_types(temp.path());
        assert!(types.contains(&"rust".to_string()));
    }

    #[test]
    fn test_detect_project_types_multiple() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("package.json"), "{}").unwrap();
        std::fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();

        let types = detect_project_types(temp.path());
        assert!(types.contains(&"node".to_string()));
        assert!(types.contains(&"rust".to_string()));
    }

    #[test]
    fn test_analyze_empty_repo() {
        let temp = TempDir::new().unwrap();
        let recs = analyze_repository(temp.path());

        // Should return universal recommendations
        assert!(!recs.recommendations.is_empty());
        assert!(recs
            .recommendations
            .iter()
            .any(|r| r.file_name == ".gitignore"));
        assert!(recs
            .recommendations
            .iter()
            .any(|r| r.file_name == "README.md"));
    }

    #[test]
    fn test_readiness_score_empty() {
        let temp = TempDir::new().unwrap();
        let recs = analyze_repository(temp.path());

        // Empty repo should have low score
        assert!(recs.readiness_score < 50);
    }

    #[test]
    fn test_readiness_score_with_essentials() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join(".gitignore"), "").unwrap();
        std::fs::write(temp.path().join("README.md"), "# Test").unwrap();
        std::fs::write(temp.path().join("LICENSE"), "MIT").unwrap();
        std::fs::create_dir_all(temp.path().join(".github/workflows")).unwrap();

        let recs = analyze_repository(temp.path());

        // Should have higher score with essentials
        assert!(recs.readiness_score > 30);
    }

    #[test]
    fn test_summary_counts() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("README.md"), "").unwrap();

        let recs = analyze_repository(temp.path());

        // recommendations only contains missing files, so total = missing + present
        assert_eq!(
            recs.summary.total_recommendations,
            recs.recommendations.len() as u32 + recs.summary.total_present
        );
        assert!(recs.summary.total_present >= 1); // README exists
    }

    #[test]
    fn test_node_recommendations() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("package.json"), "{}").unwrap();

        let recs = analyze_repository(temp.path());

        // Should include Node-specific recommendations
        assert!(recs
            .recommendations
            .iter()
            .any(|r| r.file_name.contains("lock")));
        assert!(recs
            .recommendations
            .iter()
            .any(|r| r.file_name.contains("eslint")));
    }

    #[test]
    fn test_rust_recommendations() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();

        let recs = analyze_repository(temp.path());

        // Should include Rust-specific recommendations
        assert!(recs
            .recommendations
            .iter()
            .any(|r| r.file_name == "Cargo.lock"));
        assert!(recs
            .recommendations
            .iter()
            .any(|r| r.file_name == "rustfmt.toml"));
    }
}
