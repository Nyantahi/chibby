use crate::engine::models::Project;
use crate::engine::persistence;
use serde::Serialize;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Serialize)]
pub struct ProjectInfo {
    pub project: Project,
    pub has_pipeline: bool,
}

/// Git repository information.
#[derive(Debug, Clone, Serialize)]
pub struct GitInfo {
    /// Current branch name (e.g., "main", "feature/foo").
    pub branch: Option<String>,
    /// Short commit hash of HEAD.
    pub commit: Option<String>,
    /// Whether there are uncommitted changes.
    pub is_dirty: bool,
    /// Number of commits ahead of remote (if tracking branch exists).
    pub ahead: Option<u32>,
    /// Number of commits behind remote (if tracking branch exists).
    pub behind: Option<u32>,
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
    let path = Path::new(&repo_path);

    // Check if it's a git repository
    if !path.join(".git").exists() {
        return Ok(GitInfo {
            branch: None,
            commit: None,
            is_dirty: false,
            ahead: None,
            behind: None,
        });
    }

    let branch = get_current_branch(path);
    let commit = get_head_commit(path);
    let is_dirty = check_is_dirty(path);
    let (ahead, behind) = get_ahead_behind(path);

    Ok(GitInfo {
        branch,
        commit,
        is_dirty,
        ahead,
        behind,
    })
}

/// Get the current branch name.
fn get_current_branch(repo_path: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo_path)
        .output()
        .ok()?;

    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if branch == "HEAD" {
            // Detached HEAD state - try to get more info
            None
        } else {
            Some(branch)
        }
    } else {
        None
    }
}

/// Get the short commit hash of HEAD.
fn get_head_commit(repo_path: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(repo_path)
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

/// Check if there are uncommitted changes.
fn check_is_dirty(repo_path: &Path) -> bool {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_path)
        .output();

    match output {
        Ok(out) => !out.stdout.is_empty(),
        Err(_) => false,
    }
}

/// Get ahead/behind counts relative to upstream.
fn get_ahead_behind(repo_path: &Path) -> (Option<u32>, Option<u32>) {
    let output = Command::new("git")
        .args(["rev-list", "--left-right", "--count", "HEAD...@{upstream}"])
        .current_dir(repo_path)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            let parts: Vec<&str> = text.trim().split('\t').collect();
            if parts.len() == 2 {
                let ahead = parts[0].parse().ok();
                let behind = parts[1].parse().ok();
                (ahead, behind)
            } else {
                (None, None)
            }
        }
        _ => (None, None),
    }
}
