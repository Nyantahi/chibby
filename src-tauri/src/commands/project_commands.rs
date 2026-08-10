use crate::engine::git::{self, GitInfo};
use crate::engine::models::Project;
use crate::engine::persistence;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct ProjectInfo {
    pub project: Project,
    pub has_pipeline: bool,
}

/// List all tracked projects.
#[tauri::command]
pub fn list_projects() -> Result<Vec<ProjectInfo>, String> {
    let projects = persistence::load_projects().map_err(|e| e.to_string())?;
    let infos: Vec<ProjectInfo> = projects
        .into_iter()
        .map(|p| {
            let has_pipeline = crate::engine::pipeline::has_pipeline(Path::new(&p.path));
            ProjectInfo {
                project: p,
                has_pipeline,
            }
        })
        .collect();
    Ok(infos)
}

/// Add a project by local path.
#[tauri::command]
pub fn add_project(name: String, path: String) -> Result<Project, String> {
    // Validate the path exists.
    if !Path::new(&path).is_dir() {
        return Err(format!("Directory does not exist: {}", path));
    }

    let project = Project::new(&name, &path);
    persistence::add_project(project.clone()).map_err(|e| e.to_string())?;
    Ok(project)
}

/// Remove a project by ID.
#[tauri::command]
pub fn remove_project(id: String) -> Result<(), String> {
    persistence::remove_project(&id).map_err(|e| e.to_string())
}

/// Get Git information for a repository.
#[tauri::command]
pub fn get_git_info(repo_path: String) -> Result<GitInfo, String> {
    Ok(git::info(Path::new(&repo_path)))
}
