//! Multi-folder / fullstack project structure detection.

#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::engine::models::{
    Backend, DeploymentConfig, DeploymentMethod, Environment, EnvironmentsConfig, FileConflict,
    HealthCheck, Pipeline, PipelineValidation, PipelineWarning, Stage, WarningSeverity,
};
use std::path::Path;

/// Common subdirectory names for fullstack projects (frontend/backend).
pub(crate) const FULLSTACK_SUBDIRS: &[&str] = &[
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

/// Subdirectories that indicate frontend (Node.js) projects.
const FRONTEND_SUBDIRS: &[&str] = &[
    "frontend",
    "client",
    "web",
    "app",
    "admin",
    "dashboard",
    "portal",
];

/// Subdirectories that indicate backend (Python/Node.js) projects.
const BACKEND_SUBDIRS: &[&str] = &["backend", "api", "server"];

/// Information about a detected project folder with its capabilities.
#[derive(Debug, Clone)]
pub struct ProjectFolder {
    /// Component name (e.g., "frontend", "backend", "admin").
    pub name: String,
    /// Path to the component relative to the repo root (POSIX separators). Equals
    /// `name` for a root-level component, or e.g. `main/backend` when the
    /// components live under a wrapper directory.
    pub path: String,
    /// Has package.json (Node.js project).
    pub has_node: bool,
    /// Has requirements.txt or pyproject.toml (Python project).
    pub has_python: bool,
    /// Has Cargo.toml (Rust project).
    pub has_rust: bool,
    /// Has tauri.conf.json (Tauri project in this subdirectory).
    pub has_tauri: bool,
    /// Has tests/ directory or test files.
    pub has_tests: bool,
    /// Available npm scripts (if Node.js project).
    pub npm_scripts: std::collections::HashSet<String>,
    /// Is this a frontend-type folder.
    pub is_frontend: bool,
    /// Is this a backend-type folder.
    pub is_backend: bool,
}

/// Detect all project folders in a fullstack repository.
///
/// Returns information about each subdirectory that contains a recognizable project
/// (package.json, requirements.txt, pyproject.toml, etc.).
pub fn detect_project_folders(repo_path: &Path) -> Vec<ProjectFolder> {
    let mut folders = Vec::new();

    // A project is often a monorepo of several apps under one container, and those
    // apps may sit directly at the repo root OR under a wrapper dir (e.g.
    // `main/frontend`, `main/backend`). Scan the root and each immediate subdir as
    // candidate bases so wrapper layouts are detected too.
    for base in candidate_bases(repo_path) {
        for subdir in FULLSTACK_SUBDIRS {
            let subdir_path = base.join(subdir);
            if !subdir_path.is_dir() {
                continue;
            }
            if let Some(folder) = inspect_component(repo_path, &subdir_path, subdir) {
                // Dedupe by relative path (a dir could match via two bases).
                if !folders
                    .iter()
                    .any(|f: &ProjectFolder| f.path == folder.path)
                {
                    folders.push(folder);
                }
            }
        }
    }

    folders
}

/// Base directories to scan for component folders: the repo root plus each of its
/// immediate subdirectories (potential wrapper dirs), skipping hidden and
/// vendor/build dirs.
pub(crate) fn candidate_bases(repo_path: &Path) -> Vec<std::path::PathBuf> {
    let mut bases = vec![repo_path.to_path_buf()];
    if let Ok(entries) = std::fs::read_dir(repo_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || is_wrapper_skip_dir(&name) {
                continue;
            }
            bases.push(path);
        }
    }
    bases
}

/// Dirs that are never a monorepo wrapper worth descending into.
fn is_wrapper_skip_dir(name: &str) -> bool {
    matches!(
        name,
        "node_modules" | "target" | "dist" | "build" | "out" | "vendor" | ".git" | "venv" | ".venv"
    )
}

/// Inspect a candidate component directory; returns a `ProjectFolder` when it
/// holds a recognizable project (Node/Python/Rust).
fn inspect_component(repo_path: &Path, subdir_path: &Path, name: &str) -> Option<ProjectFolder> {
    let has_package_json = subdir_path.join("package.json").exists();
    let has_requirements = subdir_path.join("requirements.txt").exists();
    let has_pyproject = subdir_path.join("pyproject.toml").exists();
    let has_python = has_requirements || has_pyproject;
    let has_rust = subdir_path.join("Cargo.toml").exists();
    let has_tauri = subdir_path.join("tauri.conf.json").exists();

    if !has_package_json && !has_python && !has_rust {
        return None;
    }

    let has_tests = subdir_path.join("tests").is_dir()
        || subdir_path.join("test").is_dir()
        || subdir_path.join("__tests__").is_dir()
        || subdir_path.join("vitest.config.ts").exists()
        || subdir_path.join("jest.config.js").exists()
        || subdir_path.join("pytest.ini").exists()
        || subdir_path.join("conftest.py").exists();

    let npm_scripts = if has_package_json {
        read_package_scripts(subdir_path).keys().cloned().collect()
    } else {
        std::collections::HashSet::new()
    };

    let is_frontend =
        FRONTEND_SUBDIRS.contains(&name.to_lowercase().as_str()) || has_package_json && !has_python;
    let is_backend = BACKEND_SUBDIRS.contains(&name.to_lowercase().as_str()) || has_python;

    // Relative path from the repo root (POSIX separators) for correct `cd`.
    let path = subdir_path
        .strip_prefix(repo_path)
        .unwrap_or(subdir_path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");

    Some(ProjectFolder {
        name: name.to_string(),
        path,
        has_node: has_package_json,
        has_python,
        has_rust,
        has_tauri,
        has_tests,
        npm_scripts,
        is_frontend,
        is_backend,
    })
}

/// Check if project is a fullstack Docker project (multiple folders + docker-compose).
pub fn is_fullstack_docker_project(repo_path: &Path) -> bool {
    let folders = detect_project_folders(repo_path);

    // Check for any docker-compose file (including variants like docker-compose.prod.yml)
    let has_docker_compose = has_any_docker_compose(repo_path);

    // Check for GitHub Actions deploy workflows
    let has_deploy_workflow = repo_path.join(".github/workflows").is_dir()
        && std::fs::read_dir(repo_path.join(".github/workflows"))
            .map(|entries| {
                entries.flatten().any(|e| {
                    let name = e.file_name().to_string_lossy().to_lowercase();
                    (name.contains("deploy") || name.contains("release"))
                        && (name.ends_with(".yml") || name.ends_with(".yaml"))
                })
            })
            .unwrap_or(false);

    // Consider fullstack if we have 2+ project folders and (docker-compose or deploy workflow)
    folders.len() >= 2 && (has_docker_compose || has_deploy_workflow)
}

/// Check if any docker-compose file exists (including variants).
pub(crate) fn has_any_docker_compose(repo_path: &Path) -> bool {
    // Check standard files first
    if repo_path.join("docker-compose.yml").exists()
        || repo_path.join("docker-compose.yaml").exists()
        || repo_path.join("compose.yml").exists()
        || repo_path.join("compose.yaml").exists()
    {
        return true;
    }

    // Check for variants like docker-compose.prod.yml
    if let Ok(entries) = std::fs::read_dir(repo_path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if is_docker_compose_file(&name) {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_detect_project_folders_empty() {
        let temp = TempDir::new().unwrap();
        let folders = detect_project_folders(temp.path());
        assert!(folders.is_empty());
    }

    #[test]
    fn test_detect_project_folders_single_frontend() {
        let temp = TempDir::new().unwrap();

        // Create frontend directory with package.json
        let frontend = temp.path().join("frontend");
        std::fs::create_dir(&frontend).unwrap();
        std::fs::write(
            frontend.join("package.json"),
            r#"{"name": "frontend", "scripts": {"build": "vite build", "test": "vitest"}}"#,
        )
        .unwrap();

        let folders = detect_project_folders(temp.path());
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].name, "frontend");
        assert!(folders[0].has_node);
        assert!(!folders[0].has_python);
        assert!(folders[0].npm_scripts.contains("build"));
        assert!(folders[0].npm_scripts.contains("test"));
    }

    #[test]
    fn test_detect_project_folders_fullstack() {
        let temp = TempDir::new().unwrap();

        // Create frontend directory
        let frontend = temp.path().join("frontend");
        std::fs::create_dir(&frontend).unwrap();
        std::fs::write(
            frontend.join("package.json"),
            r#"{"name": "frontend", "scripts": {"build": "vite build"}}"#,
        )
        .unwrap();

        // Create backend directory with Python
        let backend = temp.path().join("backend");
        std::fs::create_dir(&backend).unwrap();
        std::fs::write(backend.join("requirements.txt"), "flask\npytest").unwrap();
        std::fs::create_dir(backend.join("tests")).unwrap();

        let folders = detect_project_folders(temp.path());
        assert_eq!(folders.len(), 2);

        // Check frontend
        let fe = folders.iter().find(|f| f.name == "frontend").unwrap();
        assert!(fe.has_node);
        assert!(!fe.has_python);

        // Check backend
        let be = folders.iter().find(|f| f.name == "backend").unwrap();
        assert!(!be.has_node);
        assert!(be.has_python);
        assert!(be.has_tests);
    }

    #[test]
    fn test_detect_project_folders_with_admin() {
        let temp = TempDir::new().unwrap();

        // Create frontend, backend, and admin directories
        for name in &["frontend", "backend", "admin"] {
            let dir = temp.path().join(name);
            std::fs::create_dir(&dir).unwrap();
            std::fs::write(
                dir.join("package.json"),
                format!(r#"{{"name": "{}", "scripts": {{"test": "vitest"}}}}"#, name),
            )
            .unwrap();
        }

        let folders = detect_project_folders(temp.path());
        assert_eq!(folders.len(), 3);
        assert!(folders.iter().any(|f| f.name == "frontend"));
        assert!(folders.iter().any(|f| f.name == "backend"));
        assert!(folders.iter().any(|f| f.name == "admin"));
    }

    #[test]
    fn test_detect_project_folders_under_wrapper_dir() {
        // Monorepo nested under a container dir: repo/main/{frontend,backend,admin}.
        let temp = TempDir::new().unwrap();
        let main = temp.path().join("main");
        for name in &["frontend", "admin"] {
            let d = main.join(name);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(
                d.join("package.json"),
                r#"{"scripts":{"build":"vite build"}}"#,
            )
            .unwrap();
        }
        let backend = main.join("backend");
        std::fs::create_dir_all(&backend).unwrap();
        std::fs::write(backend.join("requirements.txt"), "flask").unwrap();

        let folders = detect_project_folders(temp.path());
        assert_eq!(folders.len(), 3, "got {folders:?}");
        let be = folders.iter().find(|f| f.name == "backend").unwrap();
        assert_eq!(be.path, "main/backend");
        assert!(be.has_python);
        let fe = folders.iter().find(|f| f.name == "frontend").unwrap();
        assert_eq!(fe.path, "main/frontend");
        assert!(fe.has_node);
    }

    #[test]
    fn test_is_fullstack_docker_project() {
        let temp = TempDir::new().unwrap();

        // Not fullstack without docker-compose
        let frontend = temp.path().join("frontend");
        std::fs::create_dir(&frontend).unwrap();
        std::fs::write(frontend.join("package.json"), r#"{"name": "frontend"}"#).unwrap();

        let backend = temp.path().join("backend");
        std::fs::create_dir(&backend).unwrap();
        std::fs::write(backend.join("requirements.txt"), "flask").unwrap();

        assert!(!is_fullstack_docker_project(temp.path()));

        // Add docker-compose
        std::fs::write(temp.path().join("docker-compose.yml"), "version: '3'").unwrap();
        assert!(is_fullstack_docker_project(temp.path()));
    }
}
