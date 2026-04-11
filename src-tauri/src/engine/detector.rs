use crate::engine::models::{Pipeline, Stage, Backend, PipelineValidation, PipelineWarning, WarningSeverity, FileConflict, DeploymentMethod, DeploymentConfig, HealthCheck, Environment, EnvironmentsConfig};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Detect common scripts, build files, and CI/CD configs in a repository
/// and generate a draft pipeline that the user can review and edit.

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
    (".github/workflows", ".github/workflows", ScriptType::GithubActions),
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

/// Common subdirectory names for fullstack projects (frontend/backend).
const FULLSTACK_SUBDIRS: &[&str] = &[
    "frontend", "backend", "api", "web", "app", "client", "server", "src", "admin", "dashboard", "portal",
];

/// Subdirectories that indicate frontend (Node.js) projects.
const FRONTEND_SUBDIRS: &[&str] = &["frontend", "client", "web", "app", "admin", "dashboard", "portal"];

/// Subdirectories that indicate backend (Python/Node.js) projects.
const BACKEND_SUBDIRS: &[&str] = &["backend", "api", "server"];

/// Information about a detected project folder with its capabilities.
#[derive(Debug, Clone)]
pub struct ProjectFolder {
    /// Subdirectory name (e.g., "frontend", "backend", "admin").
    pub name: String,
    /// Has package.json (Node.js project).
    pub has_node: bool,
    /// Has requirements.txt or pyproject.toml (Python project).
    pub has_python: bool,
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

    for subdir in FULLSTACK_SUBDIRS {
        let subdir_path = repo_path.join(subdir);
        if !subdir_path.is_dir() {
            continue;
        }

        let has_package_json = subdir_path.join("package.json").exists();
        let has_requirements = subdir_path.join("requirements.txt").exists();
        let has_pyproject = subdir_path.join("pyproject.toml").exists();
        let has_python = has_requirements || has_pyproject;

        // Skip if not a recognizable project
        if !has_package_json && !has_python {
            continue;
        }

        // Check for tests
        let has_tests = subdir_path.join("tests").is_dir()
            || subdir_path.join("test").is_dir()
            || subdir_path.join("__tests__").is_dir()
            || subdir_path.join("vitest.config.ts").exists()
            || subdir_path.join("jest.config.js").exists()
            || subdir_path.join("pytest.ini").exists()
            || subdir_path.join("conftest.py").exists();

        // Read npm scripts if applicable
        let npm_scripts = if has_package_json {
            read_package_scripts(&subdir_path).keys().cloned().collect()
        } else {
            std::collections::HashSet::new()
        };

        // Determine if frontend or backend based on folder name
        let is_frontend = FRONTEND_SUBDIRS.contains(&subdir.to_lowercase().as_str()) || has_package_json && !has_python;
        let is_backend = BACKEND_SUBDIRS.contains(&subdir.to_lowercase().as_str()) || has_python;

        folders.push(ProjectFolder {
            name: subdir.to_string(),
            has_node: has_package_json,
            has_python,
            has_tests,
            npm_scripts,
            is_frontend,
            is_backend,
        });
    }

    folders
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
fn has_any_docker_compose(repo_path: &Path) -> bool {
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

    // Check fixed-name patterns at repo root.
    for pattern in SCRIPT_PATTERNS {
        let full = repo_path.join(pattern);
        if full.exists() {
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
            for pattern in SUBDIR_PATTERNS {
                let full = subdir_path.join(pattern);
                if full.exists() {
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
    let python_test_dirs = [repo_path.to_path_buf(), repo_path.join("tests"), repo_path.join("test")];
    for test_dir in &python_test_dirs {
        if test_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(test_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if is_python_test_file(&name) {
                        let display_name = if test_dir == repo_path {
                            name.clone()
                        } else {
                            format!("{}/{}", test_dir.file_name().unwrap_or_default().to_string_lossy(), name)
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
        "docker-compose.yml" | "docker-compose.yaml"
        | "compose.yml" | "compose.yaml" => ScriptType::DockerCompose,
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
        ".eslintrc.json" | ".eslintrc.js" | ".eslintrc.yml"
        | "eslint.config.js" | "eslint.config.mjs" => ScriptType::Eslint,
        ".prettierrc" | ".prettierrc.json" => ScriptType::Prettier,
        "biome.json" => ScriptType::Biome,
        // Chibby config
        ".chibby/signing.toml" | ".chibby/artifacts.toml"
        | ".chibby/notify.toml" | ".chibby/cleanup.toml" => ScriptType::ChibbyConfig,
        // Shell / unknown
        _ if name.ends_with(".sh") => ScriptType::ShellScript,
        _ => ScriptType::Unknown,
    }
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
fn is_docker_compose_file(name: &str) -> bool {
    // Already handled in SCRIPT_PATTERNS: docker-compose.yml, docker-compose.yaml, compose.yml, compose.yaml
    // This catches variants like docker-compose.prod.yml, docker-compose.dev.yml, etc.
    let name_lower = name.to_lowercase();
    (name_lower.starts_with("docker-compose.") || name_lower.starts_with("compose."))
        && (name_lower.ends_with(".yml") || name_lower.ends_with(".yaml"))
}

/// Helper to create a local stage.
fn local_stage(name: &str, commands: Vec<&str>) -> Stage {
    Stage {
        name: name.to_string(),
        commands: commands.into_iter().map(|c| c.to_string()).collect(),
        backend: Backend::Local,
        working_dir: None,
        fail_fast: true,
        health_check: None,
    }
}

/// Read package.json scripts from a repo.
fn read_package_scripts(repo_path: &Path) -> std::collections::HashMap<String, String> {
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

/// Generate a draft pipeline from detected scripts.
///
/// Reads package.json scripts when available to produce more accurate
/// stages instead of generic "npm run build" / "npm test" defaults.
/// Includes stages from ALL detected build systems — not mutually
/// exclusive. Users can remove stages they don't need via the editor.
pub fn generate_draft_pipeline(
    repo_name: &str,
    scripts: &[DetectedScript],
    repo_path: &Path,
) -> Pipeline {
    let mut stages: Vec<Stage> = Vec::new();

    let has = |st: ScriptType| scripts.iter().any(|s| s.script_type == st);
    let has_file = |name: &str| scripts.iter().any(|s| s.file_name == name);
    let is_tauri = has(ScriptType::TauriConfig);

    // Check for ROOT package.json specifically (not in subdirectories)
    let has_root_package_json = has_file("package.json");

    // Read package.json scripts for smarter generation (only if root exists).
    let pkg_scripts = if has_root_package_json {
        read_package_scripts(repo_path)
    } else {
        std::collections::HashMap::new()
    };
    let has_script = |name: &str| pkg_scripts.contains_key(name);

    // ── Install dependencies ─────────────────────────────────────
    // Only add root-level npm stages if root package.json exists
    if has_root_package_json {
        stages.push(local_stage("install", vec!["npm install"]));
    }

    // ── Type checking ────────────────────────────────────────────
    if has_script("type-check") {
        stages.push(local_stage("type-check", vec!["npm run type-check"]));
    } else if has(ScriptType::TsConfig) && has_root_package_json {
        stages.push(local_stage("type-check", vec!["npx tsc --noEmit"]));
    }

    // ── Linting ──────────────────────────────────────────────────
    if has_script("lint") {
        stages.push(local_stage("lint", vec!["npm run lint"]));
    } else if has(ScriptType::Eslint) && has_root_package_json {
        stages.push(local_stage("lint", vec!["npx eslint ."]));
    } else if has(ScriptType::Biome) {
        stages.push(local_stage("lint", vec!["npx biome check ."]));
    }

    // ── Format check ─────────────────────────────────────────────
    if has_script("format:check") {
        stages.push(local_stage("format-check", vec!["npm run format:check"]));
    }

    // ── Rust / Cargo (root or nested) ────────────────────────────
    if has(ScriptType::CargoToml) {
        if is_tauri {
            // Tauri project — Cargo.toml is in src-tauri/
            stages.push(local_stage("cargo-test", vec!["cd src-tauri && cargo test"]));
        } else {
            stages.push(local_stage("cargo-build", vec!["cargo build --release"]));
            stages.push(local_stage("cargo-test", vec!["cargo test"]));
        }
    }

    // ── Frontend tests ───────────────────────────────────────────
    if has_script("test:run") {
        stages.push(local_stage("test", vec!["npm run test:run"]));
    } else if has_script("test") {
        // Use "npm run test:run" or "npx vitest run" for CI (non-interactive)
        if has(ScriptType::Vitest) {
            stages.push(local_stage("test", vec!["npx vitest run"]));
        } else if has(ScriptType::Jest) {
            stages.push(local_stage("test", vec!["npx jest --ci"]));
        } else {
            stages.push(local_stage("test", vec!["npm test"]));
        }
    }

    // ── Full test suite (frontend + backend) ─────────────────────
    if has_script("test:all") {
        stages.push(local_stage("test-all", vec!["npm run test:all"]));
    }

    // ── Build ────────────────────────────────────────────────────
    if is_tauri {
        // Tauri project — use tauri:build if available, otherwise tauri build
        if has_script("tauri:build") {
            stages.push(local_stage("tauri-build", vec!["npm run tauri:build"]));
        } else {
            stages.push(local_stage("tauri-build", vec!["npx tauri build"]));
        }
    } else if has_script("build") {
        stages.push(local_stage("build", vec!["npm run build"]));
    }

    // ── Non-Node languages (only if no package.json) ─────────────

    // ── Turborepo (monorepo) ──────────────────────────────────────
    if has(ScriptType::Turborepo) {
        stages.push(local_stage("turbo-build", vec!["npx turbo run build"]));
        stages.push(local_stage("turbo-test", vec!["npx turbo run test"]));
    }

    // ── Nx (monorepo) ─────────────────────────────────────────────
    if has(ScriptType::Nx) {
        stages.push(local_stage("nx-build", vec!["npx nx run-many --target=build"]));
        stages.push(local_stage("nx-test", vec!["npx nx run-many --target=test"]));
    }

    // ── Deno ──────────────────────────────────────────────────────
    if has(ScriptType::Deno) {
        stages.push(local_stage("deno-test", vec!["deno test"]));
    }

    // ── Go ────────────────────────────────────────────────────────
    if has(ScriptType::GoMod) {
        stages.push(local_stage("go-build", vec!["go build ./..."]));
        stages.push(local_stage("go-test", vec!["go test ./..."]));
    }

    // ── Python ────────────────────────────────────────────────────
    // Check for ROOT Python files specifically (not in subdirectories)
    let has_root_requirements = has_file("requirements.txt");
    let has_root_pyproject = has_file("pyproject.toml");
    let has_root_python = has_root_requirements || has_root_pyproject || has_file("setup.py");

    if has_root_python {
        if has(ScriptType::Tox) {
            stages.push(local_stage("tox", vec!["tox"]));
        } else {
            let install_cmd = if has_root_requirements {
                "pip install -r requirements.txt"
            } else {
                "pip install -e ."
            };
            stages.push(local_stage("pip-install", vec![install_cmd]));

            // Only add pytest stage if we detected test files, test directories, or pytest config
            if has(ScriptType::Pytest) || has(ScriptType::PythonTestDir) {
                stages.push(local_stage("pytest", vec!["pytest"]));
            }
        }
    }

    // ── Python linting (if Python project without Docker-only setup) ─────
    if has_root_python && !has_root_package_json {
        // Check for common Python linter configs
        let has_ruff = repo_path.join("ruff.toml").exists() || repo_path.join("pyproject.toml").exists();
        let has_flake8 = repo_path.join(".flake8").exists() || repo_path.join("setup.cfg").exists();

        if has_ruff {
            stages.push(local_stage("python-lint", vec!["ruff check ."]));
        } else if has_flake8 {
            stages.push(local_stage("python-lint", vec!["flake8 ."]));
        }
    }

    // ── Fullstack: Multiple subdirectories ──────────────────────────
    // Detect and generate stages for ALL project folders (frontend, backend, admin, etc.)
    let has_subdir_file = |subdir: &str, file: &str| {
        scripts.iter().any(|s| s.file_name == format!("{}/{}", subdir, file))
    };

    // Get all project folders with their capabilities
    let project_folders = detect_project_folders(repo_path);
    let is_fullstack = project_folders.len() >= 2;
    let has_root_pkg = has_root_package_json; // Use the root check, not subdirectory detection
    let has_root_python = has_file("requirements.txt") || has_file("pyproject.toml");

    // Process each detected project folder
    for folder in &project_folders {
        let subdir = &folder.name;

        // Generate Node.js stages for this folder
        if folder.has_node {
            // For fullstack projects, always generate per-folder stages
            // For single-folder projects, only if no root package.json
            if is_fullstack || !has_root_pkg {
                // Use npm install (not npm ci) since package-lock.json may not exist
                stages.push(local_stage(&format!("{}-install", subdir),
                    vec![&format!("cd {} && npm install", subdir)]));

                if folder.npm_scripts.contains("lint") {
                    stages.push(local_stage(&format!("{}-lint", subdir),
                        vec![&format!("cd {} && npm run lint", subdir)]));
                }

                // Check for tests
                let has_test_script = folder.npm_scripts.contains("test");
                let has_vitest = has_subdir_file(subdir, "vitest.config.ts")
                    || has_subdir_file(subdir, "vitest.config.js");
                let has_jest = has_subdir_file(subdir, "jest.config.js")
                    || has_subdir_file(subdir, "jest.config.ts");

                if has_test_script || has_vitest || has_jest || folder.has_tests {
                    // Use appropriate test command
                    let test_cmd = if has_vitest {
                        format!("cd {} && npx vitest run", subdir)
                    } else if has_jest {
                        format!("cd {} && npx jest --ci", subdir)
                    } else {
                        format!("cd {} && npm test", subdir)
                    };
                    stages.push(local_stage(&format!("{}-test", subdir), vec![&test_cmd]));
                }

                if folder.npm_scripts.contains("build") {
                    stages.push(local_stage(&format!("{}-build", subdir),
                        vec![&format!("cd {} && npm run build", subdir)]));
                }
            }
        }

        // Generate Python stages for this folder
        if folder.has_python {
            // For fullstack projects, always generate per-folder stages
            // For single-folder projects, only if no root Python setup
            if is_fullstack || !has_root_python {
                let install_cmd = if has_subdir_file(subdir, "requirements.txt") {
                    format!("cd {} && pip install -r requirements.txt", subdir)
                } else {
                    format!("cd {} && pip install -e .", subdir)
                };
                stages.push(local_stage(&format!("{}-install", subdir), vec![&install_cmd]));

                // Check for tests in subdirectory
                let has_pytest_config = has_subdir_file(subdir, "pytest.ini")
                    || has_subdir_file(subdir, "conftest.py");
                if folder.has_tests || has_pytest_config {
                    stages.push(local_stage(&format!("{}-test", subdir),
                        vec![&format!("cd {} && pytest", subdir)]));
                }
            }
        }
    }

    // ── Ruby ──────────────────────────────────────────────────────
    if has(ScriptType::Gemfile) {
        stages.push(local_stage("bundle-install", vec!["bundle install"]));
        if has(ScriptType::Rakefile) {
            stages.push(local_stage("rake-test", vec!["bundle exec rake test"]));
        } else {
            stages.push(local_stage("rspec", vec!["bundle exec rspec"]));
        }
    }

    // ── Java / Maven ──────────────────────────────────────────────
    if has(ScriptType::Maven) {
        stages.push(local_stage("mvn-build", vec!["mvn package -DskipTests"]));
        stages.push(local_stage("mvn-test", vec!["mvn test"]));
    }

    // ── Java / Gradle ─────────────────────────────────────────────
    if has(ScriptType::Gradle) {
        let gradle_cmd = if has_file("gradlew") { "./gradlew" } else { "gradle" };
        stages.push(local_stage("gradle-build", vec![&format!("{} build", gradle_cmd)]));
        stages.push(local_stage("gradle-test", vec![&format!("{} test", gradle_cmd)]));
    }

    // ── .NET ──────────────────────────────────────────────────────
    if has(ScriptType::DotNet) {
        stages.push(local_stage("dotnet-build", vec!["dotnet build"]));
        stages.push(local_stage("dotnet-test", vec!["dotnet test"]));
    }

    // ── PHP / Composer ────────────────────────────────────────────
    if has(ScriptType::Composer) {
        stages.push(local_stage("composer-install", vec!["composer install"]));
        stages.push(local_stage("phpunit", vec!["./vendor/bin/phpunit"]));
    }

    // ── C / C++ / CMake ───────────────────────────────────────────
    if has(ScriptType::CMake) {
        stages.push(local_stage("cmake-build", vec![
            "cmake -B build",
            "cmake --build build",
        ]));
        stages.push(local_stage("cmake-test", vec!["ctest --test-dir build"]));
    }

    // ── Meson ─────────────────────────────────────────────────────
    if has(ScriptType::Meson) {
        stages.push(local_stage("meson-build", vec![
            "meson setup build",
            "meson compile -C build",
        ]));
        stages.push(local_stage("meson-test", vec!["meson test -C build"]));
    }

    // ── Makefile (generic — after language-specific) ──────────────
    if has(ScriptType::Makefile) {
        stages.push(local_stage("make-build", vec!["make build"]));
        stages.push(local_stage("make-test", vec!["make test"]));
    }

    // ── Deploy stages ─────────────────────────────────────────────
    if has_file("deploy.sh") {
        stages.push(local_stage("deploy", vec!["./deploy.sh"]));
    }
    // For fullstack projects, deploy stages go in a separate deploy.toml
    // For single-component projects, include docker-deploy here
    if has(ScriptType::DockerCompose) && !is_fullstack {
        stages.push(Stage {
            name: "docker-deploy".to_string(),
            commands: vec![
                "docker compose build".to_string(),
                "docker compose up -d".to_string(),
            ],
            backend: Backend::Ssh,
            working_dir: None,
            fail_fast: true,
            health_check: None,
        });
    }

    // ── Fallback ──────────────────────────────────────────────────
    if stages.is_empty() {
        stages.push(local_stage("build", vec!["echo 'Add your build commands here'"]));
    }

    Pipeline {
        name: format!("{} Pipeline", repo_name),
        stages,
    }
}

/// Generate a deploy pipeline for fullstack Docker projects.
///
/// This creates a separate pipeline focused on deployment stages.
/// If GitHub Actions deploy workflows exist, it incorporates their steps.
/// Otherwise falls back to generic docker compose stages.
pub fn generate_deploy_pipeline(
    repo_name: &str,
    scripts: &[DetectedScript],
    repo_path: &Path,
) -> Option<Pipeline> {
    let has = |st: ScriptType| scripts.iter().any(|s| s.script_type == st);

    // Only generate deploy pipeline if Docker Compose or GitHub Actions is present
    let has_docker_compose = has(ScriptType::DockerCompose);
    let has_github_actions = has(ScriptType::GithubActions);

    if !has_docker_compose && !has_github_actions {
        return None;
    }

    let mut stages = Vec::new();

    // Parse GitHub Actions workflows and look for deploy-related ones
    let workflows = parse_github_workflows(repo_path);
    let deploy_workflows: Vec<_> = workflows
        .iter()
        .filter(|w| {
            let name_lower = w.name.to_lowercase();
            let file_lower = w.file_path.to_lowercase();
            name_lower.contains("deploy")
                || name_lower.contains("release")
                || file_lower.contains("deploy")
                || file_lower.contains("release")
        })
        .cloned()
        .collect();

    if !deploy_workflows.is_empty() {
        // Use stages from deploy workflows
        let workflow_stages = workflows_to_stages(&deploy_workflows);
        for stage in workflow_stages {
            // Determine if this is an SSH stage based on commands
            let is_ssh_stage = stage.commands.iter().any(|cmd| {
                cmd.contains("ssh ") || cmd.contains("rsync") || cmd.contains("scp ")
            });

            stages.push(Stage {
                name: stage.name,
                commands: stage.commands,
                backend: if is_ssh_stage { Backend::Ssh } else { Backend::Local },
                working_dir: stage.working_dir,
                fail_fast: stage.fail_fast,
                health_check: stage.health_check,
            });
        }
    }

    // If no deploy workflow stages were added, fall back to docker compose
    if stages.is_empty() && has_docker_compose {
        stages.push(local_stage("docker-build", vec!["docker compose build"]));
        stages.push(Stage {
            name: "docker-deploy".to_string(),
            commands: vec![
                "docker compose build".to_string(),
                "docker compose up -d".to_string(),
            ],
            backend: Backend::Ssh,
            working_dir: None,
            fail_fast: true,
            health_check: None,
        });
    }

    if stages.is_empty() {
        return None;
    }

    Some(Pipeline {
        name: format!("{} Deploy", repo_name),
        stages,
    })
}

// ---------------------------------------------------------------------------
// Deployment Detection and Pipeline Generation
// ---------------------------------------------------------------------------

/// Detected project type for deployment method suggestion.
#[derive(Debug, Clone, PartialEq)]
pub enum ProjectType {
    Rust,
    RustLibrary,
    Tauri,
    Node,
    NodeLibrary,
    Python,
    Go,
    StaticSite,
    DockerCompose,
    Unknown,
}

/// Detect the project type from repository contents.
pub fn detect_project_type(repo_path: &Path) -> ProjectType {
    let has_file = |name: &str| repo_path.join(name).exists();
    let has_dir = |name: &str| repo_path.join(name).is_dir();

    // Tauri detection (has src-tauri/tauri.conf.json)
    if has_file("src-tauri/tauri.conf.json") {
        return ProjectType::Tauri;
    }

    // Rust detection
    if has_file("Cargo.toml") {
        if is_rust_library(repo_path) {
            return ProjectType::RustLibrary;
        }
        return ProjectType::Rust;
    }

    // Node.js detection
    if has_file("package.json") {
        if is_npm_publishable(repo_path) {
            return ProjectType::NodeLibrary;
        }
        // Check for static site generators
        if is_static_site(repo_path) {
            return ProjectType::StaticSite;
        }
        return ProjectType::Node;
    }

    // Python detection
    if has_file("pyproject.toml") || has_file("requirements.txt") || has_file("setup.py") {
        return ProjectType::Python;
    }

    // Go detection
    if has_file("go.mod") {
        return ProjectType::Go;
    }

    // Docker Compose detection (fullstack)
    if has_any_docker_compose(repo_path) {
        return ProjectType::DockerCompose;
    }

    // Check for static site patterns without package.json
    if has_dir("public") || has_dir("static") {
        if has_file("index.html") || has_file("public/index.html") || has_file("static/index.html") {
            return ProjectType::StaticSite;
        }
    }

    ProjectType::Unknown
}

/// Check if Cargo.toml indicates a library (has [lib] section or no [[bin]]).
fn is_rust_library(repo_path: &Path) -> bool {
    let cargo_path = repo_path.join("Cargo.toml");
    if let Ok(content) = std::fs::read_to_string(&cargo_path) {
        // Has explicit [lib] section
        if content.contains("[lib]") {
            return true;
        }
        // Has only library target (no [[bin]] sections and no src/main.rs)
        if !content.contains("[[bin]]") && !repo_path.join("src/main.rs").exists() {
            return true;
        }
    }
    false
}

/// Check if package.json indicates an npm-publishable package (not private).
fn is_npm_publishable(repo_path: &Path) -> bool {
    let pkg_path = repo_path.join("package.json");
    if let Ok(content) = std::fs::read_to_string(&pkg_path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            // Check if "private" is not set or is false
            if let Some(private) = json.get("private") {
                return !private.as_bool().unwrap_or(false);
            }
            // Check if it has npm-related fields that indicate a package
            let has_main = json.get("main").is_some();
            let has_exports = json.get("exports").is_some();
            let has_files = json.get("files").is_some();
            return has_main || has_exports || has_files;
        }
    }
    false
}

/// Check if the project is a static site.
fn is_static_site(repo_path: &Path) -> bool {
    // Check for common static site generators
    let pkg_path = repo_path.join("package.json");
    if let Ok(content) = std::fs::read_to_string(&pkg_path) {
        let content_lower = content.to_lowercase();
        // Check for static site generator dependencies
        if content_lower.contains("\"astro\"")
            || content_lower.contains("\"vuepress\"")
            || content_lower.contains("\"docusaurus\"")
            || content_lower.contains("\"eleventy\"")
            || content_lower.contains("\"gatsby\"")
            || content_lower.contains("\"hugo\"")
            || content_lower.contains("\"jekyll\"")
        {
            return true;
        }
    }
    // Check for common static site config files
    repo_path.join("astro.config.mjs").exists()
        || repo_path.join("astro.config.ts").exists()
        || repo_path.join("docusaurus.config.js").exists()
        || repo_path.join(".eleventy.js").exists()
        || repo_path.join("gatsby-config.js").exists()
}

/// Check for GitHub Actions deploy workflows.
fn has_deploy_workflow(repo_path: &Path) -> bool {
    let workflows_dir = repo_path.join(".github/workflows");
    if !workflows_dir.is_dir() {
        return false;
    }

    if let Ok(entries) = std::fs::read_dir(&workflows_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if (name.contains("deploy") || name.contains("release") || name.contains("publish"))
                && (name.ends_with(".yml") || name.ends_with(".yaml"))
            {
                return true;
            }
        }
    }
    false
}

/// Detect the most likely deployment method based on project files.
pub fn detect_deployment_method(repo_path: &Path) -> DeploymentMethod {
    // 1. Check for GitHub Actions deploy workflows
    if has_deploy_workflow(repo_path) {
        return DeploymentMethod::AutoDetect;
    }

    let project_type = detect_project_type(repo_path);

    // 2. Project-type specific detection
    match project_type {
        ProjectType::RustLibrary => {
            return DeploymentMethod::CargoPublish;
        }
        ProjectType::Rust => {
            return DeploymentMethod::GithubRelease;
        }
        ProjectType::Tauri => {
            return DeploymentMethod::GithubRelease;
        }
        ProjectType::NodeLibrary => {
            return DeploymentMethod::NpmPublish;
        }
        ProjectType::StaticSite => {
            // Check for specific platform configs
            if repo_path.join("netlify.toml").exists() {
                return DeploymentMethod::Netlify;
            }
            if repo_path.join("vercel.json").exists() {
                return DeploymentMethod::Vercel;
            }
            return DeploymentMethod::Netlify; // Default for static sites
        }
        _ => {}
    }

    // 3. Docker-based projects
    if has_any_docker_compose(repo_path) {
        return DeploymentMethod::DockerComposeSsh;
    }
    if repo_path.join("Dockerfile").exists() {
        return DeploymentMethod::DockerRegistry;
    }

    // 4. PaaS config files
    if repo_path.join("fly.toml").exists() {
        return DeploymentMethod::Flyio;
    }
    if repo_path.join("render.yaml").exists() {
        return DeploymentMethod::Render;
    }
    if repo_path.join("railway.json").exists() || repo_path.join("railway.toml").exists() {
        return DeploymentMethod::Railway;
    }
    if repo_path.join("netlify.toml").exists() {
        return DeploymentMethod::Netlify;
    }
    if repo_path.join("vercel.json").exists() {
        return DeploymentMethod::Vercel;
    }

    // 5. Deploy scripts
    if repo_path.join("deploy.sh").exists() {
        return DeploymentMethod::SshRsync;
    }

    DeploymentMethod::Skip
}

/// Get all applicable deployment methods for a project.
pub fn get_suggested_deploy_methods(repo_path: &Path) -> Vec<DeploymentMethod> {
    let project_type = detect_project_type(repo_path);

    match project_type {
        ProjectType::Rust | ProjectType::RustLibrary => vec![
            DeploymentMethod::CargoPublish,
            DeploymentMethod::GithubRelease,
            DeploymentMethod::DockerComposeSsh,
            DeploymentMethod::Skip,
        ],
        ProjectType::Tauri => vec![
            DeploymentMethod::GithubRelease,
            DeploymentMethod::Skip,
        ],
        ProjectType::Node => vec![
            DeploymentMethod::DockerComposeSsh,
            DeploymentMethod::Vercel,
            DeploymentMethod::Netlify,
            DeploymentMethod::Flyio,
            DeploymentMethod::SshRsync,
            DeploymentMethod::Skip,
        ],
        ProjectType::NodeLibrary => vec![
            DeploymentMethod::NpmPublish,
            DeploymentMethod::GithubRelease,
            DeploymentMethod::Skip,
        ],
        ProjectType::Python => vec![
            DeploymentMethod::DockerComposeSsh,
            DeploymentMethod::Flyio,
            DeploymentMethod::Render,
            DeploymentMethod::Railway,
            DeploymentMethod::SshRsync,
            DeploymentMethod::Skip,
        ],
        ProjectType::Go => vec![
            DeploymentMethod::DockerComposeSsh,
            DeploymentMethod::GithubRelease,
            DeploymentMethod::SshRsync,
            DeploymentMethod::Skip,
        ],
        ProjectType::StaticSite => vec![
            DeploymentMethod::Netlify,
            DeploymentMethod::Vercel,
            DeploymentMethod::S3Static,
            DeploymentMethod::SshRsync,
            DeploymentMethod::Skip,
        ],
        ProjectType::DockerCompose => vec![
            DeploymentMethod::AutoDetect,
            DeploymentMethod::DockerComposeSsh,
            DeploymentMethod::DockerRegistry,
            DeploymentMethod::Skip,
        ],
        ProjectType::Unknown => vec![
            DeploymentMethod::DockerComposeSsh,
            DeploymentMethod::SshRsync,
            DeploymentMethod::Skip,
        ],
    }
}

/// Generate a deployment pipeline based on the deployment method.
pub fn generate_deployment_pipeline(
    repo_name: &str,
    deploy_config: &DeploymentConfig,
    repo_path: &Path,
) -> Option<Pipeline> {
    let mut stages = Vec::new();

    match deploy_config.method {
        DeploymentMethod::DockerComposeSsh => {
            stages.push(local_stage("docker-build", vec!["docker compose build"]));

            let compose_file = deploy_config.compose_file.as_deref().unwrap_or("docker-compose.yml");
            let compose_cmd = if compose_file != "docker-compose.yml" && compose_file != "compose.yml" {
                format!("docker compose -f {} pull && docker compose -f {} up -d --remove-orphans", compose_file, compose_file)
            } else {
                "docker compose pull && docker compose up -d --remove-orphans".to_string()
            };

            stages.push(Stage {
                name: "deploy".to_string(),
                commands: vec![compose_cmd],
                backend: Backend::Ssh,
                working_dir: None,
                fail_fast: true,
                health_check: None,
            });

            // Add health check if configured
            if let Some(health_url) = &deploy_config.health_check_url {
                stages.push(Stage {
                    name: "health-check".to_string(),
                    commands: vec![format!("curl -sf http://localhost{} || exit 1", health_url)],
                    backend: Backend::Ssh,
                    working_dir: None,
                    fail_fast: true,
                    health_check: Some(HealthCheck {
                        command: format!("curl -sf http://localhost{}", health_url),
                        retries: 5,
                        delay_secs: 10,
                    }),
                });
            }
        }

        DeploymentMethod::DockerRegistry => {
            let registry = deploy_config.docker_registry.as_deref().unwrap_or("ghcr.io");
            let image_name = format!("{}/{}", registry, repo_name.to_lowercase());

            stages.push(local_stage("docker-build", vec![
                &format!("docker build -t {} .", image_name),
                &format!("docker push {}", image_name),
            ]));

            stages.push(Stage {
                name: "deploy".to_string(),
                commands: vec![
                    format!("docker pull {}", image_name),
                    format!("docker stop {} || true", repo_name),
                    format!("docker rm {} || true", repo_name),
                    format!("docker run -d --name {} {}", repo_name, image_name),
                ],
                backend: Backend::Ssh,
                working_dir: None,
                fail_fast: true,
                health_check: None,
            });
        }

        DeploymentMethod::CargoPublish => {
            let mut commands = Vec::new();
            if deploy_config.dry_run_first {
                commands.push("cargo publish --dry-run".to_string());
            }
            commands.push("cargo publish".to_string());

            stages.push(Stage {
                name: "cargo-publish".to_string(),
                commands,
                backend: Backend::Local,
                working_dir: None,
                fail_fast: true,
                health_check: None,
            });
        }

        DeploymentMethod::NpmPublish => {
            let mut commands = Vec::new();
            if deploy_config.dry_run_first {
                commands.push("npm publish --dry-run".to_string());
            }
            commands.push("npm publish".to_string());

            stages.push(Stage {
                name: "npm-publish".to_string(),
                commands,
                backend: Backend::Local,
                working_dir: None,
                fail_fast: true,
                health_check: None,
            });
        }

        DeploymentMethod::GithubRelease => {
            // Try to detect version source
            let version_cmd = detect_version_command(repo_path);

            stages.push(Stage {
                name: "github-release".to_string(),
                commands: vec![
                    format!("gh release create v$({}) --generate-notes --draft ./dist/*", version_cmd),
                ],
                backend: Backend::Local,
                working_dir: None,
                fail_fast: true,
                health_check: None,
            });
        }

        DeploymentMethod::SshRsync => {
            let ssh_host = deploy_config.ssh_host.as_deref().unwrap_or("{{ssh_host}}");

            stages.push(local_stage("deploy", vec![
                &format!("rsync -avz --delete ./dist/ {}:~/app/", ssh_host),
            ]));

            stages.push(Stage {
                name: "restart".to_string(),
                commands: vec!["systemctl --user restart myapp || pm2 restart all".to_string()],
                backend: Backend::Ssh,
                working_dir: None,
                fail_fast: true,
                health_check: None,
            });
        }

        DeploymentMethod::Flyio => {
            stages.push(local_stage("deploy", vec!["fly deploy"]));

            if let Some(health_url) = &deploy_config.health_check_url {
                let app_name = deploy_config.platform_project.as_deref().unwrap_or(repo_name);
                stages.push(local_stage("health-check", vec![
                    &format!("curl -sf https://{}.fly.dev{} || exit 1", app_name, health_url),
                ]));
            }
        }

        DeploymentMethod::Render => {
            // Render typically auto-deploys on push, but we can trigger via API
            stages.push(local_stage("deploy", vec![
                "echo 'Render deploys automatically on push. Manual trigger:'",
                "# curl -X POST https://api.render.com/deploy/srv-xxx?key=xxx",
            ]));
        }

        DeploymentMethod::Railway => {
            stages.push(local_stage("deploy", vec!["railway up"]));
        }

        DeploymentMethod::Netlify => {
            // Detect build output directory
            let dist_dir = detect_dist_directory(repo_path);
            stages.push(local_stage("deploy", vec![
                &format!("netlify deploy --prod --dir={}", dist_dir),
            ]));
        }

        DeploymentMethod::Vercel => {
            stages.push(local_stage("deploy", vec!["vercel --prod"]));
        }

        DeploymentMethod::S3Static => {
            let dist_dir = detect_dist_directory(repo_path);
            let bucket = deploy_config.platform_project.as_deref().unwrap_or("{{s3_bucket}}");

            stages.push(local_stage("deploy", vec![
                &format!("aws s3 sync ./{} s3://{} --delete", dist_dir, bucket),
            ]));

            // Optional CloudFront invalidation
            stages.push(local_stage("invalidate-cache", vec![
                "# aws cloudfront create-invalidation --distribution-id {{distribution_id}} --paths '/*'",
            ]));
        }

        DeploymentMethod::AutoDetect => {
            // Parse GitHub Actions deploy workflows and convert to stages
            let workflows = parse_github_workflows(repo_path);
            let deploy_workflows: Vec<_> = workflows
                .iter()
                .filter(|w| {
                    let name_lower = w.name.to_lowercase();
                    let file_lower = w.file_path.to_lowercase();
                    name_lower.contains("deploy")
                        || name_lower.contains("release")
                        || name_lower.contains("publish")
                        || file_lower.contains("deploy")
                        || file_lower.contains("release")
                        || file_lower.contains("publish")
                })
                .cloned()
                .collect();

            if !deploy_workflows.is_empty() {
                let workflow_stages = workflows_to_stages(&deploy_workflows);
                for stage in workflow_stages {
                    let is_ssh_stage = stage.commands.iter().any(|cmd| {
                        cmd.contains("ssh ") || cmd.contains("rsync") || cmd.contains("scp ")
                    });

                    stages.push(Stage {
                        name: stage.name,
                        commands: stage.commands,
                        backend: if is_ssh_stage { Backend::Ssh } else { Backend::Local },
                        working_dir: stage.working_dir,
                        fail_fast: stage.fail_fast,
                        health_check: stage.health_check,
                    });
                }
            }

            // If no stages from workflows, fall back to Docker if available
            if stages.is_empty() && has_any_docker_compose(repo_path) {
                stages.push(local_stage("docker-build", vec!["docker compose build"]));
                stages.push(Stage {
                    name: "deploy".to_string(),
                    commands: vec![
                        "docker compose pull".to_string(),
                        "docker compose up -d --remove-orphans".to_string(),
                    ],
                    backend: Backend::Ssh,
                    working_dir: None,
                    fail_fast: true,
                    health_check: None,
                });
            }
        }

        DeploymentMethod::Skip => {
            return None;
        }
    }

    if stages.is_empty() {
        return None;
    }

    Some(Pipeline {
        name: format!("{} Deploy", repo_name),
        stages,
    })
}

/// Generate default environments based on deployment method.
///
/// SSH-based deployments get production + staging environments.
/// PaaS deployments get production only.
/// Package publishing (Cargo, npm) and GitHub releases don't need environments.
pub fn generate_default_environments(deploy_config: &DeploymentConfig) -> Option<EnvironmentsConfig> {
    let envs = match deploy_config.method {
        // SSH-based deployments: production + staging
        DeploymentMethod::DockerComposeSsh
        | DeploymentMethod::DockerRegistry
        | DeploymentMethod::SshRsync => {
            vec![
                Environment {
                    name: "production".to_string(),
                    ssh_host: deploy_config.ssh_host.clone(),
                    ssh_port: None,
                    variables: HashMap::new(),
                },
                Environment {
                    name: "staging".to_string(),
                    ssh_host: None, // User fills in later
                    ssh_port: None,
                    variables: HashMap::new(),
                },
            ]
        }

        // PaaS deployments: production only
        DeploymentMethod::Flyio
        | DeploymentMethod::Render
        | DeploymentMethod::Railway
        | DeploymentMethod::Vercel
        | DeploymentMethod::Netlify
        | DeploymentMethod::S3Static => {
            vec![Environment {
                name: "production".to_string(),
                ssh_host: None,
                ssh_port: None,
                variables: HashMap::new(),
            }]
        }

        // No environments needed for these
        DeploymentMethod::CargoPublish
        | DeploymentMethod::NpmPublish
        | DeploymentMethod::GithubRelease
        | DeploymentMethod::AutoDetect
        | DeploymentMethod::Skip => {
            return None;
        }
    };

    Some(EnvironmentsConfig { environments: envs })
}

/// Detect the command to get version from project files.
fn detect_version_command(repo_path: &Path) -> String {
    // Check for VERSION file
    if repo_path.join("VERSION").exists() {
        return "cat VERSION".to_string();
    }

    // Check for Cargo.toml
    if repo_path.join("Cargo.toml").exists() {
        return "cargo metadata --format-version 1 --no-deps | jq -r '.packages[0].version'".to_string();
    }

    // Check for package.json
    if repo_path.join("package.json").exists() {
        return "node -p \"require('./package.json').version\"".to_string();
    }

    // Check for pyproject.toml
    if repo_path.join("pyproject.toml").exists() {
        return "grep -m1 'version' pyproject.toml | cut -d'\"' -f2".to_string();
    }

    // Fallback
    "cat VERSION".to_string()
}

/// Detect the build output directory for static sites.
fn detect_dist_directory(repo_path: &Path) -> String {
    // Common output directories
    for dir in &["dist", "build", "out", "public", "_site", ".next/out"] {
        if repo_path.join(dir).is_dir() {
            return dir.to_string();
        }
    }

    // Check netlify.toml for publish directory
    if let Ok(content) = std::fs::read_to_string(repo_path.join("netlify.toml")) {
        for line in content.lines() {
            if line.trim().starts_with("publish") {
                if let Some(value) = line.split('=').nth(1) {
                    return value.trim().trim_matches('"').to_string();
                }
            }
        }
    }

    // Default
    "dist".to_string()
}

// ---------------------------------------------------------------------------
// CI Workflow Parsing (GitHub Actions, etc.)
// ---------------------------------------------------------------------------

/// A parsed step from a CI workflow.
#[derive(Debug, Clone)]
pub struct CiWorkflowStep {
    /// Name of the step (from CI config).
    pub name: Option<String>,
    /// The run command(s).
    pub run: String,
    /// Working directory if specified.
    pub working_directory: Option<String>,
}

/// A parsed job from a CI workflow.
#[derive(Debug, Clone)]
pub struct CiWorkflowJob {
    /// Job ID/name.
    pub name: String,
    /// Steps in this job.
    pub steps: Vec<CiWorkflowStep>,
}

/// A parsed CI workflow file.
#[derive(Debug, Clone)]
pub struct CiWorkflow {
    /// Workflow name.
    pub name: String,
    /// Source file path.
    pub file_path: String,
    /// Jobs in this workflow.
    pub jobs: Vec<CiWorkflowJob>,
}

/// Parse all GitHub Actions workflows in a repository.
pub fn parse_github_workflows(repo_path: &Path) -> Vec<CiWorkflow> {
    let workflows_dir = repo_path.join(".github/workflows");
    let mut workflows = Vec::new();

    if !workflows_dir.is_dir() {
        return workflows;
    }

    if let Ok(entries) = std::fs::read_dir(&workflows_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if name.ends_with(".yml") || name.ends_with(".yaml") {
                if let Some(workflow) = parse_single_github_workflow(&path) {
                    workflows.push(workflow);
                }
            }
        }
    }

    workflows
}

/// Parse a single GitHub Actions workflow file.
fn parse_single_github_workflow(file_path: &Path) -> Option<CiWorkflow> {
    let content = std::fs::read_to_string(file_path).ok()?;
    let yaml: serde_yaml::Value = serde_yaml::from_str(&content).ok()?;

    let workflow_name = yaml.get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| file_path.file_stem().unwrap_or_default().to_string_lossy().to_string());

    let mut jobs = Vec::new();

    if let Some(jobs_map) = yaml.get("jobs").and_then(|v| v.as_mapping()) {
        for (job_key, job_value) in jobs_map {
            let job_name = job_key.as_str().unwrap_or("unknown").to_string();
            let mut steps = Vec::new();

            if let Some(steps_arr) = job_value.get("steps").and_then(|v| v.as_sequence()) {
                for step in steps_arr {
                    // Only include steps with "run" commands (skip actions like checkout)
                    if let Some(run_cmd) = step.get("run").and_then(|v| v.as_str()) {
                        let step_name = step.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let working_dir = step.get("working-directory").and_then(|v| v.as_str()).map(|s| s.to_string());

                        steps.push(CiWorkflowStep {
                            name: step_name,
                            run: run_cmd.to_string(),
                            working_directory: working_dir,
                        });
                    }
                }
            }

            if !steps.is_empty() {
                jobs.push(CiWorkflowJob {
                    name: job_name,
                    steps,
                });
            }
        }
    }

    if jobs.is_empty() {
        return None;
    }

    Some(CiWorkflow {
        name: workflow_name,
        file_path: file_path.to_string_lossy().to_string(),
        jobs,
    })
}

/// Convert parsed CI workflows into pipeline stages.
///
/// This creates one stage per job, with each step's run commands combined.
pub fn workflows_to_stages(workflows: &[CiWorkflow]) -> Vec<Stage> {
    let mut stages = Vec::new();
    let mut seen_names: HashSet<String> = HashSet::new();

    for workflow in workflows {
        for job in &workflow.jobs {
            // Create a unique stage name
            let base_name = format!("ci-{}", job.name);
            let stage_name = if seen_names.contains(&base_name) {
                format!("{}-{}", base_name, workflow.name.to_lowercase().replace(' ', "-"))
            } else {
                base_name.clone()
            };
            seen_names.insert(stage_name.clone());

            // Collect all run commands from this job
            let commands: Vec<String> = job.steps
                .iter()
                .flat_map(|step| {
                    // Split multi-line run commands
                    step.run.lines()
                        .map(|line| line.trim().to_string())
                        .filter(|line| !line.is_empty() && !line.starts_with('#'))
                        .collect::<Vec<_>>()
                })
                .collect();

            if commands.is_empty() {
                continue;
            }

            // Use working directory from first step that has one
            let working_dir = job.steps
                .iter()
                .find_map(|s| s.working_directory.clone());

            stages.push(Stage {
                name: stage_name,
                commands,
                backend: Backend::Local,
                working_dir,
                fail_fast: true,
                health_check: None,
            });
        }
    }

    stages
}

// ---------------------------------------------------------------------------
// Pipeline Validation
// ---------------------------------------------------------------------------

/// Parse package.json and extract available script names.
fn parse_package_json_scripts(repo_path: &Path) -> HashSet<String> {
    let pkg_path = repo_path.join("package.json");
    let mut scripts = HashSet::new();

    if let Ok(content) = std::fs::read_to_string(&pkg_path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(scripts_obj) = json.get("scripts").and_then(|s| s.as_object()) {
                for key in scripts_obj.keys() {
                    scripts.insert(key.clone());
                }
            }
        }
    }

    scripts
}

/// Parse Makefile and extract available targets.
fn parse_makefile_targets(repo_path: &Path) -> HashSet<String> {
    let mut targets = HashSet::new();

    for name in &["Makefile", "makefile", "GNUmakefile"] {
        let makefile_path = repo_path.join(name);
        if let Ok(content) = std::fs::read_to_string(&makefile_path) {
            for line in content.lines() {
                // Match lines like "target:" or "target: deps"
                // Skip lines starting with . (special targets) or whitespace (recipes)
                if !line.starts_with('\t')
                    && !line.starts_with(' ')
                    && !line.starts_with('.')
                    && !line.starts_with('#')
                {
                    if let Some(colon_pos) = line.find(':') {
                        let target = line[..colon_pos].trim();
                        // Skip empty targets and targets with special chars
                        if !target.is_empty()
                            && !target.contains('%')
                            && !target.contains('$')
                            && !target.contains('=')
                        {
                            targets.insert(target.to_string());
                        }
                    }
                }
            }
            break; // Only parse the first Makefile found
        }
    }

    targets
}

/// Validate a pipeline against the actual project configuration.
///
/// Checks for common issues like:
/// - npm scripts that don't exist in package.json
/// - make targets that don't exist in Makefile
/// - Missing required files
/// - Duplicate/conflicting config files
pub fn validate_pipeline(pipeline: &Pipeline, repo_path: &Path) -> PipelineValidation {
    let mut warnings: Vec<PipelineWarning> = Vec::new();

    // Detect duplicate/conflicting config files
    let file_conflicts = detect_file_conflicts(repo_path);

    // Parse available scripts/targets from root
    let mut npm_scripts = parse_package_json_scripts(repo_path);
    let make_targets = parse_makefile_targets(repo_path);

    // Check if package.json exists (root or subdirectories)
    let has_root_package_json = repo_path.join("package.json").exists();
    let has_makefile = repo_path.join("Makefile").exists()
        || repo_path.join("makefile").exists()
        || repo_path.join("GNUmakefile").exists();

    // Also check subdirectories for package.json (fullstack projects)
    let mut subdir_package_jsons: std::collections::HashMap<String, HashSet<String>> = std::collections::HashMap::new();
    for subdir in FULLSTACK_SUBDIRS {
        let subdir_path = repo_path.join(subdir);
        if subdir_path.join("package.json").exists() {
            let scripts = parse_package_json_scripts(&subdir_path);
            subdir_package_jsons.insert(subdir.to_string(), scripts);
        }
    }

    // If no root package.json but subdirs have it, merge scripts for general validation
    let has_any_package_json = has_root_package_json || !subdir_package_jsons.is_empty();
    if !has_root_package_json {
        for scripts in subdir_package_jsons.values() {
            npm_scripts.extend(scripts.clone());
        }
    }

    for stage in &pipeline.stages {
        for cmd in &stage.commands {
            // Check npm commands - use smarter validation for fullstack projects
            if let Some(warning) = check_npm_command_fullstack(
                cmd,
                &npm_scripts,
                &subdir_package_jsons,
                has_any_package_json,
                &stage.name,
                repo_path,
            ) {
                warnings.push(warning);
            }

            // Check make commands
            if let Some(warning) = check_make_command(cmd, &make_targets, has_makefile, &stage.name) {
                warnings.push(warning);
            }

            // Check shell script exists
            if let Some(warning) = check_shell_script(cmd, repo_path, &stage.name) {
                warnings.push(warning);
            }
        }
    }

    let has_errors = warnings.iter().any(|w| w.severity == WarningSeverity::Error);

    PipelineValidation {
        warnings,
        file_conflicts,
        is_valid: !has_errors,
    }
}

/// Detect duplicate or conflicting configuration files in a repository.
///
/// This helps catch issues like having both Makefile and makefile,
/// which can cause unexpected behavior especially on case-insensitive filesystems.
fn detect_file_conflicts(repo_path: &Path) -> Vec<FileConflict> {
    let mut conflicts = Vec::new();

    // Define groups of files that conflict with each other
    let conflict_groups: &[(&str, &[&str], Option<&str>)] = &[
        // Makefiles - on case-sensitive systems, both can exist but cause confusion
        ("Makefile", &["Makefile", "makefile", "GNUmakefile"], Some("GNUmakefile > Makefile > makefile")),
        // Docker Compose - multiple variants
        ("Docker Compose", &["docker-compose.yml", "docker-compose.yaml", "compose.yml", "compose.yaml"], Some("compose.yml is the modern default")),
        // Taskfile
        ("Taskfile", &["Taskfile.yml", "Taskfile.yaml"], None),
        // Deno config
        ("Deno Config", &["deno.json", "deno.jsonc"], None),
        // Python project config (multiple ways to define a project)
        ("Python Project", &["pyproject.toml", "setup.py", "setup.cfg"], Some("pyproject.toml is the modern standard")),
        // Package lock files - indicates mixed package manager usage
        ("Package Lock", &["package-lock.json", "yarn.lock", "pnpm-lock.yaml", "bun.lockb"], Some("Use only one package manager")),
        // Gradle
        ("Gradle Build", &["build.gradle", "build.gradle.kts"], None),
    ];

    for (category, file_names, note) in conflict_groups {
        let existing: Vec<String> = file_names
            .iter()
            .filter(|name| repo_path.join(name).exists())
            .map(|s| s.to_string())
            .collect();

        if existing.len() > 1 {
            // Determine which file will be "active" (used by tools)
            let active = determine_active_file(category, &existing);

            conflicts.push(FileConflict {
                category: category.to_string(),
                files: existing.clone(),
                message: format!(
                    "Multiple {} files detected: {}. {}",
                    category,
                    existing.join(", "),
                    note.unwrap_or("This may cause confusion or unexpected behavior.")
                ),
                active_file: active,
            });
        }
    }

    // Check for duplicate .env files that might override each other
    let env_files = detect_env_file_conflicts(repo_path);
    if !env_files.is_empty() && env_files.len() > 3 {
        conflicts.push(FileConflict {
            category: "Environment Files".to_string(),
            files: env_files,
            message: "Many .env files detected. Ensure you know which ones are loaded and in what order.".to_string(),
            active_file: Some(".env".to_string()),
        });
    }

    conflicts
}

/// Determine which file in a conflict group will be used by tools.
fn determine_active_file(category: &str, files: &[String]) -> Option<String> {
    match category {
        "Makefile" => {
            // GNU make precedence: GNUmakefile > makefile > Makefile
            if files.contains(&"GNUmakefile".to_string()) {
                Some("GNUmakefile".to_string())
            } else if files.contains(&"makefile".to_string()) {
                Some("makefile".to_string())
            } else {
                Some("Makefile".to_string())
            }
        }
        "Docker Compose" => {
            // Docker Compose V2 precedence
            if files.contains(&"compose.yaml".to_string()) {
                Some("compose.yaml".to_string())
            } else if files.contains(&"compose.yml".to_string()) {
                Some("compose.yml".to_string())
            } else if files.contains(&"docker-compose.yaml".to_string()) {
                Some("docker-compose.yaml".to_string())
            } else {
                Some("docker-compose.yml".to_string())
            }
        }
        _ => files.first().cloned(),
    }
}

/// Detect environment files in the repository.
fn detect_env_file_conflicts(repo_path: &Path) -> Vec<String> {
    let mut env_files = Vec::new();

    if let Ok(entries) = std::fs::read_dir(repo_path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == ".env" || (name.starts_with(".env.") && !name.ends_with(".example") && !name.ends_with(".sample") && !name.ends_with(".template")) {
                env_files.push(name);
            }
        }
    }

    env_files.sort();
    env_files
}

/// Check if an npm command references a script that exists (fullstack-aware).
///
/// Handles:
/// - Commands with `cd <subdir> &&` prefix
/// - Fullstack projects where package.json is in subdirectories
fn check_npm_command_fullstack(
    cmd: &str,
    all_npm_scripts: &HashSet<String>,
    subdir_scripts: &std::collections::HashMap<String, HashSet<String>>,
    has_any_package_json: bool,
    stage_name: &str,
    repo_path: &Path,
) -> Option<PipelineWarning> {
    let cmd_trimmed = cmd.trim();

    // Check if command starts with "cd <dir> &&" - if so, validate against that subdir
    if cmd_trimmed.starts_with("cd ") {
        if let Some(rest) = cmd_trimmed.strip_prefix("cd ") {
            // Parse "cd frontend && npm run build" -> ("frontend", "npm run build")
            if let Some((subdir, npm_cmd)) = rest.split_once(" && ") {
                let subdir = subdir.trim();
                let npm_cmd = npm_cmd.trim();

                // Check if the subdir has a package.json
                if let Some(scripts) = subdir_scripts.get(subdir) {
                    return check_npm_command(npm_cmd, scripts, true, stage_name);
                } else if repo_path.join(subdir).join("package.json").exists() {
                    // Subdir exists but wasn't pre-parsed, treat as valid
                    return None;
                }
                // If cd to a dir without package.json, let it pass for now
                // (might be Docker or other setup)
                return None;
            }
        }
    }

    // For regular npm commands, check against all available scripts
    // For fullstack projects with package.json only in subdirs, use softer validation
    let has_root_package_json = repo_path.join("package.json").exists();
    let is_fullstack = !has_root_package_json && !subdir_scripts.is_empty();

    if is_fullstack {
        // For fullstack projects, npm commands without cd prefix are likely from
        // imported CI workflows - give a more helpful message
        return check_npm_command_fullstack_soft(cmd_trimmed, all_npm_scripts, stage_name, subdir_scripts);
    }

    check_npm_command(cmd_trimmed, all_npm_scripts, has_any_package_json, stage_name)
}

/// Softer validation for fullstack projects - downgrades errors to warnings
fn check_npm_command_fullstack_soft(
    cmd: &str,
    all_npm_scripts: &HashSet<String>,
    stage_name: &str,
    subdir_scripts: &std::collections::HashMap<String, HashSet<String>>,
) -> Option<PipelineWarning> {
    let cmd_trimmed = cmd.trim();

    // Match "npm test", "npm run <script>", "npm run-script <script>"
    if cmd_trimmed.starts_with("npm ") || cmd_trimmed.starts_with("yarn ") || cmd_trimmed.starts_with("pnpm ") {
        let script_name = extract_npm_script_name(cmd_trimmed);

        if let Some(script) = script_name {
            // Check if script exists in any subdir
            let found_in_subdir = subdir_scripts.iter().find(|(_, scripts)| scripts.contains(&script));

            if let Some((subdir, _)) = found_in_subdir {
                // Script found in a subdirectory - suggest adding cd prefix
                return Some(PipelineWarning {
                    stage_name: stage_name.to_string(),
                    command: cmd.to_string(),
                    message: format!("Script '{}' found in {}/package.json, not root", script, subdir),
                    suggestion: Some(format!("Add 'cd {} && ' prefix or move script to root package.json", subdir)),
                    severity: WarningSeverity::Warning, // Warning, not error
                });
            } else if !all_npm_scripts.contains(&script) {
                // Script not found anywhere - list available subdirs
                let subdirs: Vec<&str> = subdir_scripts.keys().map(|s| s.as_str()).collect();
                let subdir_hint = if subdirs.is_empty() {
                    "a subdirectory".to_string()
                } else {
                    subdirs.join("/")
                };
                return Some(PipelineWarning {
                    stage_name: stage_name.to_string(),
                    command: cmd.to_string(),
                    message: format!("Script '{}' not found in any package.json", script),
                    suggestion: Some(format!("Add the script to {}/package.json or remove this stage", subdir_hint)),
                    severity: WarningSeverity::Warning, // Warning for fullstack projects
                });
            }
        }
    }

    None
}

/// Extract the script name from an npm command, handling flags.
///
/// Examples:
/// - "npm run build" -> Some("build")
/// - "npm run -s lint:docs" -> Some("lint:docs")
/// - "npm test -- --coverage" -> Some("test")
/// - "npm run test:critical" -> Some("test:critical")
fn extract_npm_script_name(cmd: &str) -> Option<String> {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }

    match parts[1] {
        "test" | "start" | "build" => Some(parts[1].to_string()),
        "run" | "run-script" => {
            // Find the script name, skipping flags like -s, --silent, etc.
            for part in parts.iter().skip(2) {
                // Skip flags (start with -)
                if part.starts_with('-') {
                    continue;
                }
                // Skip if it's after -- (passthrough args)
                if *part == "--" {
                    break;
                }
                return Some(part.to_string());
            }
            None
        }
        _ => None,
    }
}

/// Check if an npm command references a script that exists.
fn check_npm_command(
    cmd: &str,
    npm_scripts: &HashSet<String>,
    has_package_json: bool,
    stage_name: &str,
) -> Option<PipelineWarning> {
    let cmd_trimmed = cmd.trim();

    // Match "npm test", "npm run <script>", "npm run-script <script>"
    if cmd_trimmed.starts_with("npm ") || cmd_trimmed.starts_with("yarn ") || cmd_trimmed.starts_with("pnpm ") {
        let script_name = extract_npm_script_name(cmd_trimmed);

        if let Some(script) = script_name {
            if !has_package_json {
                return Some(PipelineWarning {
                    stage_name: stage_name.to_string(),
                    command: cmd.to_string(),
                    message: "No package.json found in project root".to_string(),
                    suggestion: Some("Ensure package.json exists or remove this stage".to_string()),
                    severity: WarningSeverity::Error,
                });
            }

            if !npm_scripts.contains(&script) {
                return Some(PipelineWarning {
                    stage_name: stage_name.to_string(),
                    command: cmd.to_string(),
                    message: format!("Script '{}' not found in package.json", script),
                    suggestion: Some(format!(
                        "Add a '{}' script to package.json or remove this stage",
                        script
                    )),
                    severity: WarningSeverity::Error,
                });
            }
        }
    }

    None
}

/// Check if a make command references a target that exists.
fn check_make_command(
    cmd: &str,
    make_targets: &HashSet<String>,
    has_makefile: bool,
    stage_name: &str,
) -> Option<PipelineWarning> {
    let cmd_trimmed = cmd.trim();

    if cmd_trimmed.starts_with("make ") {
        let parts: Vec<&str> = cmd_trimmed.split_whitespace().collect();
        if parts.len() >= 2 {
            // Skip flags like -j, -f, etc.
            let target = parts.iter().skip(1).find(|p| !p.starts_with('-'));

            if let Some(target) = target {
                if !has_makefile {
                    return Some(PipelineWarning {
                        stage_name: stage_name.to_string(),
                        command: cmd.to_string(),
                        message: "No Makefile found in project root".to_string(),
                        suggestion: Some("Create a Makefile or remove this stage".to_string()),
                        severity: WarningSeverity::Error,
                    });
                }

                // Only warn if we actually parsed some targets (we might have missed some)
                if !make_targets.is_empty() && !make_targets.contains(*target) {
                    return Some(PipelineWarning {
                        stage_name: stage_name.to_string(),
                        command: cmd.to_string(),
                        message: format!("Target '{}' not found in Makefile", target),
                        suggestion: Some(format!(
                            "Add a '{}' target to Makefile or remove this stage",
                            target
                        )),
                        severity: WarningSeverity::Warning, // Warning because our parser might miss targets
                    });
                }
            }
        }
    }

    None
}

/// Check if a shell script file exists.
fn check_shell_script(cmd: &str, repo_path: &Path, stage_name: &str) -> Option<PipelineWarning> {
    let cmd_trimmed = cmd.trim();

    // Match "./script.sh" or "bash script.sh" or "sh script.sh"
    let script_path = if cmd_trimmed.starts_with("./") {
        Some(&cmd_trimmed[2..])
    } else if cmd_trimmed.starts_with("bash ") || cmd_trimmed.starts_with("sh ") {
        cmd_trimmed.split_whitespace().nth(1)
    } else {
        None
    };

    if let Some(script) = script_path {
        // Extract just the script path (ignore arguments)
        let script_file = script.split_whitespace().next().unwrap_or(script);

        if script_file.ends_with(".sh") {
            let full_path = repo_path.join(script_file);
            if !full_path.exists() {
                return Some(PipelineWarning {
                    stage_name: stage_name.to_string(),
                    command: cmd.to_string(),
                    message: format!("Script '{}' not found", script_file),
                    suggestion: Some(format!("Create {} or remove this stage", script_file)),
                    severity: WarningSeverity::Error,
                });
            }
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Unit Tests
// ---------------------------------------------------------------------------

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
        assert_eq!(classify_file("docker-compose.yml"), ScriptType::DockerCompose);
    }

    #[test]
    fn test_classify_file_python() {
        assert_eq!(classify_file("pyproject.toml"), ScriptType::PythonProject);
        assert_eq!(classify_file("requirements.txt"), ScriptType::PythonRequirements);
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
        assert!(scripts.iter().any(|s| s.script_type == ScriptType::PackageJson));
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
        assert!(scripts.iter().any(|s| s.script_type == ScriptType::Makefile));
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
        assert!(scripts.iter().any(|s| s.script_type == ScriptType::CargoToml));
    }

    #[test]
    fn test_generate_draft_pipeline_fallback() {
        let tmp = tempfile::tempdir().unwrap();
        // Empty scripts -> fallback stage
        let pipeline = generate_draft_pipeline("test-repo", &[], tmp.path());

        assert_eq!(pipeline.name, "test-repo Pipeline");
        assert!(!pipeline.stages.is_empty());
    }

    #[test]
    fn test_generate_draft_pipeline_npm() {
        let tmp = tempfile::tempdir().unwrap();
        let scripts = vec![DetectedScript {
            file_name: "package.json".to_string(),
            file_path: "/test/package.json".to_string(),
            script_type: ScriptType::PackageJson,
        }];

        let pipeline = generate_draft_pipeline("my-app", &scripts, tmp.path());
        assert!(!pipeline.stages.is_empty());

        // Should generate npm-based stages
        let stage_commands: Vec<_> = pipeline
            .stages
            .iter()
            .flat_map(|s| &s.commands)
            .collect();
        assert!(stage_commands.iter().any(|c| c.contains("npm")));
    }

    #[test]
    fn test_generate_draft_pipeline_rust() {
        let tmp = tempfile::tempdir().unwrap();
        let scripts = vec![DetectedScript {
            file_name: "Cargo.toml".to_string(),
            file_path: "/test/Cargo.toml".to_string(),
            script_type: ScriptType::CargoToml,
        }];

        let pipeline = generate_draft_pipeline("rust-app", &scripts, tmp.path());

        // Should detect Rust and add cargo stages
        let stage_commands: Vec<_> = pipeline
            .stages
            .iter()
            .flat_map(|s| &s.commands)
            .collect();
        assert!(stage_commands.iter().any(|c| c.contains("cargo")));
    }

    #[test]
    fn test_validate_pipeline_no_issues() {
        let pipeline = Pipeline {
            name: "Test".to_string(),
            stages: vec![Stage {
                name: "build".to_string(),
                commands: vec!["echo building".to_string()],
                backend: Backend::Local,
                working_dir: None,
                fail_fast: true,
                health_check: None,
            }],
        };

        let temp = TempDir::new().unwrap();
        let validation = validate_pipeline(&pipeline, temp.path());

        // Echo commands don't require any special files
        assert!(validation.warnings.is_empty());
        assert!(validation.is_valid);
    }

    #[test]
    fn test_validate_pipeline_npm_missing_script() {
        let pipeline = Pipeline {
            name: "Test".to_string(),
            stages: vec![Stage {
                name: "test".to_string(),
                commands: vec!["npm run nonexistent".to_string()],
                backend: Backend::Local,
                working_dir: None,
                fail_fast: true,
                health_check: None,
            }],
        };

        let temp = TempDir::new().unwrap();
        // Create package.json without the script
        std::fs::write(
            temp.path().join("package.json"),
            r#"{"name": "test", "scripts": {"build": "tsc"}}"#,
        )
        .unwrap();

        let validation = validate_pipeline(&pipeline, temp.path());

        // Should warn about missing npm script
        assert!(validation
            .warnings
            .iter()
            .any(|w| w.message.contains("not found")));
    }

    #[test]
    fn test_validate_pipeline_missing_script() {
        let pipeline = Pipeline {
            name: "Test".to_string(),
            stages: vec![Stage {
                name: "deploy".to_string(),
                commands: vec!["./deploy.sh".to_string()],
                backend: Backend::Local,
                working_dir: None,
                fail_fast: true,
                health_check: None,
            }],
        };

        let temp = TempDir::new().unwrap();
        let validation = validate_pipeline(&pipeline, temp.path());

        // Should warn about missing deploy.sh
        assert!(validation
            .warnings
            .iter()
            .any(|w| w.message.contains("not found")));
    }

    #[test]
    fn test_validate_pipeline_valid_script() {
        let temp = TempDir::new().unwrap();

        // Create an actual script
        std::fs::write(temp.path().join("build.sh"), "#!/bin/bash\necho build").unwrap();

        let pipeline = Pipeline {
            name: "Test".to_string(),
            stages: vec![Stage {
                name: "build".to_string(),
                commands: vec!["./build.sh".to_string()],
                backend: Backend::Local,
                working_dir: None,
                fail_fast: true,
                health_check: None,
            }],
        };

        let validation = validate_pipeline(&pipeline, temp.path());

        // Should not warn about missing script
        assert!(!validation
            .warnings
            .iter()
            .any(|w| w.message.contains("not found")));
    }

    #[test]
    fn test_parse_github_workflows_empty() {
        let temp = TempDir::new().unwrap();
        let workflows = parse_github_workflows(temp.path());
        assert!(workflows.is_empty());
    }

    #[test]
    fn test_parse_github_workflows_with_file() {
        let temp = TempDir::new().unwrap();
        let workflows_dir = temp.path().join(".github/workflows");
        std::fs::create_dir_all(&workflows_dir).unwrap();

        std::fs::write(
            workflows_dir.join("ci.yml"),
            r#"
name: CI
on: [push]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Build
        run: npm run build
"#,
        )
        .unwrap();

        let workflows = parse_github_workflows(temp.path());
        assert!(!workflows.is_empty());
        assert_eq!(workflows[0].name, "CI");
    }

    #[test]
    fn test_check_shell_script_missing() {
        let temp = TempDir::new().unwrap();

        let warning = check_shell_script("./missing.sh", temp.path(), "deploy");
        assert!(warning.is_some());
        assert!(warning.unwrap().message.contains("not found"));
    }

    #[test]
    fn test_check_shell_script_exists() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("exists.sh"), "echo hi").unwrap();

        let warning = check_shell_script("./exists.sh", temp.path(), "deploy");
        assert!(warning.is_none());
    }

    #[test]
    fn test_check_shell_script_bash_command() {
        let temp = TempDir::new().unwrap();

        let warning = check_shell_script("bash missing.sh", temp.path(), "test");
        assert!(warning.is_some());

        std::fs::write(temp.path().join("exists.sh"), "").unwrap();
        let warning = check_shell_script("bash exists.sh", temp.path(), "test");
        assert!(warning.is_none());
    }

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
        ).unwrap();

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
        ).unwrap();

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
            ).unwrap();
        }

        let folders = detect_project_folders(temp.path());
        assert_eq!(folders.len(), 3);
        assert!(folders.iter().any(|f| f.name == "frontend"));
        assert!(folders.iter().any(|f| f.name == "backend"));
        assert!(folders.iter().any(|f| f.name == "admin"));
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

    #[test]
    fn test_generate_pipeline_fullstack_all_folders() {
        let temp = TempDir::new().unwrap();

        // Create frontend, backend, admin directories with package.json
        for name in &["frontend", "backend", "admin"] {
            let dir = temp.path().join(name);
            std::fs::create_dir(&dir).unwrap();
            std::fs::write(
                dir.join("package.json"),
                format!(r#"{{"name": "{}", "scripts": {{"lint": "eslint", "test": "vitest"}}}}"#, name),
            ).unwrap();
        }

        // Create docker-compose.yml
        std::fs::write(temp.path().join("docker-compose.yml"), "version: '3'").unwrap();

        let scripts = detect_scripts(temp.path());
        let pipeline = generate_draft_pipeline("bituntu", &scripts, temp.path());

        // Should have stages for ALL folders (frontend, backend, admin)
        let stage_names: Vec<_> = pipeline.stages.iter().map(|s| s.name.as_str()).collect();

        assert!(stage_names.contains(&"frontend-install"), "Missing frontend-install: {:?}", stage_names);
        assert!(stage_names.contains(&"frontend-lint"), "Missing frontend-lint: {:?}", stage_names);
        assert!(stage_names.contains(&"frontend-test"), "Missing frontend-test: {:?}", stage_names);

        assert!(stage_names.contains(&"backend-install"), "Missing backend-install: {:?}", stage_names);
        assert!(stage_names.contains(&"backend-lint"), "Missing backend-lint: {:?}", stage_names);
        assert!(stage_names.contains(&"backend-test"), "Missing backend-test: {:?}", stage_names);

        assert!(stage_names.contains(&"admin-install"), "Missing admin-install: {:?}", stage_names);
        assert!(stage_names.contains(&"admin-lint"), "Missing admin-lint: {:?}", stage_names);
        assert!(stage_names.contains(&"admin-test"), "Missing admin-test: {:?}", stage_names);

        // Docker-deploy should NOT be in CI pipeline (it goes in deploy.toml)
        assert!(!stage_names.contains(&"docker-deploy"), "docker-deploy should not be in CI pipeline for fullstack");
    }

    #[test]
    fn test_generate_deploy_pipeline() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("docker-compose.yml"), "version: '3'").unwrap();

        let scripts = detect_scripts(temp.path());
        let deploy = generate_deploy_pipeline("test", &scripts, temp.path());

        assert!(deploy.is_some());
        let pipeline = deploy.unwrap();
        assert_eq!(pipeline.name, "test Deploy");

        let stage_names: Vec<_> = pipeline.stages.iter().map(|s| s.name.as_str()).collect();
        assert!(stage_names.contains(&"docker-build"));
        assert!(stage_names.contains(&"docker-deploy"));
    }

    #[test]
    fn test_fullstack_pipeline_commands_have_cd_prefix() {
        let temp = TempDir::new().unwrap();

        // Create frontend directory
        let frontend = temp.path().join("frontend");
        std::fs::create_dir(&frontend).unwrap();
        std::fs::write(
            frontend.join("package.json"),
            r#"{"name": "frontend", "scripts": {"lint": "eslint", "test": "vitest"}}"#,
        ).unwrap();

        // Create backend with Python
        let backend = temp.path().join("backend");
        std::fs::create_dir(&backend).unwrap();
        std::fs::write(backend.join("requirements.txt"), "flask").unwrap();
        std::fs::create_dir(backend.join("tests")).unwrap();

        let scripts = detect_scripts(temp.path());
        let pipeline = generate_draft_pipeline("myapp", &scripts, temp.path());

        // Check that commands have proper cd prefix
        for stage in &pipeline.stages {
            for cmd in &stage.commands {
                if stage.name.starts_with("frontend-") {
                    assert!(cmd.starts_with("cd frontend && "), "Command missing 'cd frontend &&' prefix: {}", cmd);
                } else if stage.name.starts_with("backend-") {
                    assert!(cmd.starts_with("cd backend &&" ), "Command missing 'cd backend &&' prefix: {}", cmd);
                }
            }
        }
    }

    #[test]
    fn test_generate_default_environments_ssh_deploys() {
        // SSH-based deployments should create production + staging
        for method in [
            DeploymentMethod::DockerComposeSsh,
            DeploymentMethod::DockerRegistry,
            DeploymentMethod::SshRsync,
        ] {
            let config = DeploymentConfig {
                method,
                ssh_host: Some("user@example.com".to_string()),
                ..Default::default()
            };
            let result = generate_default_environments(&config);
            assert!(result.is_some(), "Expected environments for {:?}", config.method);
            let envs = result.unwrap();
            assert_eq!(envs.environments.len(), 2);
            assert_eq!(envs.environments[0].name, "production");
            assert_eq!(envs.environments[0].ssh_host, Some("user@example.com".to_string()));
            assert_eq!(envs.environments[1].name, "staging");
            assert!(envs.environments[1].ssh_host.is_none());
        }
    }

    #[test]
    fn test_generate_default_environments_paas() {
        // PaaS deployments should create production only
        for method in [
            DeploymentMethod::Flyio,
            DeploymentMethod::Render,
            DeploymentMethod::Railway,
            DeploymentMethod::Vercel,
            DeploymentMethod::Netlify,
            DeploymentMethod::S3Static,
        ] {
            let config = DeploymentConfig {
                method,
                ..Default::default()
            };
            let result = generate_default_environments(&config);
            assert!(result.is_some(), "Expected environments for {:?}", config.method);
            let envs = result.unwrap();
            assert_eq!(envs.environments.len(), 1);
            assert_eq!(envs.environments[0].name, "production");
            assert!(envs.environments[0].ssh_host.is_none());
        }
    }

    #[test]
    fn test_generate_default_environments_no_envs() {
        // These methods should not create any environments
        for method in [
            DeploymentMethod::CargoPublish,
            DeploymentMethod::NpmPublish,
            DeploymentMethod::GithubRelease,
            DeploymentMethod::AutoDetect,
            DeploymentMethod::Skip,
        ] {
            let config = DeploymentConfig {
                method,
                ..Default::default()
            };
            let result = generate_default_environments(&config);
            assert!(result.is_none(), "Expected no environments for {:?}", config.method);
        }
    }
}
