//! Pipeline validation and file-conflict detection.

#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::engine::models::{
    Backend, DeploymentConfig, DeploymentMethod, Environment, EnvironmentsConfig, FileConflict,
    HealthCheck, Pipeline, PipelineValidation, PipelineWarning, Stage, WarningSeverity,
};
use std::collections::HashSet;
use std::path::Path;

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
    let mut subdir_package_jsons: std::collections::HashMap<String, HashSet<String>> =
        std::collections::HashMap::new();
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
            if let Some(warning) = check_make_command(cmd, &make_targets, has_makefile, &stage.name)
            {
                warnings.push(warning);
            }

            // Check shell script exists
            if let Some(warning) = check_shell_script(cmd, repo_path, &stage.name) {
                warnings.push(warning);
            }
        }
    }

    let has_errors = warnings
        .iter()
        .any(|w| w.severity == WarningSeverity::Error);

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

    // Match against actual on-disk filenames; Path::exists() is case-insensitive on APFS/NTFS
    // and would falsely report Makefile + makefile as separate files when only one exists.
    let entries = list_dir_filenames(repo_path);

    // Define groups of files that conflict with each other
    let conflict_groups: &[(&str, &[&str], Option<&str>)] = &[
        // Makefiles - on case-sensitive systems, both can exist but cause confusion
        (
            "Makefile",
            &["Makefile", "makefile", "GNUmakefile"],
            Some("GNUmakefile > Makefile > makefile"),
        ),
        // Docker Compose - multiple variants
        (
            "Docker Compose",
            &[
                "docker-compose.yml",
                "docker-compose.yaml",
                "compose.yml",
                "compose.yaml",
            ],
            Some("compose.yml is the modern default"),
        ),
        // Taskfile
        ("Taskfile", &["Taskfile.yml", "Taskfile.yaml"], None),
        // Deno config
        ("Deno Config", &["deno.json", "deno.jsonc"], None),
        // Python project config (multiple ways to define a project)
        (
            "Python Project",
            &["pyproject.toml", "setup.py", "setup.cfg"],
            Some("pyproject.toml is the modern standard"),
        ),
        // Package lock files - indicates mixed package manager usage
        (
            "Package Lock",
            &[
                "package-lock.json",
                "yarn.lock",
                "pnpm-lock.yaml",
                "bun.lockb",
            ],
            Some("Use only one package manager"),
        ),
        // Gradle
        ("Gradle Build", &["build.gradle", "build.gradle.kts"], None),
    ];

    for (category, file_names, note) in conflict_groups {
        let existing: Vec<String> = file_names
            .iter()
            .filter(|name| entries.contains(**name))
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
            message:
                "Many .env files detected. Ensure you know which ones are loaded and in what order."
                    .to_string(),
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
            if name == ".env"
                || (name.starts_with(".env.")
                    && !name.ends_with(".example")
                    && !name.ends_with(".sample")
                    && !name.ends_with(".template"))
            {
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
        return check_npm_command_fullstack_soft(
            cmd_trimmed,
            all_npm_scripts,
            stage_name,
            subdir_scripts,
        );
    }

    check_npm_command(
        cmd_trimmed,
        all_npm_scripts,
        has_any_package_json,
        stage_name,
    )
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
    if cmd_trimmed.starts_with("npm ")
        || cmd_trimmed.starts_with("yarn ")
        || cmd_trimmed.starts_with("pnpm ")
    {
        let script_name = extract_npm_script_name(cmd_trimmed);

        if let Some(script) = script_name {
            // Check if script exists in any subdir
            let found_in_subdir = subdir_scripts
                .iter()
                .find(|(_, scripts)| scripts.contains(&script));

            if let Some((subdir, _)) = found_in_subdir {
                // Script found in a subdirectory - suggest adding cd prefix
                return Some(PipelineWarning {
                    stage_name: stage_name.to_string(),
                    command: cmd.to_string(),
                    message: format!(
                        "Script '{}' found in {}/package.json, not root",
                        script, subdir
                    ),
                    suggestion: Some(format!(
                        "Add 'cd {} && ' prefix or move script to root package.json",
                        subdir
                    )),
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
                    suggestion: Some(format!(
                        "Add the script to {}/package.json or remove this stage",
                        subdir_hint
                    )),
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
    if cmd_trimmed.starts_with("npm ")
        || cmd_trimmed.starts_with("yarn ")
        || cmd_trimmed.starts_with("pnpm ")
    {
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
    fn test_detect_file_conflicts_no_case_duplicate_on_apfs() {
        // Regression: a single Makefile must not be reported as a conflict
        // just because the filesystem is case-insensitive.
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("Makefile"), "build:\n\techo building").unwrap();

        let conflicts = detect_file_conflicts(temp.path());
        assert!(
            !conflicts.iter().any(|c| c.category == "Makefile"),
            "expected no Makefile conflict for a single file, got {:?}",
            conflicts
        );
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
