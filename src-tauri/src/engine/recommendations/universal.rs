//! Recommendations that apply to every project regardless of language.

use crate::engine::models::{FileRecommendation, RecommendationCategory, RecommendationPriority};
use std::path::Path;

/// Add universal recommendations (apply to all projects).
pub(super) fn add_universal_recommendations(repo_path: &Path, recs: &mut Vec<FileRecommendation>) {
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
        description: "Maintains consistent coding styles across different editors and IDEs."
            .to_string(),
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
        description: "Track notable changes for each version. Helps users understand what's new."
            .to_string(),
        priority: RecommendationPriority::Medium,
        category: RecommendationCategory::Documentation,
        docs_url: Some("https://keepachangelog.com/".to_string()),
        exists: repo_path.join("CHANGELOG.md").exists()
            || repo_path.join("docs/community/CHANGELOG.md").exists(),
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

    // High: Chibby security gates config
    recs.push(FileRecommendation {
        file_name: ".chibby/gates.toml".to_string(),
        title: "Chibby Security Gates".to_string(),
        description: "Enables Chibby's built-in scanners (secrets, dependency CVEs, SAST, container, IaC, license) and surfaces findings in the Quality tab. Without this file, security stages won't appear when the pipeline is regenerated.".to_string(),
        priority: RecommendationPriority::High,
        category: RecommendationCategory::Security,
        docs_url: None,
        exists: repo_path.join(".chibby").join("gates.toml").exists(),
        template_hint: Some("Run `chibby gates init` or open the Quality tab in the desktop app".to_string()),
    });

    // High: Dedicated security workflow (gitleaks + npm audit, etc.)
    let has_security_workflow = ["security.yml", "security.yaml", "codeql.yml", "codeql.yaml"]
        .iter()
        .any(|name| repo_path.join(".github/workflows").join(name).exists());
    recs.push(FileRecommendation {
        file_name: ".github/workflows/security.yml".to_string(),
        title: "Security Scans Workflow".to_string(),
        description: "Dedicated workflow that runs gitleaks (secret scanning), npm audit / pip-audit / cargo audit (dependency CVEs), and (optionally) CodeQL or Semgrep on every push and weekly schedule.".to_string(),
        priority: RecommendationPriority::High,
        category: RecommendationCategory::Security,
        docs_url: Some("https://github.com/gitleaks/gitleaks-action".to_string()),
        exists: has_security_workflow,
        template_hint: Some("Include: gitleaks, npm audit, dependency-review-action".to_string()),
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
        exists: repo_path.join("CODE_OF_CONDUCT.md").exists()
            || repo_path.join("docs/community/CODE_OF_CONDUCT.md").exists(),
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
