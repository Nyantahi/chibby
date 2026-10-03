//! Draft (local) pipeline generation from detected scripts.

#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::engine::models::{
    Backend, DeploymentConfig, DeploymentMethod, Environment, EnvironmentsConfig, FileConflict,
    HealthCheck, Pipeline, PipelineValidation, PipelineWarning, Stage, WarningSeverity,
};
use std::path::Path;

/// First shared scheme of an Xcode project/workspace bundle, read from
/// `<bundle>/xcshareddata/xcschemes/<Name>.xcscheme`. Returns `None` when there
/// are no *shared* schemes (user-local schemes aren't committed, so CI can't
/// rely on them). Sorted for a deterministic pick.
fn detect_xcode_scheme(bundle_abs: &Path) -> Option<String> {
    let dir = bundle_abs.join("xcshareddata").join("xcschemes");
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|e| {
            e.file_name()
                .to_string_lossy()
                .strip_suffix(".xcscheme")
                .map(str::to_string)
        })
        .collect();
    names.sort();
    names.into_iter().next()
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
    // Relative directory of the first detected script of `st`, or "" when it sits
    // at the repo root. Lets nested single-build-system projects (monorepo /
    // mobile layouts) run their build commands in the right directory.
    let dir_of = |st: ScriptType| -> String {
        scripts
            .iter()
            .find(|s| s.script_type == st)
            .and_then(|s| Path::new(&s.file_name).parent())
            .map(|p| p.to_string_lossy().to_string())
            .filter(|p| !p.is_empty())
            .unwrap_or_default()
    };
    // Prefix `cmd` with `cd <dir> &&` when the build system is nested.
    let in_dir = |dir: &str, cmd: &str| -> String {
        if dir.is_empty() {
            cmd.to_string()
        } else {
            format!("cd {dir} && {cmd}")
        }
    };
    // Use exact file name match so that backend/tauri.conf.json does NOT trigger the
    // standard src-tauri Tauri layout detection.
    let is_tauri = has_file("src-tauri/tauri.conf.json");

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

    // ── Rust / Cargo (root or standard src-tauri layout only) ───────────────────
    // Use has_file to match only root-level Cargo.toml or src-tauri/Cargo.toml.
    // Subdirectory Cargo.toml files (e.g. backend/Cargo.toml) are handled later
    // in the project_folders loop with the correct --manifest-path flag.
    let has_root_cargo = has_file("Cargo.toml") || has_file("src-tauri/Cargo.toml");
    if has_root_cargo {
        if is_tauri {
            // Standard Tauri project — Cargo.toml is in src-tauri/
            stages.push(local_stage(
                "cargo-test",
                vec!["cd src-tauri && cargo test"],
            ));
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
        stages.push(local_stage(
            "nx-build",
            vec!["npx nx run-many --target=build"],
        ));
        stages.push(local_stage(
            "nx-test",
            vec!["npx nx run-many --target=test"],
        ));
    }

    // ── Deno ──────────────────────────────────────────────────────
    if has(ScriptType::Deno) {
        stages.push(local_stage("deno-test", vec!["deno test"]));
    }

    // ── Go ────────────────────────────────────────────────────────
    if has(ScriptType::GoMod) {
        let d = dir_of(ScriptType::GoMod);
        stages.push(local_stage("go-build", vec![&in_dir(&d, "go build ./...")]));
        stages.push(local_stage("go-test", vec![&in_dir(&d, "go test ./...")]));
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
        let has_ruff =
            repo_path.join("ruff.toml").exists() || repo_path.join("pyproject.toml").exists();
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
        scripts
            .iter()
            .any(|s| s.file_name == format!("{}/{}", subdir, file))
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
                stages.push(local_stage(
                    &format!("{}-install", subdir),
                    vec![&format!("cd {} && npm install", subdir)],
                ));

                if folder.npm_scripts.contains("lint") {
                    stages.push(local_stage(
                        &format!("{}-lint", subdir),
                        vec![&format!("cd {} && npm run lint", subdir)],
                    ));
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
                    stages.push(local_stage(
                        &format!("{}-build", subdir),
                        vec![&format!("cd {} && npm run build", subdir)],
                    ));
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
                stages.push(local_stage(
                    &format!("{}-install", subdir),
                    vec![&install_cmd],
                ));

                // Check for tests in subdirectory
                let has_pytest_config =
                    has_subdir_file(subdir, "pytest.ini") || has_subdir_file(subdir, "conftest.py");
                if folder.has_tests || has_pytest_config {
                    stages.push(local_stage(
                        &format!("{}-test", subdir),
                        vec![&format!("cd {} && pytest", subdir)],
                    ));
                }
            }
        }

        // Generate Rust stages for this folder (e.g. backend/Cargo.toml).
        // Always generate these regardless of is_fullstack since a subdir Cargo.toml
        // is always distinct from a root-level one and requires --manifest-path.
        if folder.has_rust {
            let manifest = format!("{}/Cargo.toml", subdir);
            if folder.has_tauri {
                // Tauri project with non-standard layout (e.g. backend/tauri.conf.json).
                // Emit cargo-build, cargo-test, and tauri-build with the correct config path.
                let tauri_conf = format!("{}/tauri.conf.json", subdir);
                stages.push(local_stage(
                    "cargo-build",
                    vec![&format!(
                        "cargo build --release --manifest-path {}",
                        manifest
                    )],
                ));
                stages.push(local_stage(
                    "cargo-test",
                    vec![&format!("cargo test --manifest-path {}", manifest)],
                ));
                if has_script("tauri:build") {
                    stages.push(local_stage("tauri-build", vec!["npm run tauri:build"]));
                } else {
                    stages.push(local_stage(
                        "tauri-build",
                        vec![&format!("npx tauri build -c {}", tauri_conf)],
                    ));
                }
            } else {
                // Plain Rust in a subdirectory — prefix stage names with folder name.
                stages.push(local_stage(
                    &format!("{}-cargo-build", subdir),
                    vec![&format!(
                        "cargo build --release --manifest-path {}",
                        manifest
                    )],
                ));
                stages.push(local_stage(
                    &format!("{}-cargo-test", subdir),
                    vec![&format!("cargo test --manifest-path {}", manifest)],
                ));
            }
        }
    }

    // ── Ruby ──────────────────────────────────────────────────────
    if has(ScriptType::Gemfile) {
        let d = dir_of(ScriptType::Gemfile);
        stages.push(local_stage(
            "bundle-install",
            vec![&in_dir(&d, "bundle install")],
        ));
        if has(ScriptType::Rakefile) {
            stages.push(local_stage(
                "rake-test",
                vec![&in_dir(&d, "bundle exec rake test")],
            ));
        } else {
            stages.push(local_stage("rspec", vec![&in_dir(&d, "bundle exec rspec")]));
        }
    }

    // ── Java / Maven ──────────────────────────────────────────────
    if has(ScriptType::Maven) {
        let d = dir_of(ScriptType::Maven);
        stages.push(local_stage(
            "mvn-build",
            vec![&in_dir(&d, "mvn package -DskipTests")],
        ));
        stages.push(local_stage("mvn-test", vec![&in_dir(&d, "mvn test")]));
    }

    // ── Java / Kotlin / Gradle ────────────────────────────────────
    if has(ScriptType::Gradle) {
        let gradle_cmd = if has_file("gradlew") {
            "./gradlew"
        } else {
            "gradle"
        };
        let gradle = |sub: &str| format!("{gradle_cmd} {sub}");
        if is_android_project(repo_path) {
            stages.push(local_stage("android-lint", vec![&gradle("lintDebug")]));
            stages.push(local_stage(
                "android-test",
                vec![&gradle("testDebugUnitTest")],
            ));
            stages.push(local_stage(
                "android-build",
                vec![&gradle("assembleRelease")],
            ));
        } else {
            stages.push(local_stage("gradle-build", vec![&gradle("build")]));
            stages.push(local_stage("gradle-test", vec![&gradle("test")]));
        }
    }

    // ── Swift / iOS ───────────────────────────────────────────────
    if has(ScriptType::Swift) {
        let d = dir_of(ScriptType::Swift);
        // SPM when the detected Swift manifest is a Package.swift (root or nested);
        // otherwise it's an Xcode project/workspace.
        let is_spm = scripts
            .iter()
            .any(|s| s.script_type == ScriptType::Swift && s.file_name.ends_with("Package.swift"));
        if is_spm {
            // Swift Package Manager: builds headless without a scheme.
            stages.push(local_stage("swift-build", vec![&in_dir(&d, "swift build")]));
            stages.push(local_stage("swift-test", vec![&in_dir(&d, "swift test")]));
        } else {
            // Xcode app project: build-only core. We intentionally do NOT emit a
            // simulator `xcodebuild test` stage — it's macOS-only, slow, and breaks
            // when the hardcoded simulator name is absent on a runner. `xcodebuild
            // build` compiles against the SDK and auto-resolves SPM packages, which
            // is the robust "does it compile" gate. Apply the "Swift iOS App"
            // template for a full scheme-based lint/test/build pipeline.
            let has_swiftlint = repo_path.join(".swiftlint.yml").exists()
                || repo_path.join(".swiftlint.yaml").exists()
                || (!d.is_empty() && repo_path.join(&d).join(".swiftlint.yml").exists());
            if has_swiftlint {
                stages.push(local_stage(
                    "lint",
                    vec![&in_dir(&d, "swiftlint lint --strict")],
                ));
            }
            // Point xcodebuild at the project/workspace BY PATH (quoted for spaces),
            // not a bare `xcodebuild` — which fails "directory does not contain an
            // Xcode project" when nested. A `.xcworkspace` (CocoaPods/SPM) also
            // REQUIRES `-scheme`, so resolve a scheme from the project's shared
            // schemes, falling back to the bundle's base name (the common default).
            let xcode_target = scripts
                .iter()
                .find(|s| s.script_type == ScriptType::Swift && is_xcode_bundle(&s.file_name));
            let build_cmd = match xcode_target {
                Some(s) => {
                    let is_ws = s.file_name.ends_with(".xcworkspace");
                    let flag = if is_ws { "-workspace" } else { "-project" };
                    let base = Path::new(&s.file_name)
                        .file_stem()
                        .map(|x| x.to_string_lossy().to_string());
                    // Real shared scheme if we can find one; else the base name for a
                    // workspace (scheme is mandatory there), else none for a project.
                    let fallback = if is_ws { base } else { None };
                    let scheme = detect_xcode_scheme(Path::new(&s.file_path)).or(fallback);
                    match scheme {
                        Some(sc) => format!(
                            "xcodebuild {flag} \"{}\" -scheme \"{sc}\" build CODE_SIGNING_ALLOWED=NO",
                            s.file_name
                        ),
                        None => format!(
                            "xcodebuild {flag} \"{}\" build CODE_SIGNING_ALLOWED=NO",
                            s.file_name
                        ),
                    }
                }
                None => "xcodebuild build CODE_SIGNING_ALLOWED=NO".to_string(),
            };
            stages.push(local_stage("xcode-build", vec![&build_cmd]));
        }
    }

    // ── .NET ──────────────────────────────────────────────────────
    if has(ScriptType::DotNet) {
        let d = dir_of(ScriptType::DotNet);
        stages.push(local_stage(
            "dotnet-build",
            vec![&in_dir(&d, "dotnet build")],
        ));
        stages.push(local_stage("dotnet-test", vec![&in_dir(&d, "dotnet test")]));
    }

    // ── PHP / Composer ────────────────────────────────────────────
    if has(ScriptType::Composer) {
        let d = dir_of(ScriptType::Composer);
        stages.push(local_stage(
            "composer-install",
            vec![&in_dir(&d, "composer install")],
        ));
        stages.push(local_stage(
            "phpunit",
            vec![&in_dir(&d, "./vendor/bin/phpunit")],
        ));
    }

    // ── C / C++ / CMake ───────────────────────────────────────────
    if has(ScriptType::CMake) {
        let d = dir_of(ScriptType::CMake);
        stages.push(local_stage(
            "cmake-build",
            vec![
                &in_dir(&d, "cmake -B build"),
                &in_dir(&d, "cmake --build build"),
            ],
        ));
        stages.push(local_stage(
            "cmake-test",
            vec![&in_dir(&d, "ctest --test-dir build")],
        ));
    }

    // ── Meson ─────────────────────────────────────────────────────
    if has(ScriptType::Meson) {
        let d = dir_of(ScriptType::Meson);
        stages.push(local_stage(
            "meson-build",
            vec![
                &in_dir(&d, "meson setup build"),
                &in_dir(&d, "meson compile -C build"),
            ],
        ));
        stages.push(local_stage(
            "meson-test",
            vec![&in_dir(&d, "meson test -C build")],
        ));
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
            ..Default::default()
        });
    }

    // ── Fallback ──────────────────────────────────────────────────
    if stages.is_empty() {
        stages.push(local_stage(
            "build",
            vec!["echo 'Add your build commands here'"],
        ));
    }

    // ── Security gates ────────────────────────────────────────────
    // Append one stage per enabled gate when .chibby/gates.toml exists.
    // Stages call back into the chibby CLI so allowlists / thresholds /
    // baselines defined in gates.toml are respected at run time.
    append_security_gate_stages(repo_path, &mut stages);

    Pipeline {
        name: format!("{} Pipeline", repo_name),
        on_health_failure: None,
        stages,
    }
}

/// Append a `security-*` stage per enabled gate when `.chibby/gates.toml`
/// exists. Off-mode gates are skipped. Runs `chibby scan <gate>` (which loads
/// the same gates.toml the GUI's Quality tab uses).
fn append_security_gate_stages(repo_path: &Path, stages: &mut Vec<Stage>) {
    use crate::engine::gates;
    use crate::engine::models::GateMode;

    let gates_path = repo_path.join(".chibby").join("gates.toml");
    if !gates_path.exists() {
        return;
    }
    let config = match gates::load_gates_config(repo_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let push = |stages: &mut Vec<Stage>, name: &str, sub: &str| {
        stages.push(local_stage(
            &format!("security-{}", name),
            vec![&format!("chibby scan {}", sub)],
        ));
    };

    // Container and IaC scans only make sense when the repo actually has those
    // artifacts. Adding them to, say, a pure iOS app just pads the pipeline with
    // gates that scan nothing. Scope them to the relevant files (shallow walk).
    let has_container = exists_within(repo_path, NESTED_SCAN_DEPTH, &|n: &str| {
        n == "Dockerfile" || is_docker_compose_file(n)
    });
    let has_iac = exists_within(repo_path, NESTED_SCAN_DEPTH, &|n: &str| {
        n.ends_with(".tf") || n.ends_with(".tf.json") || n == "Chart.yaml"
    });

    // Universal gates: apply to any repo.
    if config.secret_scanning != GateMode::Off {
        push(stages, "secrets", "secrets");
    }
    if config.dependency_scanning != GateMode::Off {
        push(stages, "deps", "deps");
    }
    if config.sast != GateMode::Off {
        push(stages, "sast", "sast");
    }
    // File-scoped gates: only when the artifact they scan is present.
    if config.container_scan != GateMode::Off && has_container {
        push(stages, "container", "container");
    }
    if config.iac_scan != GateMode::Off && has_iac {
        push(stages, "iac", "iac");
    }
    if config.license_check != GateMode::Off {
        push(stages, "license", "license");
    }
    if config.commit_lint != GateMode::Off {
        push(stages, "commit-lint", "commits");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_generate_draft_pipeline_ios_build_only() {
        // Xcode app project: build-only core, NO simulator test stage.
        let tmp = tempfile::tempdir().unwrap();
        let scripts = vec![DetectedScript {
            file_name: "MyApp.xcodeproj".to_string(),
            file_path: "/test/MyApp.xcodeproj".to_string(),
            script_type: ScriptType::Swift,
        }];
        let pipeline = generate_draft_pipeline("MyApp", &scripts, tmp.path());
        let names: Vec<_> = pipeline.stages.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"xcode-build"));
        // No simulator test stage (macOS-only / fragile).
        let cmds: Vec<_> = pipeline.stages.iter().flat_map(|s| &s.commands).collect();
        assert!(!cmds.iter().any(|c| c.contains("xcodebuild test")));
        assert!(!names.contains(&"test"));
    }

    #[test]
    fn test_security_gates_scoped_to_relevant_files() {
        use std::fs;
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".chibby")).unwrap();
        fs::write(
            tmp.path().join(".chibby/gates.toml"),
            "secret_scanning = \"block\"\ncontainer_scan = \"block\"\niac_scan = \"block\"\n",
        )
        .unwrap();

        // No Dockerfile / IaC present: container + iac gates are skipped.
        let pipeline = generate_draft_pipeline("x", &[], tmp.path());
        let names: Vec<_> = pipeline.stages.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"security-secrets"));
        assert!(!names.contains(&"security-container"));
        assert!(!names.contains(&"security-iac"));

        // Add a Dockerfile: the container gate now applies.
        fs::write(tmp.path().join("Dockerfile"), "FROM scratch").unwrap();
        let pipeline2 = generate_draft_pipeline("x", &[], tmp.path());
        let names2: Vec<_> = pipeline2.stages.iter().map(|s| s.name.as_str()).collect();
        assert!(names2.contains(&"security-container"));
        assert!(!names2.contains(&"security-iac"));
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
        let stage_commands: Vec<_> = pipeline.stages.iter().flat_map(|s| &s.commands).collect();
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
        let stage_commands: Vec<_> = pipeline.stages.iter().flat_map(|s| &s.commands).collect();
        assert!(stage_commands.iter().any(|c| c.contains("cargo")));
    }

    #[test]
    fn test_generate_draft_pipeline_swift_spm() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("Package.swift"), "").unwrap();
        let scripts = vec![DetectedScript {
            file_name: "Package.swift".to_string(),
            file_path: tmp
                .path()
                .join("Package.swift")
                .to_string_lossy()
                .to_string(),
            script_type: ScriptType::Swift,
        }];

        let pipeline = generate_draft_pipeline("swift-pkg", &scripts, tmp.path());
        let cmds: Vec<_> = pipeline.stages.iter().flat_map(|s| &s.commands).collect();
        assert!(cmds.iter().any(|c| c.contains("swift build")));
        assert!(cmds.iter().any(|c| c.contains("swift test")));
    }

    #[test]
    fn test_generate_draft_pipeline_swift_xcode() {
        // No Package.swift on disk -> Xcode app path.
        let tmp = tempfile::tempdir().unwrap();
        let scripts = vec![DetectedScript {
            file_name: "MyApp.xcodeproj".to_string(),
            file_path: "/test/MyApp.xcodeproj".to_string(),
            script_type: ScriptType::Swift,
        }];

        let pipeline = generate_draft_pipeline("ios-app", &scripts, tmp.path());
        let cmds: Vec<_> = pipeline.stages.iter().flat_map(|s| &s.commands).collect();
        assert!(cmds.iter().any(|c| c.contains("xcodebuild")));
    }

    #[test]
    fn test_generate_draft_pipeline_nested_xcodeproj_uses_project_path() {
        // Nested .xcodeproj: xcodebuild must be pointed at it by path so it works
        // from the repo root (the "directory does not contain an Xcode project" fix).
        let tmp = tempfile::tempdir().unwrap();
        let scripts = vec![DetectedScript {
            file_name: "KibokoKids/KibokoKids.xcodeproj".to_string(),
            file_path: "/test/KibokoKids/KibokoKids.xcodeproj".to_string(),
            script_type: ScriptType::Swift,
        }];
        let pipeline = generate_draft_pipeline("KibokoKids", &scripts, tmp.path());
        let build = pipeline
            .stages
            .iter()
            .find(|s| s.name == "xcode-build")
            .unwrap();
        assert_eq!(
            build.commands[0],
            "xcodebuild -project \"KibokoKids/KibokoKids.xcodeproj\" build CODE_SIGNING_ALLOWED=NO"
        );
    }

    #[test]
    fn test_generate_draft_pipeline_xcworkspace_uses_workspace_flag() {
        let tmp = tempfile::tempdir().unwrap();
        let scripts = vec![DetectedScript {
            file_name: "App/App.xcworkspace".to_string(),
            file_path: "/test/App/App.xcworkspace".to_string(),
            script_type: ScriptType::Swift,
        }];
        let pipeline = generate_draft_pipeline("App", &scripts, tmp.path());
        let build = pipeline
            .stages
            .iter()
            .find(|s| s.name == "xcode-build")
            .unwrap();
        assert!(build.commands[0].contains("-workspace \"App/App.xcworkspace\""));
    }

    #[test]
    fn test_xcode_build_uses_detected_shared_scheme() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().join("App.xcworkspace");
        std::fs::create_dir_all(ws.join("xcshareddata/xcschemes")).unwrap();
        std::fs::write(
            ws.join("xcshareddata/xcschemes/MyApp.xcscheme"),
            "<Scheme/>",
        )
        .unwrap();
        let scripts = vec![DetectedScript {
            file_name: "App.xcworkspace".to_string(),
            file_path: ws.to_string_lossy().to_string(),
            script_type: ScriptType::Swift,
        }];
        let pipeline = generate_draft_pipeline("App", &scripts, tmp.path());
        let build = pipeline
            .stages
            .iter()
            .find(|s| s.name == "xcode-build")
            .unwrap();
        assert_eq!(
            build.commands[0],
            "xcodebuild -workspace \"App.xcworkspace\" -scheme \"MyApp\" build CODE_SIGNING_ALLOWED=NO"
        );
    }

    #[test]
    fn test_xcode_workspace_falls_back_to_basename_scheme() {
        // Workspace with no shared schemes -> scheme defaults to the workspace name
        // (the common convention). Mirrors the KibokoKids.xcworkspace case.
        let tmp = tempfile::tempdir().unwrap();
        let scripts = vec![DetectedScript {
            file_name: "KibokoKids.xcworkspace".to_string(),
            file_path: tmp
                .path()
                .join("KibokoKids.xcworkspace")
                .to_string_lossy()
                .to_string(),
            script_type: ScriptType::Swift,
        }];
        let pipeline = generate_draft_pipeline("KibokoKids", &scripts, tmp.path());
        let build = pipeline
            .stages
            .iter()
            .find(|s| s.name == "xcode-build")
            .unwrap();
        assert_eq!(
            build.commands[0],
            "xcodebuild -workspace \"KibokoKids.xcworkspace\" -scheme \"KibokoKids\" build CODE_SIGNING_ALLOWED=NO"
        );
    }

    #[test]
    fn test_generate_draft_pipeline_android_gradle() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("app")).unwrap();
        std::fs::write(tmp.path().join("app/build.gradle"), "").unwrap();
        let scripts = vec![DetectedScript {
            file_name: "build.gradle".to_string(),
            file_path: "/test/build.gradle".to_string(),
            script_type: ScriptType::Gradle,
        }];

        let pipeline = generate_draft_pipeline("android-app", &scripts, tmp.path());
        let cmds: Vec<_> = pipeline.stages.iter().flat_map(|s| &s.commands).collect();
        // Android-aware stages, not the generic gradle build/test.
        assert!(cmds.iter().any(|c| c.contains("assembleRelease")));
        assert!(cmds.iter().any(|c| c.contains("testDebugUnitTest")));
    }

    #[test]
    fn test_generate_draft_pipeline_nested_go_cds_into_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let scripts = vec![DetectedScript {
            file_name: "services/api/go.mod".to_string(),
            file_path: "/test/services/api/go.mod".to_string(),
            script_type: ScriptType::GoMod,
        }];
        let pipeline = generate_draft_pipeline("svc", &scripts, tmp.path());
        let cmds: Vec<_> = pipeline.stages.iter().flat_map(|s| &s.commands).collect();
        assert!(cmds
            .iter()
            .any(|c| c.as_str() == "cd services/api && go build ./..."));
    }

    #[test]
    fn test_generate_draft_pipeline_root_go_no_cd() {
        let tmp = tempfile::tempdir().unwrap();
        let scripts = vec![DetectedScript {
            file_name: "go.mod".to_string(),
            file_path: "/test/go.mod".to_string(),
            script_type: ScriptType::GoMod,
        }];
        let pipeline = generate_draft_pipeline("svc", &scripts, tmp.path());
        let cmds: Vec<_> = pipeline.stages.iter().flat_map(|s| &s.commands).collect();
        assert!(cmds.iter().any(|c| c.as_str() == "go build ./..."));
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
                format!(
                    r#"{{"name": "{}", "scripts": {{"lint": "eslint", "test": "vitest"}}}}"#,
                    name
                ),
            )
            .unwrap();
        }

        // Create docker-compose.yml
        std::fs::write(temp.path().join("docker-compose.yml"), "version: '3'").unwrap();

        let scripts = detect_scripts(temp.path());
        let pipeline = generate_draft_pipeline("bituntu", &scripts, temp.path());

        // Should have stages for ALL folders (frontend, backend, admin)
        let stage_names: Vec<_> = pipeline.stages.iter().map(|s| s.name.as_str()).collect();

        assert!(
            stage_names.contains(&"frontend-install"),
            "Missing frontend-install: {:?}",
            stage_names
        );
        assert!(
            stage_names.contains(&"frontend-lint"),
            "Missing frontend-lint: {:?}",
            stage_names
        );
        assert!(
            stage_names.contains(&"frontend-test"),
            "Missing frontend-test: {:?}",
            stage_names
        );

        assert!(
            stage_names.contains(&"backend-install"),
            "Missing backend-install: {:?}",
            stage_names
        );
        assert!(
            stage_names.contains(&"backend-lint"),
            "Missing backend-lint: {:?}",
            stage_names
        );
        assert!(
            stage_names.contains(&"backend-test"),
            "Missing backend-test: {:?}",
            stage_names
        );

        assert!(
            stage_names.contains(&"admin-install"),
            "Missing admin-install: {:?}",
            stage_names
        );
        assert!(
            stage_names.contains(&"admin-lint"),
            "Missing admin-lint: {:?}",
            stage_names
        );
        assert!(
            stage_names.contains(&"admin-test"),
            "Missing admin-test: {:?}",
            stage_names
        );

        // Docker-deploy should NOT be in CI pipeline (it goes in deploy.toml)
        assert!(
            !stage_names.contains(&"docker-deploy"),
            "docker-deploy should not be in CI pipeline for fullstack"
        );
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
        )
        .unwrap();

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
                    assert!(
                        cmd.starts_with("cd frontend && "),
                        "Command missing 'cd frontend &&' prefix: {}",
                        cmd
                    );
                } else if stage.name.starts_with("backend-") {
                    assert!(
                        cmd.starts_with("cd backend &&"),
                        "Command missing 'cd backend &&' prefix: {}",
                        cmd
                    );
                }
            }
        }
    }
}
