//! Python recommendations.

use super::detect::has_python_test_files;
use crate::engine::models::{FileRecommendation, RecommendationCategory, RecommendationPriority};
use std::path::Path;

/// Add Python specific recommendations.
pub(super) fn add_python_recommendations(repo_path: &Path, recs: &mut Vec<FileRecommendation>) {
    // pyproject.toml (modern standard)
    let has_pyproject = repo_path.join("pyproject.toml").exists();
    recs.push(FileRecommendation {
        file_name: "pyproject.toml".to_string(),
        title: "Python Project Config".to_string(),
        description: "Modern Python project configuration (PEP 518/621).".to_string(),
        priority: RecommendationPriority::Critical,
        category: RecommendationCategory::Dependencies,
        docs_url: Some(
            "https://packaging.python.org/en/latest/guides/writing-pyproject-toml/".to_string(),
        ),
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
        docs_url: Some(
            "https://pip.pypa.io/en/stable/reference/requirements-file-format/".to_string(),
        ),
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

    // pytest config
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

    // Python test directory
    let has_test_dir = repo_path.join("tests").is_dir() || repo_path.join("test").is_dir();
    let has_test_files = has_python_test_files(repo_path);

    recs.push(FileRecommendation {
        file_name: "tests/".to_string(),
        title: "Python Test Directory".to_string(),
        description: "Directory containing Python test files (test_*.py or *_test.py).".to_string(),
        priority: RecommendationPriority::High,
        category: RecommendationCategory::Testing,
        docs_url: Some(
            "https://docs.pytest.org/en/stable/explanation/goodpractices.html".to_string(),
        ),
        exists: has_test_dir || has_test_files,
        template_hint: Some("Create tests/ with test_*.py files".to_string()),
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
