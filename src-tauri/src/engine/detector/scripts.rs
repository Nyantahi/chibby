//! Script, build-file, and CI-config detection at the repo root.

#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::engine::models::{
    Backend, DeploymentConfig, DeploymentMethod, Environment, EnvironmentsConfig, FileConflict,
    HealthCheck, Pipeline, PipelineValidation, PipelineWarning, Stage, WarningSeverity,
};
use std::collections::HashSet;
use std::path::Path;

/// Known script / task file patterns to scan for at the repo root.
const SCRIPT_PATTERNS: &[&str] = &[
    // Shell scripts
    "deploy.sh",
    "build.sh",
    "test.sh",
    // Make
    "Makefile",
    "GNUmakefile",
    "makefile",
    // Task runners
    "justfile",
    "Taskfile.yml",
    "Taskfile.yaml",
    "Rakefile",
    "Gruntfile.js",
    "gulpfile.js",
    // Containers
    "Dockerfile",
    "docker-compose.yml",
    "docker-compose.yaml",
    "compose.yml",
    "compose.yaml",
    "skaffold.yaml",
    "Vagrantfile",
    // Node / JS / TS
    "package.json",
    "turbo.json",
    "nx.json",
    "deno.json",
    "deno.jsonc",
    // Rust
    "Cargo.toml",
    // Go
    "go.mod",
    // Python
    "pyproject.toml",
    "setup.py",
    "setup.cfg",
    "requirements.txt",
    "Pipfile",
    "tox.ini",
    // Python test configs
    "pytest.ini",
    "conftest.py",
    ".coveragerc",
    // Ruby
    "Gemfile",
    // Java / Kotlin
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "gradlew",
    // Swift / iOS
    "Package.swift",
    // .NET / C#
    "global.json",
    // PHP
    "composer.json",
    // C / C++
    "CMakeLists.txt",
    "meson.build",
    // Infrastructure
    "netlify.toml",
    "vercel.json",
    // Process managers
    "Procfile",
    // CI platform configs (root-level)
    ".gitlab-ci.yml",
    "Jenkinsfile",
    ".travis.yml",
    ".drone.yml",
    "azure-pipelines.yml",
    "bitbucket-pipelines.yml",
    // Quality / hooks
    ".pre-commit-config.yaml",
    // Test configs
    "vitest.config.ts",
    "vitest.config.js",
    "vitest.config.mts",
    "jest.config.js",
    "jest.config.ts",
    "jest.config.mjs",
    ".mocharc.yml",
    ".mocharc.yaml",
    ".mocharc.json",
    // TypeScript
    "tsconfig.json",
    // Bundlers
    "vite.config.ts",
    "vite.config.js",
    "webpack.config.js",
    "webpack.config.ts",
    // Tauri
    "src-tauri/tauri.conf.json",
    "src-tauri/Cargo.toml",
    // Linters / formatters
    ".eslintrc.json",
    ".eslintrc.js",
    ".eslintrc.yml",
    "eslint.config.js",
    "eslint.config.mjs",
    ".prettierrc",
    ".prettierrc.json",
    "biome.json",
    // Chibby config (Phase 5)
    ".chibby/signing.toml",
    ".chibby/artifacts.toml",
    ".chibby/notify.toml",
    ".chibby/cleanup.toml",
];

/// Directory-based CI patterns to check for existence.
const CI_DIR_PATTERNS: &[(&str, &str, ScriptType)] = &[
    (
        ".github/workflows",
        ".github/workflows",
        ScriptType::GithubActions,
    ),
    (".circleci", ".circleci/config.yml", ScriptType::CircleCi),
    // Python test directories
    ("tests", "tests/", ScriptType::PythonTestDir),
    ("test", "test/", ScriptType::PythonTestDir),
    // Shell scripts directory
    ("scripts", "scripts/", ScriptType::ShellScript),
];

/// Information about a detected script or task source.
#[derive(Debug, Clone)]
pub struct DetectedScript {
    pub file_name: String,
    pub file_path: String,
    pub script_type: ScriptType,
}

/// The kind of task source detected.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptType {
    ShellScript,
    Makefile,
    Justfile,
    Dockerfile,
    DockerCompose,
    PackageJson,
    CargoToml,
    Taskfile,
    Procfile,
    EnvFile,
    // Go
    GoMod,
    // Python
    PythonProject,
    PythonRequirements,
    Tox,
    // Ruby
    Gemfile,
    Rakefile,
    // Java / Kotlin
    Maven,
    Gradle,
    // Swift / iOS
    Swift,
    // .NET
    DotNet,
    // PHP
    Composer,
    // C / C++
    CMake,
    Meson,
    // Monorepo tools
    Turborepo,
    Nx,
    // Deno
    Deno,
    // JS task runners
    Grunt,
    Gulp,
    // Container / infra
    Skaffold,
    Vagrantfile,
    Netlify,
    Vercel,
    // CI platforms (detected for awareness)
    GithubActions,
    GitlabCi,
    Jenkinsfile,
    TravisCi,
    DroneCi,
    CircleCi,
    AzurePipelines,
    BitbucketPipelines,
    // Quality
    PreCommit,
    // Test frameworks
    Vitest,
    Jest,
    Mocha,
    Pytest,
    PythonTestDir,
    // TypeScript
    TsConfig,
    // Bundlers
    ViteConfig,
    WebpackConfig,
    // Tauri
    TauriConfig,
    // Linters / formatters
    Eslint,
    Prettier,
    Biome,
    // Chibby config
    ChibbyConfig,
    Unknown,
}

/// Patterns to check in fullstack subdirectories.
const SUBDIR_PATTERNS: &[&str] = &[
    // Node / JS / TS
    "package.json",
    "tsconfig.json",
    "vite.config.ts",
    "vite.config.js",
    "next.config.js",
    "next.config.mjs",
    // Python
    "pyproject.toml",
    "requirements.txt",
    "setup.py",
    "Pipfile",
    "pytest.ini",
    "conftest.py",
    // Test configs
    "vitest.config.ts",
    "jest.config.js",
    "jest.config.ts",
];

/// Scan a repository directory for known script and task files.
pub fn detect_scripts(repo_path: &Path) -> Vec<DetectedScript> {
    let mut found = Vec::new();

    // Read directory entries once. Path::exists() is case-insensitive on APFS/NTFS,
    // so probing each pattern name separately would double-count (e.g. Makefile + makefile
    // resolve to the same inode). Match against actual on-disk names instead.
    let root_entries = list_dir_filenames(repo_path);

    // Check fixed-name patterns at repo root.
    for pattern in SCRIPT_PATTERNS {
        if root_entries.contains(*pattern) {
            let full = repo_path.join(pattern);
            let script_type = classify_file(pattern);
            found.push(DetectedScript {
                file_name: pattern.to_string(),
                file_path: full.to_string_lossy().to_string(),
                script_type,
            });
        }
    }

    // Check directory-based CI patterns.
    for &(dir_name, display_name, ref stype) in CI_DIR_PATTERNS {
        let dir_path = repo_path.join(dir_name);
        if dir_path.is_dir() {
            found.push(DetectedScript {
                file_name: display_name.to_string(),
                file_path: dir_path.to_string_lossy().to_string(),
                script_type: stype.clone(),
            });
        }
    }

    // Scan .github/workflows/ for individual workflow files
    let workflows_dir = repo_path.join(".github/workflows");
    if workflows_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&workflows_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".yml") || name.ends_with(".yaml") {
                    let display_name = format!(".github/workflows/{}", name);
                    if !found.iter().any(|s| s.file_name == display_name) {
                        found.push(DetectedScript {
                            file_name: display_name,
                            file_path: entry.path().to_string_lossy().to_string(),
                            script_type: ScriptType::GithubActions,
                        });
                    }
                }
            }
        }
    }

    // Check fullstack subdirectories (frontend/, backend/, api/, etc.)
    for subdir in FULLSTACK_SUBDIRS {
        let subdir_path = repo_path.join(subdir);
        if subdir_path.is_dir() {
            let subdir_entries = list_dir_filenames(&subdir_path);
            for pattern in SUBDIR_PATTERNS {
                if subdir_entries.contains(*pattern) {
                    let full = subdir_path.join(pattern);
                    let display_name = format!("{}/{}", subdir, pattern);
                    let script_type = classify_file(pattern);
                    // Avoid duplicates
                    if !found.iter().any(|s| s.file_name == display_name) {
                        found.push(DetectedScript {
                            file_name: display_name,
                            file_path: full.to_string_lossy().to_string(),
                            script_type,
                        });
                    }
                }
            }

            // Check for tests/ directory inside subdirectory
            let tests_path = subdir_path.join("tests");
            if tests_path.is_dir() {
                let display_name = format!("{}/tests/", subdir);
                if !found.iter().any(|s| s.file_name == display_name) {
                    found.push(DetectedScript {
                        file_name: display_name,
                        file_path: tests_path.to_string_lossy().to_string(),
                        script_type: ScriptType::PythonTestDir,
                    });
                }
            }
        }
    }

    // Scan repo root for *.sh, .env*, .sln/*.csproj, and docker-compose variants.
    if let Ok(entries) = std::fs::read_dir(repo_path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".sh") && !found.iter().any(|s| s.file_name == name) {
                found.push(DetectedScript {
                    file_name: name,
                    file_path: entry.path().to_string_lossy().to_string(),
                    script_type: ScriptType::ShellScript,
                });
            } else if is_env_file(&name) {
                found.push(DetectedScript {
                    file_name: name,
                    file_path: entry.path().to_string_lossy().to_string(),
                    script_type: ScriptType::EnvFile,
                });
            } else if name.ends_with(".sln") || name.ends_with(".csproj") {
                found.push(DetectedScript {
                    file_name: name,
                    file_path: entry.path().to_string_lossy().to_string(),
                    script_type: ScriptType::DotNet,
                });
            } else if is_xcode_bundle(&name)
                && !found.iter().any(|s| s.script_type == ScriptType::Swift)
            {
                found.push(DetectedScript {
                    file_name: name,
                    file_path: entry.path().to_string_lossy().to_string(),
                    script_type: ScriptType::Swift,
                });
            } else if is_docker_compose_file(&name) && !found.iter().any(|s| s.file_name == name) {
                // Detect docker-compose variants (docker-compose.prod.yml, etc.)
                found.push(DetectedScript {
                    file_name: name,
                    file_path: entry.path().to_string_lossy().to_string(),
                    script_type: ScriptType::DockerCompose,
                });
            }
        }
    }

    // Nested single-build-system fallbacks. In monorepo / mobile / `src/<App>/`
    // layouts the primary manifest sits below the root, which the root scans
    // above miss — leaving the draft pipeline on its generic `echo` fallback.
    // Shallow-walk for each so the real build system is detected. Only fires
    // when that build system wasn't already found at the root or in a fullstack
    // subdir. Swift and Gradle search deeper (bundles / `app/src/main`).
    push_nested_fallback(
        &mut found,
        repo_path,
        ScriptType::Swift,
        NESTED_SCAN_DEPTH,
        is_xcode_bundle,
    );
    push_nested_fallback(
        &mut found,
        repo_path,
        ScriptType::Swift,
        NESTED_SCAN_DEPTH,
        |n| n == "Package.swift",
    );
    push_nested_fallback(
        &mut found,
        repo_path,
        ScriptType::Gradle,
        DEEP_NESTED_SCAN_DEPTH,
        |n| n == "build.gradle" || n == "build.gradle.kts",
    );
    push_nested_fallback(
        &mut found,
        repo_path,
        ScriptType::GoMod,
        NESTED_SCAN_DEPTH,
        |n| n == "go.mod",
    );
    push_nested_fallback(
        &mut found,
        repo_path,
        ScriptType::Maven,
        NESTED_SCAN_DEPTH,
        |n| n == "pom.xml",
    );
    push_nested_fallback(
        &mut found,
        repo_path,
        ScriptType::Gemfile,
        NESTED_SCAN_DEPTH,
        |n| n == "Gemfile",
    );
    push_nested_fallback(
        &mut found,
        repo_path,
        ScriptType::Composer,
        NESTED_SCAN_DEPTH,
        |n| n == "composer.json",
    );
    push_nested_fallback(
        &mut found,
        repo_path,
        ScriptType::CMake,
        NESTED_SCAN_DEPTH,
        |n| n == "CMakeLists.txt",
    );
    push_nested_fallback(
        &mut found,
        repo_path,
        ScriptType::Meson,
        NESTED_SCAN_DEPTH,
        |n| n == "meson.build",
    );
    push_nested_fallback(
        &mut found,
        repo_path,
        ScriptType::DotNet,
        NESTED_SCAN_DEPTH,
        |n| n.ends_with(".csproj") || n.ends_with(".sln"),
    );

    // Scan scripts/ directory for shell scripts
    let scripts_dir = repo_path.join("scripts");
    if scripts_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&scripts_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".sh") {
                    let display_name = format!("scripts/{}", name);
                    if !found.iter().any(|s| s.file_name == display_name) {
                        found.push(DetectedScript {
                            file_name: display_name,
                            file_path: entry.path().to_string_lossy().to_string(),
                            script_type: ScriptType::ShellScript,
                        });
                    }
                }
            }
        }
    }

    // Scan for Python test files (test_*.py, *_test.py) in repo root and tests/ directories
    let python_test_dirs = [
        repo_path.to_path_buf(),
        repo_path.join("tests"),
        repo_path.join("test"),
    ];
    for test_dir in &python_test_dirs {
        if test_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(test_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if is_python_test_file(&name) {
                        let display_name = if test_dir == repo_path {
                            name.clone()
                        } else {
                            format!(
                                "{}/{}",
                                test_dir.file_name().unwrap_or_default().to_string_lossy(),
                                name
                            )
                        };
                        // Only add if not already found
                        if !found.iter().any(|s| s.file_name == display_name) {
                            found.push(DetectedScript {
                                file_name: display_name,
                                file_path: entry.path().to_string_lossy().to_string(),
                                script_type: ScriptType::Pytest,
                            });
                        }
                    }
                }
            }
        }
    }

    found
}

/// Classify a file by name.
fn classify_file(name: &str) -> ScriptType {
    match name {
        // Make
        "Makefile" | "GNUmakefile" | "makefile" => ScriptType::Makefile,
        // Task runners
        "justfile" => ScriptType::Justfile,
        "Taskfile.yml" | "Taskfile.yaml" => ScriptType::Taskfile,
        "Rakefile" => ScriptType::Rakefile,
        "Gruntfile.js" => ScriptType::Grunt,
        "gulpfile.js" => ScriptType::Gulp,
        // Containers
        "Dockerfile" => ScriptType::Dockerfile,
        "docker-compose.yml" | "docker-compose.yaml" | "compose.yml" | "compose.yaml" => {
            ScriptType::DockerCompose
        }
        "skaffold.yaml" => ScriptType::Skaffold,
        "Vagrantfile" => ScriptType::Vagrantfile,
        // Node / JS / TS
        "package.json" => ScriptType::PackageJson,
        "turbo.json" => ScriptType::Turborepo,
        "nx.json" => ScriptType::Nx,
        "deno.json" | "deno.jsonc" => ScriptType::Deno,
        // Rust
        "Cargo.toml" => ScriptType::CargoToml,
        // Go
        "go.mod" => ScriptType::GoMod,
        // Python
        "pyproject.toml" | "setup.py" | "setup.cfg" | "Pipfile" => ScriptType::PythonProject,
        "requirements.txt" => ScriptType::PythonRequirements,
        "tox.ini" => ScriptType::Tox,
        // Ruby
        "Gemfile" => ScriptType::Gemfile,
        // Java / Kotlin
        "pom.xml" => ScriptType::Maven,
        "build.gradle" | "build.gradle.kts" | "gradlew" => ScriptType::Gradle,
        // Swift / iOS
        "Package.swift" => ScriptType::Swift,
        // .NET
        "global.json" => ScriptType::DotNet,
        // PHP
        "composer.json" => ScriptType::Composer,
        // C / C++
        "CMakeLists.txt" => ScriptType::CMake,
        "meson.build" => ScriptType::Meson,
        // Infra / deploy
        "netlify.toml" => ScriptType::Netlify,
        "vercel.json" => ScriptType::Vercel,
        "Procfile" => ScriptType::Procfile,
        // CI platforms
        ".gitlab-ci.yml" => ScriptType::GitlabCi,
        "Jenkinsfile" => ScriptType::Jenkinsfile,
        ".travis.yml" => ScriptType::TravisCi,
        ".drone.yml" => ScriptType::DroneCi,
        "azure-pipelines.yml" => ScriptType::AzurePipelines,
        "bitbucket-pipelines.yml" => ScriptType::BitbucketPipelines,
        // Quality
        ".pre-commit-config.yaml" => ScriptType::PreCommit,
        // Test frameworks
        "vitest.config.ts" | "vitest.config.js" | "vitest.config.mts" => ScriptType::Vitest,
        "jest.config.js" | "jest.config.ts" | "jest.config.mjs" => ScriptType::Jest,
        ".mocharc.yml" | ".mocharc.yaml" | ".mocharc.json" => ScriptType::Mocha,
        "pytest.ini" | "conftest.py" | ".coveragerc" => ScriptType::Pytest,
        // TypeScript
        "tsconfig.json" => ScriptType::TsConfig,
        // Bundlers
        "vite.config.ts" | "vite.config.js" => ScriptType::ViteConfig,
        "webpack.config.js" | "webpack.config.ts" => ScriptType::WebpackConfig,
        // Tauri (nested paths)
        "src-tauri/tauri.conf.json" => ScriptType::TauriConfig,
        "src-tauri/Cargo.toml" => ScriptType::CargoToml,
        // Linters / formatters
        ".eslintrc.json" | ".eslintrc.js" | ".eslintrc.yml" | "eslint.config.js"
        | "eslint.config.mjs" => ScriptType::Eslint,
        ".prettierrc" | ".prettierrc.json" => ScriptType::Prettier,
        "biome.json" => ScriptType::Biome,
        // Chibby config
        ".chibby/signing.toml"
        | ".chibby/artifacts.toml"
        | ".chibby/notify.toml"
        | ".chibby/cleanup.toml" => ScriptType::ChibbyConfig,
        // Shell / unknown
        _ if name.ends_with(".sh") => ScriptType::ShellScript,
        _ => ScriptType::Unknown,
    }
}

/// Whether the repo is an Android project: an `app/` module with a Gradle build
/// or, anywhere within a shallow walk, an `AndroidManifest.xml` (the definitive
/// Android signal). The nested walk covers layouts where the module sits below
/// the root (e.g. `mobile/app/src/main/AndroidManifest.xml`). Shared by draft
/// generation and recommendations so both classify a repo the same way.
pub fn is_android_project(repo_path: &Path) -> bool {
    repo_path.join("app/build.gradle").exists()
        || repo_path.join("app/build.gradle.kts").exists()
        || exists_within(repo_path, DEEP_NESTED_SCAN_DEPTH, &|n: &str| {
            n == "AndroidManifest.xml"
        })
}

/// Check if a file is an environment variable file (.env, .env.local, etc.).
fn is_env_file(name: &str) -> bool {
    name == ".env"
        || (name.starts_with(".env.") && !name.ends_with(".example") && !name.ends_with(".sample"))
}

/// Check if a file is a Python test file (test_*.py or *_test.py).
fn is_python_test_file(name: &str) -> bool {
    if !name.ends_with(".py") {
        return false;
    }
    let base = &name[..name.len() - 3]; // Remove .py
    base.starts_with("test_") || base.ends_with("_test")
}

/// Check if a file is a docker-compose variant (docker-compose.*.yml, compose.*.yml).
pub(crate) fn is_docker_compose_file(name: &str) -> bool {
    // Already handled in SCRIPT_PATTERNS: docker-compose.yml, docker-compose.yaml, compose.yml, compose.yaml
    // This catches variants like docker-compose.prod.yml, docker-compose.dev.yml, etc.
    let name_lower = name.to_lowercase();
    (name_lower.starts_with("docker-compose.") || name_lower.starts_with("compose."))
        && (name_lower.ends_with(".yml") || name_lower.ends_with(".yaml"))
}

/// Helper to create a local stage.
pub(crate) fn local_stage(name: &str, commands: Vec<&str>) -> Stage {
    Stage {
        name: name.to_string(),
        commands: commands.into_iter().map(|c| c.to_string()).collect(),
        ..Default::default()
    }
}

/// Read package.json scripts from a repo.
pub(crate) fn read_package_scripts(repo_path: &Path) -> std::collections::HashMap<String, String> {
    let pkg_path = repo_path.join("package.json");
    if !pkg_path.exists() {
        return std::collections::HashMap::new();
    }
    let content = match std::fs::read_to_string(&pkg_path) {
        Ok(c) => c,
        Err(_) => return std::collections::HashMap::new(),
    };
    let value: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return std::collections::HashMap::new(),
    };
    let mut scripts = std::collections::HashMap::new();
    if let Some(obj) = value.get("scripts").and_then(|s| s.as_object()) {
        for (k, v) in obj {
            if let Some(cmd) = v.as_str() {
                scripts.insert(k.clone(), cmd.to_string());
            }
        }
    }
    scripts
}

/// How many directory levels below the repo root to shallow-walk when probing
/// for nested project files (e.g. a nested `.xcodeproj`). Depth 0 = root only.
pub(crate) const NESTED_SCAN_DEPTH: usize = 3;

/// Deeper scan depth for build files that nest further than usual: an
/// `AndroidManifest.xml` lives at `<module>/src/main/` (two levels below the
/// module root) and Gradle modules can sit several levels down. Covers
/// `app/src/main/` and `mobile/app/src/main/`.
pub(crate) const DEEP_NESTED_SCAN_DEPTH: usize = 5;

/// Directories never worth descending into when probing for nested project
/// files (vendored deps, build output, VCS internals, bundle dirs).
const NESTED_SKIP_DIRS: &[&str] = &[
    "node_modules",
    "Pods",
    "Carthage",
    "DerivedData",
    "build",
    ".build",
    "target",
    "dist",
    "out",
    "vendor",
    ".gradle",
    "__pycache__",
];

/// Find the first entry at or below `dir` (within `max_depth` levels; depth 0 =
/// `dir` itself) whose file name satisfies `pred`. Skips vendor/build/bundle
/// dirs and hidden dirs so a shallow walk stays cheap and avoids false hits
/// inside `.xcodeproj`/`.xcworkspace` bundles.
pub(crate) fn find_within<F>(dir: &Path, max_depth: usize, pred: &F) -> Option<std::path::PathBuf>
where
    F: Fn(&str) -> bool,
{
    let entries = std::fs::read_dir(dir).ok()?;
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if pred(&name) {
            return Some(entry.path());
        }
        if max_depth > 0 {
            let path = entry.path();
            if path.is_dir() && !name.starts_with('.') && !is_skip_dir(&name) {
                subdirs.push(path);
            }
        }
    }
    subdirs
        .iter()
        .find_map(|sub| find_within(sub, max_depth - 1, pred))
}

/// Whether any entry at or below `dir` (within `max_depth`) matches `pred`.
pub(crate) fn exists_within<F>(dir: &Path, max_depth: usize, pred: &F) -> bool
where
    F: Fn(&str) -> bool,
{
    find_within(dir, max_depth, pred).is_some()
}

/// If no script of `stype` was detected yet, shallow-walk for the first file
/// matching `pred` and record it (relative to the repo root). Keeps nested
/// monorepo / mobile / `src/<App>/` layouts from falling through to the generic
/// `echo` fallback pipeline.
fn push_nested_fallback<F>(
    found: &mut Vec<DetectedScript>,
    repo_path: &Path,
    stype: ScriptType,
    max_depth: usize,
    pred: F,
) where
    F: Fn(&str) -> bool,
{
    if found.iter().any(|s| s.script_type == stype) {
        return;
    }
    if let Some(path) = find_within(repo_path, max_depth, &pred) {
        found.push(DetectedScript {
            file_name: rel_display_name(repo_path, &path),
            file_path: path.to_string_lossy().to_string(),
            script_type: stype,
        });
    }
}

/// Dirs to never descend into (vendor/build output and Xcode bundle dirs).
fn is_skip_dir(name: &str) -> bool {
    NESTED_SKIP_DIRS.contains(&name)
        || name.ends_with(".xcodeproj")
        || name.ends_with(".xcworkspace")
}

/// Whether a file name is an Xcode project or workspace bundle.
pub(crate) fn is_xcode_bundle(name: &str) -> bool {
    name.ends_with(".xcodeproj") || name.ends_with(".xcworkspace")
}

/// Display name for a detected file: its path relative to the repo root (POSIX
/// separators) so nested matches read as `KibokoKids/KibokoKids.xcodeproj`.
/// Falls back to the full path, then the bare file name.
fn rel_display_name(repo_path: &Path, path: &Path) -> String {
    path.strip_prefix(repo_path)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Read the immediate filenames in a directory (case-preserving, no recursion).
pub(crate) fn list_dir_filenames(dir: &Path) -> HashSet<String> {
    let mut names = HashSet::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            names.insert(entry.file_name().to_string_lossy().to_string());
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_classify_file_makefile() {
        assert_eq!(classify_file("Makefile"), ScriptType::Makefile);
        assert_eq!(classify_file("GNUmakefile"), ScriptType::Makefile);
    }

    #[test]
    fn test_classify_file_package_json() {
        assert_eq!(classify_file("package.json"), ScriptType::PackageJson);
    }

    #[test]
    fn test_classify_file_cargo() {
        assert_eq!(classify_file("Cargo.toml"), ScriptType::CargoToml);
    }

    #[test]
    fn test_classify_file_docker() {
        assert_eq!(classify_file("Dockerfile"), ScriptType::Dockerfile);
        assert_eq!(
            classify_file("docker-compose.yml"),
            ScriptType::DockerCompose
        );
    }

    #[test]
    fn test_classify_file_python() {
        assert_eq!(classify_file("pyproject.toml"), ScriptType::PythonProject);
        assert_eq!(
            classify_file("requirements.txt"),
            ScriptType::PythonRequirements
        );
    }

    #[test]
    fn test_classify_file_go() {
        assert_eq!(classify_file("go.mod"), ScriptType::GoMod);
    }

    #[test]
    fn test_classify_file_unknown() {
        assert_eq!(classify_file("random.txt"), ScriptType::Unknown);
        assert_eq!(classify_file("BUILD"), ScriptType::Unknown);
    }

    #[test]
    fn test_detect_scripts_empty_dir() {
        let temp = TempDir::new().unwrap();
        let scripts = detect_scripts(temp.path());
        assert!(scripts.is_empty());
    }

    #[test]
    fn test_detect_scripts_package_json() {
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("package.json"),
            r#"{ "scripts": { "build": "vite build", "test": "vitest" } }"#,
        )
        .unwrap();

        let scripts = detect_scripts(temp.path());
        assert!(!scripts.is_empty());
        assert!(scripts
            .iter()
            .any(|s| s.script_type == ScriptType::PackageJson));
    }

    #[test]
    fn test_detect_scripts_makefile() {
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("Makefile"),
            "build:\n\techo building\ntest:\n\techo testing",
        )
        .unwrap();

        let scripts = detect_scripts(temp.path());
        assert!(!scripts.is_empty());
        assert!(scripts
            .iter()
            .any(|s| s.script_type == ScriptType::Makefile));
    }

    #[test]
    fn test_detect_scripts_no_case_duplicate_on_apfs() {
        // Regression: on case-insensitive filesystems (default macOS APFS),
        // Path::exists() returned true for both "Makefile" and "makefile"
        // when only one file existed, producing phantom duplicates.
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("Makefile"), "build:\n\techo building").unwrap();

        let scripts = detect_scripts(temp.path());
        let makefile_hits: Vec<_> = scripts
            .iter()
            .filter(|s| s.script_type == ScriptType::Makefile)
            .collect();
        assert_eq!(
            makefile_hits.len(),
            1,
            "expected one Makefile entry, got {:?}",
            makefile_hits
        );
        assert_eq!(makefile_hits[0].file_name, "Makefile");
    }

    #[test]
    fn test_detect_scripts_cargo() {
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"",
        )
        .unwrap();

        let scripts = detect_scripts(temp.path());
        assert!(!scripts.is_empty());
        assert!(scripts
            .iter()
            .any(|s| s.script_type == ScriptType::CargoToml));
    }

    #[test]
    fn test_detect_scripts_nested_xcodeproj() {
        // Standard iOS layout: project bundle nested one level down.
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join("KibokoKids/KibokoKids.xcodeproj")).unwrap();

        let scripts = detect_scripts(temp.path());
        let swift = scripts
            .iter()
            .find(|s| s.script_type == ScriptType::Swift)
            .expect("nested .xcodeproj should be detected as Swift");
        assert_eq!(swift.file_name, "KibokoKids/KibokoKids.xcodeproj");
    }

    #[test]
    fn test_detect_scripts_skips_vendor_dirs() {
        // An .xcodeproj only inside Pods/ must not count as the app project.
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join("Pods/Pods.xcodeproj")).unwrap();

        let scripts = detect_scripts(temp.path());
        assert!(!scripts.iter().any(|s| s.script_type == ScriptType::Swift));
    }

    #[test]
    fn test_detect_scripts_root_xcodeproj_still_works() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join("MyApp.xcodeproj")).unwrap();

        let scripts = detect_scripts(temp.path());
        assert!(scripts.iter().any(|s| s.script_type == ScriptType::Swift));
    }

    #[test]
    fn test_detect_scripts_nested_gradle() {
        // Gradle module nested below the root (monorepo-style layout).
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join("mobile/app")).unwrap();
        std::fs::write(temp.path().join("mobile/app/build.gradle.kts"), "").unwrap();

        let scripts = detect_scripts(temp.path());
        let gradle = scripts
            .iter()
            .find(|s| s.script_type == ScriptType::Gradle)
            .expect("nested build.gradle.kts should be detected as Gradle");
        assert_eq!(gradle.file_name, "mobile/app/build.gradle.kts");
    }

    #[test]
    fn test_detect_scripts_nested_go_ruby_php() {
        for (rel, stype) in [
            ("services/api/go.mod", ScriptType::GoMod),
            ("backend/Gemfile", ScriptType::Gemfile),
            ("web/composer.json", ScriptType::Composer),
            ("src/app/CMakeLists.txt", ScriptType::CMake),
        ] {
            let temp = TempDir::new().unwrap();
            let p = temp.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, "").unwrap();
            let scripts = detect_scripts(temp.path());
            assert!(
                scripts.iter().any(|s| s.script_type == stype),
                "{rel} should be detected as {stype:?}"
            );
        }
    }

    #[test]
    fn test_is_android_project_nested_manifest() {
        // AndroidManifest below the root with no root-level `app/` module.
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join("mobile/app/src/main")).unwrap();
        std::fs::write(
            temp.path().join("mobile/app/src/main/AndroidManifest.xml"),
            "",
        )
        .unwrap();

        assert!(is_android_project(temp.path()));
    }
}
