use crate::engine::detector;
use crate::engine::models::{Pipeline, PipelineValidation, ProjectRecommendations, Stage};
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
#[tauri::command]
pub fn generate_pipeline(repo_path: String, repo_name: String) -> Result<Pipeline, String> {
    let scripts = detector::detect_scripts(Path::new(&repo_path));
    let draft = detector::generate_draft_pipeline(&repo_name, &scripts, Path::new(&repo_path));
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
