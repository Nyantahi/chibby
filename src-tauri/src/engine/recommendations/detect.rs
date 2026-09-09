//! Project-type detection from manifest files, plus fullstack heuristics.

use std::path::Path;

/// Detect project types based on manifest files.
pub(super) fn detect_project_types(repo_path: &Path) -> Vec<String> {
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

    // Fullstack detection: React/Node frontend + Python backend + Docker
    // Check both root and common subdirectories (frontend/, backend/, etc.)
    let has_react = has_react_project(repo_path);
    let has_python_backend = has_python_project(repo_path);
    let has_docker = repo_path.join("docker-compose.yml").exists()
        || repo_path.join("compose.yml").exists()
        || repo_path.join("docker-compose.yaml").exists()
        || repo_path.join("compose.yaml").exists();

    if has_react && has_python_backend {
        types.push("fullstack".to_string());
    }
    if has_react && has_python_backend && has_docker {
        types.push("fullstack-docker".to_string());
    }

    if types.is_empty() {
        types.push("unknown".to_string());
    }

    types
}

/// Common subdirectory names for fullstack projects.
const FULLSTACK_SUBDIRS: &[&str] = &[
    "frontend",
    "backend",
    "api",
    "web",
    "app",
    "client",
    "server",
    "src",
    "admin",
    "dashboard",
    "portal",
];

/// Check if package.json contains React as a dependency.
fn has_react_dependency(pkg_path: &Path) -> bool {
    if let Ok(content) = std::fs::read_to_string(pkg_path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            // Check dependencies and devDependencies for react
            let deps = json.get("dependencies").and_then(|d| d.as_object());
            let dev_deps = json.get("devDependencies").and_then(|d| d.as_object());

            if let Some(deps) = deps {
                if deps.contains_key("react")
                    || deps.contains_key("next")
                    || deps.contains_key("vue")
                {
                    return true;
                }
            }
            if let Some(dev_deps) = dev_deps {
                if dev_deps.contains_key("react")
                    || dev_deps.contains_key("next")
                    || dev_deps.contains_key("vue")
                {
                    return true;
                }
            }
        }
    }
    false
}

/// Check if the project has React/Node frontend (in root or subdirectories).
fn has_react_project(repo_path: &Path) -> bool {
    // Check root
    let root_pkg = repo_path.join("package.json");
    if root_pkg.exists()
        && (has_react_dependency(&root_pkg)
            || repo_path.join("vite.config.ts").exists()
            || repo_path.join("vite.config.js").exists()
            || repo_path.join("next.config.js").exists()
            || repo_path.join("next.config.mjs").exists())
    {
        return true;
    }

    // Check common subdirectories
    for subdir in FULLSTACK_SUBDIRS {
        let subdir_path = repo_path.join(subdir);
        let pkg_path = subdir_path.join("package.json");
        if pkg_path.exists()
            && (has_react_dependency(&pkg_path)
                || subdir_path.join("vite.config.ts").exists()
                || subdir_path.join("vite.config.js").exists()
                || subdir_path.join("next.config.js").exists()
                || subdir_path.join("next.config.mjs").exists())
        {
            return true;
        }
    }

    false
}

/// Check if the project has a Python backend (in root or subdirectories).
fn has_python_project(repo_path: &Path) -> bool {
    // Check root
    if repo_path.join("pyproject.toml").exists()
        || repo_path.join("requirements.txt").exists()
        || has_python_backend_framework(repo_path)
    {
        return true;
    }

    // Check common subdirectories
    for subdir in FULLSTACK_SUBDIRS {
        let subdir_path = repo_path.join(subdir);
        if subdir_path.join("pyproject.toml").exists()
            || subdir_path.join("requirements.txt").exists()
            || subdir_path.join("setup.py").exists()
            || subdir_path.join("main.py").exists()
            || subdir_path.join("app.py").exists()
        {
            return true;
        }
    }

    false
}

/// Check if the project has a Python backend framework (FastAPI, Django, Flask).
fn has_python_backend_framework(repo_path: &Path) -> bool {
    // Check requirements.txt for backend frameworks
    let req_path = repo_path.join("requirements.txt");
    if let Ok(content) = std::fs::read_to_string(&req_path) {
        let content_lower = content.to_lowercase();
        if content_lower.contains("fastapi")
            || content_lower.contains("django")
            || content_lower.contains("flask")
            || content_lower.contains("starlette")
        {
            return true;
        }
    }

    // Check pyproject.toml for backend frameworks
    let pyproject_path = repo_path.join("pyproject.toml");
    if let Ok(content) = std::fs::read_to_string(&pyproject_path) {
        let content_lower = content.to_lowercase();
        if content_lower.contains("fastapi")
            || content_lower.contains("django")
            || content_lower.contains("flask")
        {
            return true;
        }
    }

    // Check for common backend entry point files
    repo_path.join("app.py").exists()
        || repo_path.join("main.py").exists()
        || repo_path.join("manage.py").exists()
        || repo_path.join("wsgi.py").exists()
        || repo_path.join("asgi.py").exists()
}

/// Check if the project has Python test files (test_*.py or *_test.py).
pub(super) fn has_python_test_files(repo_path: &Path) -> bool {
    // Check root directory
    if let Ok(entries) = std::fs::read_dir(repo_path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".py") {
                let base = &name[..name.len() - 3];
                if base.starts_with("test_") || base.ends_with("_test") {
                    return true;
                }
            }
        }
    }

    // Check tests/ and test/ directories
    for test_dir in &["tests", "test"] {
        let dir_path = repo_path.join(test_dir);
        if let Ok(entries) = std::fs::read_dir(&dir_path) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".py") {
                    let base = &name[..name.len() - 3];
                    if base.starts_with("test_") || base.ends_with("_test") {
                        return true;
                    }
                }
            }
        }
    }

    false
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
