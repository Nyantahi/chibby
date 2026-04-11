use crate::engine::detector;
use crate::engine::models::{Pipeline, PipelineValidation, ProjectRecommendations, Stage, DeploymentMethod, DeploymentConfig};
use crate::engine::pipeline;
use crate::engine::recommendations;
use serde::Serialize;
use std::path::Path;

/// Info about a detected script returned to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct DetectedScriptInfo {
    pub file_name: String,
    pub file_path: String,
    pub script_type: String,
}

/// Info about a CI workflow job step.
#[derive(Debug, Clone, Serialize)]
pub struct WorkflowStepInfo {
    pub name: Option<String>,
    pub run: String,
}

/// Info about a CI workflow job.
#[derive(Debug, Clone, Serialize)]
pub struct WorkflowJobInfo {
    pub name: String,
    pub steps: Vec<WorkflowStepInfo>,
}

/// Info about a CI workflow file.
#[derive(Debug, Clone, Serialize)]
pub struct WorkflowInfo {
    pub name: String,
    pub file_name: String,
    pub jobs: Vec<WorkflowJobInfo>,
}

/// Detect scripts in a repository.
#[tauri::command]
pub fn detect_scripts(repo_path: String) -> Result<Vec<DetectedScriptInfo>, String> {
    let scripts = detector::detect_scripts(Path::new(&repo_path));
    let infos: Vec<DetectedScriptInfo> = scripts
        .into_iter()
        .map(|s| DetectedScriptInfo {
            file_name: s.file_name,
            file_path: s.file_path,
            script_type: format!("{:?}", s.script_type),
        })
        .collect();
    Ok(infos)
}

/// Generate a draft pipeline from detected scripts.
///
/// For fullstack Docker projects (multiple folders + docker-compose), this also
/// generates a separate deploy.toml with Docker deployment stages.
#[tauri::command]
pub fn generate_pipeline(repo_path: String, repo_name: String) -> Result<Pipeline, String> {
    let path = Path::new(&repo_path);
    let scripts = detector::detect_scripts(path);
    let draft = detector::generate_draft_pipeline(&repo_name, &scripts, path);

    // For fullstack Docker projects, also generate and save a deploy pipeline
    if detector::is_fullstack_docker_project(path) {
        if let Some(deploy_pipeline) = detector::generate_deploy_pipeline(&repo_name, &scripts, path) {
            // Save the deploy pipeline to deploy.toml
            if let Err(e) = pipeline::save_pipeline_by_name(path, "deploy", &deploy_pipeline) {
                log::warn!("Failed to save deploy pipeline: {}", e);
            }
        }
    }

    Ok(draft)
}

/// Save a pipeline to .chibby/pipeline.toml.
#[tauri::command]
pub fn save_pipeline(repo_path: String, p: Pipeline) -> Result<(), String> {
    pipeline::save_pipeline(Path::new(&repo_path), &p).map_err(|e| e.to_string())
}

/// Load a pipeline from .chibby/pipeline.toml.
#[tauri::command]
pub fn load_pipeline(repo_path: String) -> Result<Pipeline, String> {
    pipeline::load_pipeline(Path::new(&repo_path)).map_err(|e| e.to_string())
}

/// List all available pipelines in .chibby/ directory.
#[tauri::command]
pub fn list_pipelines(repo_path: String) -> Result<Vec<String>, String> {
    Ok(pipeline::list_pipelines(Path::new(&repo_path)))
}

/// Load a specific pipeline by name (file stem).
#[tauri::command]
pub fn load_pipeline_by_name(repo_path: String, name: String) -> Result<Pipeline, String> {
    pipeline::load_pipeline_by_name(Path::new(&repo_path), &name).map_err(|e| e.to_string())
}

/// Save a pipeline to a specific file by name.
#[tauri::command]
pub fn save_pipeline_by_name(repo_path: String, name: String, p: Pipeline) -> Result<(), String> {
    pipeline::save_pipeline_by_name(Path::new(&repo_path), &name, &p).map_err(|e| e.to_string())
}

/// Validate a pipeline against the actual project configuration.
///
/// Checks for issues like missing npm scripts, Makefile targets, or shell scripts.
#[tauri::command]
pub fn validate_pipeline(repo_path: String) -> Result<PipelineValidation, String> {
    let path = Path::new(&repo_path);
    let p = pipeline::load_pipeline(path).map_err(|e| e.to_string())?;
    let validation = detector::validate_pipeline(&p, path);
    Ok(validation)
}

/// Parse GitHub Actions workflows from a repository.
///
/// Returns structured info about all workflows found in .github/workflows/.
#[tauri::command]
pub fn get_github_workflows(repo_path: String) -> Result<Vec<WorkflowInfo>, String> {
    let workflows = detector::parse_github_workflows(Path::new(&repo_path));
    let infos: Vec<WorkflowInfo> = workflows
        .into_iter()
        .map(|w| {
            let file_name = Path::new(&w.file_path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| w.file_path.clone());
            WorkflowInfo {
                name: w.name,
                file_name,
                jobs: w
                    .jobs
                    .into_iter()
                    .map(|j| WorkflowJobInfo {
                        name: j.name,
                        steps: j
                            .steps
                            .into_iter()
                            .map(|s| WorkflowStepInfo {
                                name: s.name,
                                run: s.run,
                            })
                            .collect(),
                    })
                    .collect(),
            }
        })
        .collect();
    Ok(infos)
}

/// Convert GitHub workflows to Chibby pipeline stages.
///
/// Useful for importing existing CI configurations.
#[tauri::command]
pub fn workflows_to_pipeline_stages(repo_path: String) -> Result<Vec<Stage>, String> {
    let workflows = detector::parse_github_workflows(Path::new(&repo_path));
    let stages = detector::workflows_to_stages(&workflows);
    Ok(stages)
}

/// Analyze a repository and get CI/CD file recommendations.
///
/// Returns recommendations for missing configuration files,
/// categorized by priority (Critical, High, Medium, Low).
#[tauri::command]
pub fn get_recommendations(repo_path: String) -> Result<ProjectRecommendations, String> {
    let path = Path::new(&repo_path);
    if !path.exists() {
        return Err(format!("Repository path does not exist: {}", repo_path));
    }
    Ok(recommendations::analyze_repository(path))
}

/// Detect the most likely deployment method for a repository.
///
/// Analyzes project type, existing config files (fly.toml, netlify.toml, etc.),
/// and GitHub Actions workflows to determine the best deployment method.
#[tauri::command]
pub fn detect_deployment_method(repo_path: String) -> Result<DeploymentMethod, String> {
    let path = Path::new(&repo_path);
    if !path.exists() {
        return Err(format!("Repository path does not exist: {}", repo_path));
    }
    Ok(detector::detect_deployment_method(path))
}

/// Get suggested deployment methods for a repository.
///
/// Returns a list of applicable deployment methods based on the project type.
#[tauri::command]
pub fn get_suggested_deploy_methods(repo_path: String) -> Result<Vec<DeploymentMethod>, String> {
    let path = Path::new(&repo_path);
    if !path.exists() {
        return Err(format!("Repository path does not exist: {}", repo_path));
    }
    Ok(detector::get_suggested_deploy_methods(path))
}

/// Get the detected project type for a repository.
///
/// Returns a string describing the project type (Rust, Node, Python, etc.).
#[tauri::command]
pub fn detect_project_type(repo_path: String) -> Result<String, String> {
    let path = Path::new(&repo_path);
    if !path.exists() {
        return Err(format!("Repository path does not exist: {}", repo_path));
    }
    let project_type = detector::detect_project_type(path);
    Ok(format!("{:?}", project_type))
}

/// Generate a CI pipeline and optionally a CD pipeline based on deployment config.
///
/// If deploy_config is provided and method is not Skip, generates and saves a deploy.toml.
/// Returns the main CI pipeline.
#[tauri::command]
pub fn generate_pipeline_with_deploy(
    repo_path: String,
    repo_name: String,
    deploy_config: Option<DeploymentConfig>,
) -> Result<Pipeline, String> {
    let path = Path::new(&repo_path);
    let scripts = detector::detect_scripts(path);

    // Generate the main CI pipeline
    let ci_pipeline = detector::generate_draft_pipeline(&repo_name, &scripts, path);

    // Generate and save deploy pipeline if requested
    if let Some(ref config) = deploy_config {
        if config.method != DeploymentMethod::Skip {
            if let Some(deploy_pipeline) = detector::generate_deployment_pipeline(&repo_name, config, path) {
                if let Err(e) = pipeline::save_pipeline_by_name(path, "deploy", &deploy_pipeline) {
                    log::warn!("Failed to save deploy pipeline: {}", e);
                }
            }

            // Auto-create environments.toml if it doesn't exist
            if !pipeline::has_environments(path) {
                if let Some(env_config) = detector::generate_default_environments(config) {
                    if let Err(e) = pipeline::save_environments(path, &env_config) {
                        log::warn!("Failed to save environments: {}", e);
                    }
                }
            }
        }
    } else {
        // For fullstack Docker projects, auto-generate deploy pipeline if no config provided
        if detector::is_fullstack_docker_project(path) {
            if let Some(deploy_pipeline) = detector::generate_deploy_pipeline(&repo_name, &scripts, path) {
                if let Err(e) = pipeline::save_pipeline_by_name(path, "deploy", &deploy_pipeline) {
                    log::warn!("Failed to save deploy pipeline: {}", e);
                }
            }
        }
    }

    Ok(ci_pipeline)
}
