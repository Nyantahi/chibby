//! Ruby recommendations.

use crate::engine::models::{FileRecommendation, RecommendationCategory, RecommendationPriority};
use std::path::Path;

/// Add Ruby specific recommendations.
pub(super) fn add_ruby_recommendations(repo_path: &Path, recs: &mut Vec<FileRecommendation>) {
    // Gemfile.lock
    recs.push(FileRecommendation {
        file_name: "Gemfile.lock".to_string(),
        title: "Gem Lock File".to_string(),
        description: "Locks gem versions for reproducible installs.".to_string(),
        priority: RecommendationPriority::Critical,
        category: RecommendationCategory::Dependencies,
        docs_url: Some("https://bundler.io/guides/faq.html".to_string()),
        exists: repo_path.join("Gemfile.lock").exists(),
        template_hint: Some("Run 'bundle install' to generate".to_string()),
    });

    // Rubocop
    recs.push(FileRecommendation {
        file_name: ".rubocop.yml".to_string(),
        title: "RuboCop Config".to_string(),
        description: "Ruby static code analyzer and formatter.".to_string(),
        priority: RecommendationPriority::High,
        category: RecommendationCategory::CodeQuality,
        docs_url: Some("https://docs.rubocop.org/rubocop/".to_string()),
        exists: repo_path.join(".rubocop.yml").exists(),
        template_hint: Some("Enforce Ruby style guide".to_string()),
    });

    // .ruby-version
    recs.push(FileRecommendation {
        file_name: ".ruby-version".to_string(),
        title: "Ruby Version File".to_string(),
        description: "Specifies Ruby version for rbenv/rvm.".to_string(),
        priority: RecommendationPriority::Medium,
        category: RecommendationCategory::Dependencies,
        docs_url: Some("https://github.com/rbenv/rbenv#choosing-the-ruby-version".to_string()),
        exists: repo_path.join(".ruby-version").exists(),
        template_hint: Some("Just the version, e.g., '3.3.0'".to_string()),
    });
}
