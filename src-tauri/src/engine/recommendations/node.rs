//! Node.js / JavaScript / TypeScript recommendations.

use crate::engine::models::{FileRecommendation, RecommendationCategory, RecommendationPriority};
use std::path::Path;

/// Add Node.js/TypeScript specific recommendations.
pub(super) fn add_node_recommendations(repo_path: &Path, recs: &mut Vec<FileRecommendation>) {
    // Lock file
    let has_lock = repo_path.join("package-lock.json").exists()
        || repo_path.join("yarn.lock").exists()
        || repo_path.join("pnpm-lock.yaml").exists()
        || repo_path.join("bun.lockb").exists();

    recs.push(FileRecommendation {
        file_name: "package-lock.json".to_string(),
        title: "Package Lock File".to_string(),
        description: "Ensures reproducible builds by locking dependency versions.".to_string(),
        priority: RecommendationPriority::Critical,
        category: RecommendationCategory::Dependencies,
        docs_url: Some(
            "https://docs.npmjs.com/cli/v10/configuring-npm/package-lock-json".to_string(),
        ),
        exists: has_lock,
        template_hint: Some("Run 'npm install' to generate".to_string()),
    });

    // ESLint
    let has_eslint = repo_path.join(".eslintrc").exists()
        || repo_path.join(".eslintrc.js").exists()
        || repo_path.join(".eslintrc.json").exists()
        || repo_path.join(".eslintrc.cjs").exists()
        || repo_path.join("eslint.config.js").exists()
        || repo_path.join("eslint.config.mjs").exists();

    recs.push(FileRecommendation {
        file_name: "eslint.config.js".to_string(),
        title: "ESLint Config".to_string(),
        description: "Identifies and fixes problems in JavaScript/TypeScript code.".to_string(),
        priority: RecommendationPriority::High,
        category: RecommendationCategory::CodeQuality,
        docs_url: Some("https://eslint.org/docs/latest/use/getting-started".to_string()),
        exists: has_eslint,
        template_hint: Some("Use flat config format (eslint.config.js)".to_string()),
    });

    // Prettier
    let has_prettier = repo_path.join(".prettierrc").exists()
        || repo_path.join(".prettierrc.js").exists()
        || repo_path.join(".prettierrc.json").exists()
        || repo_path.join("prettier.config.js").exists();

    recs.push(FileRecommendation {
        file_name: ".prettierrc".to_string(),
        title: "Prettier Config".to_string(),
        description: "Automatic code formatting for consistent style.".to_string(),
        priority: RecommendationPriority::High,
        category: RecommendationCategory::CodeQuality,
        docs_url: Some("https://prettier.io/docs/en/configuration.html".to_string()),
        exists: has_prettier,
        template_hint: Some("Define tab width, semicolons, quotes".to_string()),
    });

    // TypeScript config (if TS detected)
    if repo_path.join("tsconfig.json").exists() {
        recs.push(FileRecommendation {
            file_name: "tsconfig.json".to_string(),
            title: "TypeScript Config".to_string(),
            description: "TypeScript compiler configuration for type checking.".to_string(),
            priority: RecommendationPriority::Critical,
            category: RecommendationCategory::CodeQuality,
            docs_url: Some("https://www.typescriptlang.org/tsconfig".to_string()),
            exists: true,
            template_hint: None,
        });
    }

    // nvmrc
    recs.push(FileRecommendation {
        file_name: ".nvmrc".to_string(),
        title: "Node Version File".to_string(),
        description: "Specifies the Node.js version for the project.".to_string(),
        priority: RecommendationPriority::Medium,
        category: RecommendationCategory::Dependencies,
        docs_url: Some("https://github.com/nvm-sh/nvm#nvmrc".to_string()),
        exists: repo_path.join(".nvmrc").exists() || repo_path.join(".node-version").exists(),
        template_hint: Some("Just the version number, e.g., '20'".to_string()),
    });

    // Test config
    let has_test_config = repo_path.join("jest.config.js").exists()
        || repo_path.join("jest.config.ts").exists()
        || repo_path.join("vitest.config.ts").exists()
        || repo_path.join("vitest.config.js").exists();

    recs.push(FileRecommendation {
        file_name: "vitest.config.ts".to_string(),
        title: "Test Config".to_string(),
        description: "Configuration for running unit and integration tests.".to_string(),
        priority: RecommendationPriority::High,
        category: RecommendationCategory::Testing,
        docs_url: Some("https://vitest.dev/config/".to_string()),
        exists: has_test_config,
        template_hint: Some("Vitest is fast and Vite-compatible".to_string()),
    });
}
