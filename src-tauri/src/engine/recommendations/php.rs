//! PHP recommendations.

use crate::engine::models::{FileRecommendation, RecommendationCategory, RecommendationPriority};
use std::path::Path;

/// Add PHP specific recommendations.
pub(super) fn add_php_recommendations(repo_path: &Path, recs: &mut Vec<FileRecommendation>) {
    // composer.lock
    recs.push(FileRecommendation {
        file_name: "composer.lock".to_string(),
        title: "Composer Lock File".to_string(),
        description: "Locks dependency versions for consistent installs.".to_string(),
        priority: RecommendationPriority::Critical,
        category: RecommendationCategory::Dependencies,
        docs_url: Some(
            "https://getcomposer.org/doc/01-basic-usage.md#installing-dependencies".to_string(),
        ),
        exists: repo_path.join("composer.lock").exists(),
        template_hint: Some("Run 'composer install' to generate".to_string()),
    });

    // PHP CS Fixer
    recs.push(FileRecommendation {
        file_name: ".php-cs-fixer.php".to_string(),
        title: "PHP CS Fixer Config".to_string(),
        description: "PHP coding standards fixer configuration.".to_string(),
        priority: RecommendationPriority::High,
        category: RecommendationCategory::CodeQuality,
        docs_url: Some("https://cs.symfony.com/doc/config.html".to_string()),
        exists: repo_path.join(".php-cs-fixer.php").exists()
            || repo_path.join(".php-cs-fixer.dist.php").exists(),
        template_hint: Some("Use PSR-12 or Symfony style".to_string()),
    });

    // PHPStan
    recs.push(FileRecommendation {
        file_name: "phpstan.neon".to_string(),
        title: "PHPStan Config".to_string(),
        description: "PHP static analysis tool for finding bugs.".to_string(),
        priority: RecommendationPriority::High,
        category: RecommendationCategory::CodeQuality,
        docs_url: Some("https://phpstan.org/config-reference".to_string()),
        exists: repo_path.join("phpstan.neon").exists()
            || repo_path.join("phpstan.neon.dist").exists(),
        template_hint: Some("Start with level 5, work up to 9".to_string()),
    });
}
