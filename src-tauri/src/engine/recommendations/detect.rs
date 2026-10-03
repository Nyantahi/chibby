//! Project-type detection from manifest files, plus fullstack heuristics.

use crate::engine::detector::{
    candidate_bases, exists_within, is_android_project, is_xcode_bundle, DEEP_NESTED_SCAN_DEPTH,
    FULLSTACK_SUBDIRS, NESTED_SCAN_DEPTH,
};
use std::path::Path;

/// Whether a file named `name` exists at or below the repo root within the
/// shallow scan depth. Manifests nest in monorepo / mobile / `src/<App>/`
/// layouts, so detection must look past the root to avoid "unknown".
fn has_nested_file(repo_path: &Path, name: &str) -> bool {
    exists_within(repo_path, NESTED_SCAN_DEPTH, &|n: &str| n == name)
}

/// Whether a file with extension `ext` exists at or below the repo root within
/// the shallow scan depth (e.g. a nested `src/MyApp/MyApp.csproj`).
fn has_nested_ext(repo_path: &Path, ext: &str) -> bool {
    let suffix = format!(".{ext}");
    exists_within(repo_path, NESTED_SCAN_DEPTH, &|n: &str| {
        n.ends_with(&suffix)
    })
}

/// Detect project types based on manifest files. Every build system is matched
/// with a shallow nested walk (not root-only) so monorepo / mobile layouts are
/// classified instead of falling back to "unknown".
pub fn detect_project_types(repo_path: &Path) -> Vec<String> {
    let mut types = Vec::new();

    // Node.js / JavaScript / TypeScript
    if has_nested_file(repo_path, "package.json") {
        types.push("node".to_string());
        if has_nested_file(repo_path, "tsconfig.json") {
            types.push("typescript".to_string());
        }
    }

    // Rust (covers the standard Tauri layout at `src-tauri/Cargo.toml`)
    if has_nested_file(repo_path, "Cargo.toml") {
        types.push("rust".to_string());
    }

    // Python
    if has_nested_file(repo_path, "pyproject.toml")
        || has_nested_file(repo_path, "setup.py")
        || has_nested_file(repo_path, "requirements.txt")
    {
        types.push("python".to_string());
    }

    // Go
    if has_nested_file(repo_path, "go.mod") {
        types.push("go".to_string());
    }

    // Java / Kotlin / Android. Gradle modules often nest below the root
    // (`mobile/app/build.gradle.kts`).
    let has_gradle_kts = has_nested_file(repo_path, "build.gradle.kts");
    let has_build_system = has_nested_file(repo_path, "pom.xml")
        || has_nested_file(repo_path, "build.gradle")
        || has_gradle_kts;
    if has_build_system {
        types.push("java".to_string());
        // Kotlin DSL build scripts signal a Kotlin project.
        if has_gradle_kts {
            types.push("kotlin".to_string());
        }
        if is_android_project(repo_path) {
            types.push("android".to_string());
        }
    }

    // Swift / iOS. Xcode bundles can sit several levels down in mobile monorepos
    // (`apps/ios/App/App.xcworkspace`), so search deeper than the default.
    let has_xcode_app = exists_within(repo_path, DEEP_NESTED_SCAN_DEPTH, &is_xcode_bundle);
    let has_spm = exists_within(repo_path, DEEP_NESTED_SCAN_DEPTH, &|n: &str| {
        n == "Package.swift"
    });
    if has_spm || has_xcode_app {
        types.push("swift".to_string());
        // An Xcode project/workspace signals an iOS app; an SPM package alone does not.
        if has_xcode_app {
            types.push("ios".to_string());
        }
    }

    // .NET / C#
    if has_nested_file(repo_path, "global.json")
        || has_nested_ext(repo_path, "csproj")
        || has_nested_ext(repo_path, "sln")
    {
        types.push("dotnet".to_string());
    }

    // Ruby
    if has_nested_file(repo_path, "Gemfile") {
        types.push("ruby".to_string());
    }

    // PHP
    if has_nested_file(repo_path, "composer.json") {
        types.push("php".to_string());
    }

    // C / C++ (CMake or Meson)
    if has_nested_file(repo_path, "CMakeLists.txt") || has_nested_file(repo_path, "meson.build") {
        types.push("cpp".to_string());
    }

    // Docker
    if has_nested_file(repo_path, "Dockerfile")
        || has_nested_file(repo_path, "docker-compose.yml")
        || has_nested_file(repo_path, "compose.yml")
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

    // Check common subdirectories, including one wrapper level down (monorepos
    // that nest apps under a container dir, e.g. `main/frontend`).
    for base in candidate_bases(repo_path) {
        for subdir in FULLSTACK_SUBDIRS {
            let subdir_path = base.join(subdir);
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

    // Check common subdirectories, including one wrapper level down.
    for base in candidate_bases(repo_path) {
        for subdir in FULLSTACK_SUBDIRS {
            let subdir_path = base.join(subdir);
            if subdir_path.join("pyproject.toml").exists()
                || subdir_path.join("requirements.txt").exists()
                || subdir_path.join("setup.py").exists()
                || subdir_path.join("main.py").exists()
                || subdir_path.join("app.py").exists()
            {
                return true;
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn touch(dir: &Path, rel: &str) {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, "").unwrap();
    }

    #[test]
    fn test_detect_swift_spm() {
        let tmp = tempfile::tempdir().unwrap();
        touch(tmp.path(), "Package.swift");
        let types = detect_project_types(tmp.path());
        assert!(types.contains(&"swift".to_string()));
        // SPM package is not an app, so no "ios".
        assert!(!types.contains(&"ios".to_string()));
    }

    #[test]
    fn test_detect_swift_ios_app() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("MyApp.xcodeproj")).unwrap();
        let types = detect_project_types(tmp.path());
        assert!(types.contains(&"swift".to_string()));
        assert!(types.contains(&"ios".to_string()));
    }

    #[test]
    fn test_detect_swift_ios_app_nested() {
        // Standard iOS layout nests the project bundle a level down.
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("KibokoKids/KibokoKids.xcodeproj")).unwrap();
        let types = detect_project_types(tmp.path());
        assert!(types.contains(&"swift".to_string()));
        assert!(types.contains(&"ios".to_string()));
        assert!(!types.contains(&"unknown".to_string()));
    }

    #[test]
    fn test_detect_kotlin_android() {
        let tmp = tempfile::tempdir().unwrap();
        touch(tmp.path(), "build.gradle.kts");
        touch(tmp.path(), "app/src/main/AndroidManifest.xml");
        let types = detect_project_types(tmp.path());
        assert!(types.contains(&"kotlin".to_string()));
        assert!(types.contains(&"android".to_string()));
    }

    #[test]
    fn test_detect_rust_tauri_layout() {
        // Standard Tauri repo: Cargo.toml lives at src-tauri/, not root.
        let tmp = tempfile::tempdir().unwrap();
        touch(tmp.path(), "src-tauri/Cargo.toml");
        let types = detect_project_types(tmp.path());
        assert!(types.contains(&"rust".to_string()));
        assert!(!types.contains(&"unknown".to_string()));
    }

    #[test]
    fn test_detect_nested_single_build_systems() {
        // go.mod / pom.xml / Gemfile / composer.json nested in a subdir.
        for (rel, expected) in [
            ("services/api/go.mod", "go"),
            ("backend/pom.xml", "java"),
            ("api/Gemfile", "ruby"),
            ("web/composer.json", "php"),
            ("native/App.xcodeproj/project.pbxproj", "ios"),
        ] {
            let tmp = tempfile::tempdir().unwrap();
            touch(tmp.path(), rel);
            let types = detect_project_types(tmp.path());
            assert!(
                types.contains(&expected.to_string()),
                "{rel} should detect {expected}, got {types:?}"
            );
        }
    }

    #[test]
    fn test_detect_cpp_cmake_and_meson() {
        let tmp = tempfile::tempdir().unwrap();
        touch(tmp.path(), "src/CMakeLists.txt");
        assert!(detect_project_types(tmp.path()).contains(&"cpp".to_string()));

        let tmp2 = tempfile::tempdir().unwrap();
        touch(tmp2.path(), "meson.build");
        assert!(detect_project_types(tmp2.path()).contains(&"cpp".to_string()));
    }

    #[test]
    fn test_detect_dotnet_nested_csproj() {
        let tmp = tempfile::tempdir().unwrap();
        touch(tmp.path(), "src/MyApp/MyApp.csproj");
        assert!(detect_project_types(tmp.path()).contains(&"dotnet".to_string()));
    }

    #[test]
    fn test_detect_kotlin_android_nested() {
        // Gradle module + manifest nested below the root (monorepo layout).
        let tmp = tempfile::tempdir().unwrap();
        touch(tmp.path(), "mobile/app/build.gradle.kts");
        touch(tmp.path(), "mobile/app/src/main/AndroidManifest.xml");
        let types = detect_project_types(tmp.path());
        assert!(types.contains(&"kotlin".to_string()));
        assert!(types.contains(&"android".to_string()));
        assert!(!types.contains(&"unknown".to_string()));
    }
}
