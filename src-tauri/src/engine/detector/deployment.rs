//! Deployment method detection and deploy pipeline generation.

#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::engine::models::{
    Backend, DeploymentConfig, DeploymentMethod, Environment, EnvironmentsConfig, FileConflict,
    HealthCheck, Pipeline, PipelineValidation, PipelineWarning, Stage, WarningSeverity,
};
use std::path::Path;

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
            let is_ssh_stage = stage
                .commands
                .iter()
                .any(|cmd| cmd.contains("ssh ") || cmd.contains("rsync") || cmd.contains("scp "));

            stages.push(Stage {
                name: stage.name,
                commands: stage.commands,
                backend: if is_ssh_stage {
                    Backend::Ssh
                } else {
                    Backend::Local
                },
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
        if has_file("index.html") || has_file("public/index.html") || has_file("static/index.html")
        {
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
pub(crate) fn has_deploy_workflow(repo_path: &Path) -> bool {
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
        ProjectType::Tauri => vec![DeploymentMethod::GithubRelease, DeploymentMethod::Skip],
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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
}
