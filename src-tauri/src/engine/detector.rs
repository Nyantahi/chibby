use crate::engine::models::{Pipeline, Stage, Backend, PipelineValidation, PipelineWarning, WarningSeverity, FileConflict};
use std::collections::HashSet;
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

    // Scan repo root for *.sh, .env*, and .sln/*.csproj files.
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

    // Read package.json scripts for smarter generation.
    let pkg_scripts = if has(ScriptType::PackageJson) {
        read_package_scripts(repo_path)
    } else {
        std::collections::HashMap::new()
    };
    let has_script = |name: &str| pkg_scripts.contains_key(name);

    // ── Install dependencies ─────────────────────────────────────
    if has(ScriptType::PackageJson) {
        stages.push(local_stage("install", vec!["npm ci"]));
    }

    // ── Type checking ────────────────────────────────────────────
    if has_script("type-check") {
        stages.push(local_stage("type-check", vec!["npm run type-check"]));
    } else if has(ScriptType::TsConfig) && has(ScriptType::PackageJson) {
        stages.push(local_stage("type-check", vec!["npx tsc --noEmit"]));
    }

    // ── Linting ──────────────────────────────────────────────────
    if has_script("lint") {
        stages.push(local_stage("lint", vec!["npm run lint"]));
    } else if has(ScriptType::Eslint) && has(ScriptType::PackageJson) {
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
    if has(ScriptType::PythonProject) || has(ScriptType::PythonRequirements) {
        if has(ScriptType::Tox) {
            stages.push(local_stage("tox", vec!["tox"]));
        } else {
            let install_cmd = if has(ScriptType::PythonRequirements) {
                "pip install -r requirements.txt"
            } else {
                "pip install -e ."
            };
            stages.push(local_stage("pip-install", vec![install_cmd]));
            stages.push(local_stage("pytest", vec!["pytest"]));
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
    if has(ScriptType::DockerCompose) {
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

    // Parse available scripts/targets
    let npm_scripts = parse_package_json_scripts(repo_path);
    let make_targets = parse_makefile_targets(repo_path);

    // Check if package.json exists
    let has_package_json = repo_path.join("package.json").exists();
    let has_makefile = repo_path.join("Makefile").exists()
        || repo_path.join("makefile").exists()
        || repo_path.join("GNUmakefile").exists();

    for stage in &pipeline.stages {
        for cmd in &stage.commands {
            // Check npm commands
            if let Some(warning) = check_npm_command(cmd, &npm_scripts, has_package_json, &stage.name) {
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
        let parts: Vec<&str> = cmd_trimmed.split_whitespace().collect();
        if parts.len() < 2 {
            return None;
        }

        let script_name = match parts[1] {
            "test" => Some("test"),
            "start" => Some("start"),
            "build" => Some("build"),
            "run" | "run-script" if parts.len() >= 3 => Some(parts[2]),
            _ => None,
        };

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

            if !npm_scripts.contains(script) {
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
}
