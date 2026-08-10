//! Java / Kotlin recommendations.

use crate::engine::models::{FileRecommendation, RecommendationCategory, RecommendationPriority};
use std::path::Path;

/// Add Java/Kotlin specific recommendations.
pub(super) fn add_java_recommendations(repo_path: &Path, recs: &mut Vec<FileRecommendation>) {
    // Gradle wrapper
    let has_wrapper = repo_path.join("gradlew").exists() || repo_path.join("mvnw").exists();

    recs.push(FileRecommendation {
        file_name: "gradlew".to_string(),
        title: "Build Wrapper".to_string(),
        description: "Ensures consistent build tool version across environments.".to_string(),
        priority: RecommendationPriority::High,
        category: RecommendationCategory::Dependencies,
        docs_url: Some("https://docs.gradle.org/current/userguide/gradle_wrapper.html".to_string()),
        exists: has_wrapper,
        template_hint: Some("Run 'gradle wrapper' to generate".to_string()),
    });

    // Checkstyle or similar
    let has_linter =
        repo_path.join("checkstyle.xml").exists() || repo_path.join(".editorconfig").exists();

    recs.push(FileRecommendation {
        file_name: "checkstyle.xml".to_string(),
        title: "Checkstyle Config".to_string(),
        description: "Java code style checking and enforcement.".to_string(),
        priority: RecommendationPriority::Medium,
        category: RecommendationCategory::CodeQuality,
        docs_url: Some("https://checkstyle.sourceforge.io/".to_string()),
        exists: has_linter,
        template_hint: Some("Use Google or Sun style guide".to_string()),
    });
}
