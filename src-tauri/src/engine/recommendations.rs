//! CI/CD Recommendations Engine
//!
//! Analyzes a repository and recommends missing CI/CD configuration files
//! based on detected project types and industry best practices.

use crate::engine::models::{
    FileRecommendation, ProjectRecommendations, RecommendationCategory,
    RecommendationPriority, RecommendationSummary,
};
use std::path::Path;

/// Analyze a repository and generate CI/CD recommendations.
pub fn analyze_repository(repo_path: &Path) -> ProjectRecommendations {
    let project_types = detect_project_types(repo_path);
    let mut recommendations = Vec::new();

    // Add universal recommendations
    add_universal_recommendations(repo_path, &mut recommendations);

    // Add project-type specific recommendations
    for project_type in &project_types {
        match project_type.as_str() {
            "node" | "javascript" | "typescript" => {
                add_node_recommendations(repo_path, &mut recommendations);
            }
            "rust" => {
                add_rust_recommendations(repo_path, &mut recommendations);
            }
            "python" => {
                add_python_recommendations(repo_path, &mut recommendations);
            }
            "go" => {
                add_go_recommendations(repo_path, &mut recommendations);
            }
            "java" | "kotlin" => {
                add_java_recommendations(repo_path, &mut recommendations);
            }
            "dotnet" | "csharp" => {
                add_dotnet_recommendations(repo_path, &mut recommendations);
            }
            "ruby" => {
                add_ruby_recommendations(repo_path, &mut recommendations);
            }
            "php" => {
                add_php_recommendations(repo_path, &mut recommendations);
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
    let missing_recommendations: Vec<FileRecommendation> = recommendations
        .into_iter()
        .filter(|r| !r.exists)
        .collect();

    ProjectRecommendations {
        project_types,
        recommendations: missing_recommendations,
        readiness_score,
        summary,
    }
}

/// Detect project types based on manifest files.
fn detect_project_types(repo_path: &Path) -> Vec<String> {
    let mut types = Vec::new();

    // Node.js / JavaScript / TypeScript
    if repo_path.join("package.json").exists() {
        types.push("node".to_string());
        if repo_path.join("tsconfig.json").exists() {
            types.push("typescript".to_string());
        }
    }

    // Rust
    if repo_path.join("Cargo.toml").exists() {
        types.push("rust".to_string());
    }

    // Python
    if repo_path.join("pyproject.toml").exists()
        || repo_path.join("setup.py").exists()
        || repo_path.join("requirements.txt").exists()
    {
        types.push("python".to_string());
    }

    // Go
    if repo_path.join("go.mod").exists() {
        types.push("go".to_string());
    }

    // Java / Kotlin
    if repo_path.join("pom.xml").exists()
        || repo_path.join("build.gradle").exists()
        || repo_path.join("build.gradle.kts").exists()
    {
        types.push("java".to_string());
    }

    // .NET / C#
    if repo_path.join("global.json").exists()
        || has_extension_in_dir(repo_path, "csproj")
        || has_extension_in_dir(repo_path, "sln")
    {
        types.push("dotnet".to_string());
    }

    // Ruby
    if repo_path.join("Gemfile").exists() {
        types.push("ruby".to_string());
    }

    // PHP
    if repo_path.join("composer.json").exists() {
        types.push("php".to_string());
    }

    // Docker
    if repo_path.join("Dockerfile").exists()
        || repo_path.join("docker-compose.yml").exists()
        || repo_path.join("compose.yml").exists()
    {
        types.push("docker".to_string());
    }

    if types.is_empty() {
        types.push("unknown".to_string());
    }

    types
}

/// Check if directory contains files with given extension.
fn has_extension_in_dir(dir: &Path, ext: &str) -> bool {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(&format!(".{}", ext)) {
                return true;
            }
        }
    }
    false
}

/// Add universal recommendations (apply to all projects).
fn add_universal_recommendations(repo_path: &Path, recs: &mut Vec<FileRecommendation>) {
    // Critical: Version Control
    recs.push(FileRecommendation {
        file_name: ".gitignore".to_string(),
        title: "Git Ignore File".to_string(),
        description: "Prevents committing build artifacts, dependencies, and sensitive files to version control.".to_string(),
        priority: RecommendationPriority::Critical,
        category: RecommendationCategory::VersionControl,
        docs_url: Some("https://git-scm.com/docs/gitignore".to_string()),
        exists: repo_path.join(".gitignore").exists(),
        template_hint: Some("Use gitignore.io to generate for your stack".to_string()),
    });

    // Critical: Documentation
    recs.push(FileRecommendation {
        file_name: "README.md".to_string(),
        title: "Project README".to_string(),
        description: "Essential documentation explaining what the project does, how to install, and how to use it.".to_string(),
        priority: RecommendationPriority::Critical,
        category: RecommendationCategory::Documentation,
        docs_url: Some("https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-readmes".to_string()),
        exists: repo_path.join("README.md").exists() || repo_path.join("readme.md").exists(),
        template_hint: Some("Include: Description, Installation, Usage, Contributing".to_string()),
    });

    // Critical: License
    recs.push(FileRecommendation {
        file_name: "LICENSE".to_string(),
        title: "License File".to_string(),
        description: "Defines how others can use, modify, and distribute your code. Required for open source.".to_string(),
        priority: RecommendationPriority::Critical,
        category: RecommendationCategory::Documentation,
        docs_url: Some("https://choosealicense.com/".to_string()),
        exists: repo_path.join("LICENSE").exists() 
            || repo_path.join("LICENSE.md").exists()
            || repo_path.join("LICENSE.txt").exists(),
        template_hint: Some("MIT, Apache 2.0, or GPL are popular choices".to_string()),
    });

    // High: CI/CD Workflow
    let has_ci = repo_path.join(".github/workflows").exists()
        || repo_path.join(".gitlab-ci.yml").exists()
        || repo_path.join(".circleci").exists()
        || repo_path.join("Jenkinsfile").exists()
        || repo_path.join(".travis.yml").exists();

    recs.push(FileRecommendation {
        file_name: ".github/workflows/ci.yml".to_string(),
        title: "CI Workflow".to_string(),
        description: "Automated testing and building on every push. Catches bugs early and ensures code quality.".to_string(),
        priority: RecommendationPriority::Critical,
        category: RecommendationCategory::CiCd,
        docs_url: Some("https://docs.github.com/en/actions/quickstart".to_string()),
        exists: has_ci,
        template_hint: Some("Run tests, linting, and builds on push/PR".to_string()),
    });

    // High: Editor Config
    recs.push(FileRecommendation {
        file_name: ".editorconfig".to_string(),
        title: "EditorConfig".to_string(),
        description: "Maintains consistent coding styles across different editors and IDEs.".to_string(),
        priority: RecommendationPriority::High,
        category: RecommendationCategory::CodeQuality,
        docs_url: Some("https://editorconfig.org/".to_string()),
        exists: repo_path.join(".editorconfig").exists(),
        template_hint: Some("Define indent style, charset, line endings".to_string()),
    });

    // Medium: Contributing Guide
    recs.push(FileRecommendation {
        file_name: "CONTRIBUTING.md".to_string(),
        title: "Contributing Guide".to_string(),
        description: "Guidelines for contributors on how to submit changes, code style, and PR process.".to_string(),
        priority: RecommendationPriority::Medium,
        category: RecommendationCategory::Documentation,
        docs_url: Some("https://docs.github.com/en/communities/setting-up-your-project-for-healthy-contributions".to_string()),
        exists: repo_path.join("CONTRIBUTING.md").exists() || repo_path.join("docs/community/CONTRIBUTING.md").exists(),
        template_hint: Some("Include: Setup, Code style, PR process, Issue reporting".to_string()),
    });

    // Medium: Changelog
    recs.push(FileRecommendation {
        file_name: "CHANGELOG.md".to_string(),
        title: "Changelog".to_string(),
        description: "Track notable changes for each version. Helps users understand what's new.".to_string(),
        priority: RecommendationPriority::Medium,
        category: RecommendationCategory::Documentation,
        docs_url: Some("https://keepachangelog.com/".to_string()),
        exists: repo_path.join("CHANGELOG.md").exists() || repo_path.join("docs/community/CHANGELOG.md").exists(),
        template_hint: Some("Follow Keep a Changelog format".to_string()),
    });

    // Medium: Security Policy
    recs.push(FileRecommendation {
        file_name: "SECURITY.md".to_string(),
        title: "Security Policy".to_string(),
        description: "Instructions for reporting security vulnerabilities responsibly.".to_string(),
        priority: RecommendationPriority::Medium,
        category: RecommendationCategory::Security,
        docs_url: Some("https://docs.github.com/en/code-security/getting-started/adding-a-security-policy-to-your-repository".to_string()),
        exists: repo_path.join("SECURITY.md").exists() || repo_path.join(".github/SECURITY.md").exists() || repo_path.join("docs/community/SECURITY.md").exists(),
        template_hint: Some("Include: Supported versions, Reporting process".to_string()),
    });

    // Medium: Issue Templates
    let has_issue_templates = repo_path.join(".github/ISSUE_TEMPLATE").exists()
        || repo_path.join(".github/ISSUE_TEMPLATE.md").exists();
    recs.push(FileRecommendation {
        file_name: ".github/ISSUE_TEMPLATE/".to_string(),
        title: "Issue Templates".to_string(),
        description: "Structured templates for bug reports and feature requests.".to_string(),
        priority: RecommendationPriority::Medium,
        category: RecommendationCategory::Documentation,
        docs_url: Some("https://docs.github.com/en/communities/using-templates-to-encourage-useful-issues-and-pull-requests".to_string()),
        exists: has_issue_templates,
        template_hint: Some("Create bug_report.md and feature_request.md".to_string()),
    });

    // Low: Code of Conduct
    recs.push(FileRecommendation {
        file_name: "CODE_OF_CONDUCT.md".to_string(),
        title: "Code of Conduct".to_string(),
        description: "Community standards for respectful and inclusive contributions.".to_string(),
        priority: RecommendationPriority::Low,
        category: RecommendationCategory::Documentation,
        docs_url: Some("https://www.contributor-covenant.org/".to_string()),
        exists: repo_path.join("CODE_OF_CONDUCT.md").exists() || repo_path.join("docs/community/CODE_OF_CONDUCT.md").exists(),
        template_hint: Some("Contributor Covenant is widely used".to_string()),
    });

    // Low: PR Template
    recs.push(FileRecommendation {
        file_name: ".github/PULL_REQUEST_TEMPLATE.md".to_string(),
        title: "PR Template".to_string(),
        description: "Standardized template for pull request descriptions.".to_string(),
        priority: RecommendationPriority::Low,
        category: RecommendationCategory::Documentation,
        docs_url: Some("https://docs.github.com/en/communities/using-templates-to-encourage-useful-issues-and-pull-requests".to_string()),
        exists: repo_path.join(".github/PULL_REQUEST_TEMPLATE.md").exists(),
        template_hint: Some("Include: Description, Type of change, Checklist".to_string()),
    });

    // Low: Dependabot
    recs.push(FileRecommendation {
        file_name: ".github/dependabot.yml".to_string(),
        title: "Dependabot Config".to_string(),
        description: "Automated dependency updates to keep your project secure.".to_string(),
        priority: RecommendationPriority::Low,
        category: RecommendationCategory::Dependencies,
        docs_url: Some("https://docs.github.com/en/code-security/dependabot".to_string()),
        exists: repo_path.join(".github/dependabot.yml").exists(),
        template_hint: Some("Configure update frequency and package ecosystems".to_string()),
    });
}

/// Add Node.js/TypeScript specific recommendations.
fn add_node_recommendations(repo_path: &Path, recs: &mut Vec<FileRecommendation>) {
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
        docs_url: Some("https://docs.npmjs.com/cli/v10/configuring-npm/package-lock-json".to_string()),
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

/// Add Rust specific recommendations.
fn add_rust_recommendations(repo_path: &Path, recs: &mut Vec<FileRecommendation>) {
    // Cargo.lock
    recs.push(FileRecommendation {
        file_name: "Cargo.lock".to_string(),
        title: "Cargo Lock File".to_string(),
        description: "Locks dependency versions for reproducible builds.".to_string(),
        priority: RecommendationPriority::Critical,
        category: RecommendationCategory::Dependencies,
        docs_url: Some("https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html".to_string()),
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

/// Add Python specific recommendations.
fn add_python_recommendations(repo_path: &Path, recs: &mut Vec<FileRecommendation>) {
    // pyproject.toml (modern standard)
    let has_pyproject = repo_path.join("pyproject.toml").exists();
    recs.push(FileRecommendation {
        file_name: "pyproject.toml".to_string(),
        title: "Python Project Config".to_string(),
        description: "Modern Python project configuration (PEP 518/621).".to_string(),
        priority: RecommendationPriority::Critical,
        category: RecommendationCategory::Dependencies,
        docs_url: Some("https://packaging.python.org/en/latest/guides/writing-pyproject-toml/".to_string()),
        exists: has_pyproject,
        template_hint: Some("Replaces setup.py, setup.cfg".to_string()),
    });

    // requirements.txt or lock file
    let has_deps = repo_path.join("requirements.txt").exists()
        || repo_path.join("requirements-dev.txt").exists()
        || repo_path.join("poetry.lock").exists()
        || repo_path.join("Pipfile.lock").exists();

    recs.push(FileRecommendation {
        file_name: "requirements.txt".to_string(),
        title: "Python Dependencies".to_string(),
        description: "Lists project dependencies with pinned versions.".to_string(),
        priority: RecommendationPriority::Critical,
        category: RecommendationCategory::Dependencies,
        docs_url: Some("https://pip.pypa.io/en/stable/reference/requirements-file-format/".to_string()),
        exists: has_deps,
        template_hint: Some("Use 'pip freeze > requirements.txt'".to_string()),
    });

    // Ruff or flake8/black
    let has_linter = repo_path.join("ruff.toml").exists()
        || repo_path.join(".flake8").exists()
        || repo_path.join("pyproject.toml").exists(); // ruff/black can be configured here

    recs.push(FileRecommendation {
        file_name: "ruff.toml".to_string(),
        title: "Ruff Linter Config".to_string(),
        description: "Fast Python linter and formatter (replaces flake8 + black).".to_string(),
        priority: RecommendationPriority::High,
        category: RecommendationCategory::CodeQuality,
        docs_url: Some("https://docs.astral.sh/ruff/".to_string()),
        exists: has_linter,
        template_hint: Some("Modern replacement for flake8, isort, black".to_string()),
    });

    // pytest
    let has_pytest = repo_path.join("pytest.ini").exists()
        || repo_path.join("pyproject.toml").exists()
        || repo_path.join("conftest.py").exists();

    recs.push(FileRecommendation {
        file_name: "pytest.ini".to_string(),
        title: "Pytest Config".to_string(),
        description: "Configuration for Python testing framework.".to_string(),
        priority: RecommendationPriority::High,
        category: RecommendationCategory::Testing,
        docs_url: Some("https://docs.pytest.org/en/stable/reference/customize.html".to_string()),
        exists: has_pytest,
        template_hint: Some("Or configure in pyproject.toml".to_string()),
    });

    // Python version
    recs.push(FileRecommendation {
        file_name: ".python-version".to_string(),
        title: "Python Version File".to_string(),
        description: "Specifies the Python version for pyenv and other tools.".to_string(),
        priority: RecommendationPriority::Medium,
        category: RecommendationCategory::Dependencies,
        docs_url: Some("https://github.com/pyenv/pyenv#choosing-the-python-version".to_string()),
        exists: repo_path.join(".python-version").exists(),
        template_hint: Some("Just the version, e.g., '3.12'".to_string()),
    });
}

/// Add Go specific recommendations.
fn add_go_recommendations(repo_path: &Path, recs: &mut Vec<FileRecommendation>) {
    // go.sum
    recs.push(FileRecommendation {
        file_name: "go.sum".to_string(),
        title: "Go Checksum File".to_string(),
        description: "Cryptographic checksums for module dependencies.".to_string(),
        priority: RecommendationPriority::Critical,
        category: RecommendationCategory::Dependencies,
        docs_url: Some("https://go.dev/ref/mod#go-sum-files".to_string()),
        exists: repo_path.join("go.sum").exists(),
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
        exists: repo_path.join(".golangci.yml").exists() 
            || repo_path.join(".golangci.yaml").exists(),
        template_hint: Some("Enable staticcheck, gosec, errcheck".to_string()),
    });
}

/// Add Java/Kotlin specific recommendations.
fn add_java_recommendations(repo_path: &Path, recs: &mut Vec<FileRecommendation>) {
    // Gradle wrapper
    let has_wrapper = repo_path.join("gradlew").exists()
        || repo_path.join("mvnw").exists();

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
    let has_linter = repo_path.join("checkstyle.xml").exists()
        || repo_path.join(".editorconfig").exists();

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

/// Add .NET/C# specific recommendations.
fn add_dotnet_recommendations(repo_path: &Path, recs: &mut Vec<FileRecommendation>) {
    // .editorconfig for C#
    recs.push(FileRecommendation {
        file_name: ".editorconfig".to_string(),
        title: "EditorConfig with C# Rules".to_string(),
        description: "Code style and analyzer rules for C# projects.".to_string(),
        priority: RecommendationPriority::High,
        category: RecommendationCategory::CodeQuality,
        docs_url: Some("https://learn.microsoft.com/en-us/dotnet/fundamentals/code-analysis/code-style-rule-options".to_string()),
        exists: repo_path.join(".editorconfig").exists(),
        template_hint: Some("Include C# naming and formatting rules".to_string()),
    });

    // Directory.Build.props
    recs.push(FileRecommendation {
        file_name: "Directory.Build.props".to_string(),
        title: "Directory Build Props".to_string(),
        description: "Centralized MSBuild properties for all projects.".to_string(),
        priority: RecommendationPriority::Medium,
        category: RecommendationCategory::Dependencies,
        docs_url: Some("https://learn.microsoft.com/en-us/visualstudio/msbuild/customize-your-build".to_string()),
        exists: repo_path.join("Directory.Build.props").exists(),
        template_hint: Some("Set TreatWarningsAsErrors, nullable, etc.".to_string()),
    });

    // NuGet config
    recs.push(FileRecommendation {
        file_name: "nuget.config".to_string(),
        title: "NuGet Config".to_string(),
        description: "Configure package sources and settings.".to_string(),
        priority: RecommendationPriority::Low,
        category: RecommendationCategory::Dependencies,
        docs_url: Some("https://learn.microsoft.com/en-us/nuget/reference/nuget-config-file".to_string()),
        exists: repo_path.join("nuget.config").exists() 
            || repo_path.join("NuGet.Config").exists(),
        template_hint: Some("Useful for private feeds".to_string()),
    });
}

/// Add Ruby specific recommendations.
fn add_ruby_recommendations(repo_path: &Path, recs: &mut Vec<FileRecommendation>) {
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

/// Add PHP specific recommendations.
fn add_php_recommendations(repo_path: &Path, recs: &mut Vec<FileRecommendation>) {
    // composer.lock
    recs.push(FileRecommendation {
        file_name: "composer.lock".to_string(),
        title: "Composer Lock File".to_string(),
        description: "Locks dependency versions for consistent installs.".to_string(),
        priority: RecommendationPriority::Critical,
        category: RecommendationCategory::Dependencies,
        docs_url: Some("https://getcomposer.org/doc/01-basic-usage.md#installing-dependencies".to_string()),
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
        assert!(recs.recommendations.iter().any(|r| r.file_name == ".gitignore"));
        assert!(recs.recommendations.iter().any(|r| r.file_name == "README.md"));
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
        assert!(recs.recommendations.iter().any(|r| r.file_name.contains("lock")));
        assert!(recs.recommendations.iter().any(|r| r.file_name.contains("eslint")));
    }

    #[test]
    fn test_rust_recommendations() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();

        let recs = analyze_repository(temp.path());

        // Should include Rust-specific recommendations
        assert!(recs.recommendations.iter().any(|r| r.file_name == "Cargo.lock"));
        assert!(recs.recommendations.iter().any(|r| r.file_name == "rustfmt.toml"));
    }
}
