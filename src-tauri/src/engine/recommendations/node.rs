//! Node.js / JavaScript / TypeScript recommendations.

use super::exists_nested;
use crate::engine::models::{FileRecommendation, RecommendationCategory, RecommendationPriority};
use std::path::Path;

/// Add Node.js/TypeScript specific recommendations.
pub(super) fn add_node_recommendations(repo_path: &Path, recs: &mut Vec<FileRecommendation>) {
    // Lock file
    let has_lock = exists_nested(
        repo_path,
        &[
            "package-lock.json",
            "yarn.lock",
            "pnpm-lock.yaml",
            "bun.lockb",
        ],
    );

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
    let has_eslint = exists_nested(
        repo_path,
        &[
            ".eslintrc",
            ".eslintrc.js",
            ".eslintrc.json",
            ".eslintrc.cjs",
            "eslint.config.js",
            "eslint.config.mjs",
        ],
    );

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
    let has_prettier = exists_nested(
        repo_path,
        &[
            ".prettierrc",
            ".prettierrc.js",
            ".prettierrc.json",
            "prettier.config.js",
        ],
    );

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
    if exists_nested(repo_path, &["tsconfig.json"]) {
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
        exists: exists_nested(repo_path, &[".nvmrc", ".node-version"]),
        template_hint: Some("Just the version number, e.g., '20'".to_string()),
    });

    // Test config
    let has_test_config = exists_nested(
        repo_path,
        &[
            "jest.config.js",
            "jest.config.ts",
            "vitest.config.ts",
            "vitest.config.js",
        ],
    );

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_config_detected_in_component_dir() {
        // Monorepo: no root package.json; the app + its config live in frontend/.
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("frontend")).unwrap();
        std::fs::write(tmp.path().join("frontend/package.json"), "{}").unwrap();
        std::fs::write(tmp.path().join("frontend/vitest.config.ts"), "").unwrap();
        std::fs::write(tmp.path().join("frontend/eslint.config.js"), "").unwrap();
        std::fs::write(tmp.path().join("frontend/package-lock.json"), "{}").unwrap();

        let mut recs = Vec::new();
        add_node_recommendations(tmp.path(), &mut recs);
        let find = |name: &str| recs.iter().find(|r| r.file_name == name).unwrap();

        // Present in frontend/ -> recognized, not falsely "missing".
        assert!(
            find("vitest.config.ts").exists,
            "vitest config in frontend/ should count"
        );
        assert!(find("eslint.config.js").exists);
        assert!(find("package-lock.json").exists);
        // Genuinely absent -> still flagged.
        assert!(!find(".prettierrc").exists);
    }
}
