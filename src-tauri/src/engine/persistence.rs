use crate::engine::models::{DeploymentRecord, PipelineRun, Project, RunStatus};
use anyhow::{Context, Result};
use std::path::PathBuf;

/// Get the Chibby application data directory.
///
/// - macOS: ~/Library/Application Support/Chibby/
/// - Linux: ~/.local/share/chibby/
/// - Windows: %APPDATA%\Chibby\
pub fn data_dir() -> Result<PathBuf> {
    let base = dirs::data_dir().context("Could not determine app data directory")?;

    #[cfg(target_os = "macos")]
    let dir = base.join("Chibby");
    #[cfg(target_os = "linux")]
    let dir = base.join("chibby");
    #[cfg(target_os = "windows")]
    let dir = base.join("Chibby");
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let dir = base.join("chibby");

    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create data directory: {}", dir.display()))?;
    Ok(dir)
}

/// Path to the projects index file.
fn projects_file() -> Result<PathBuf> {
    Ok(data_dir()?.join("projects.json"))
}

/// Path to the runs directory.
fn runs_dir() -> Result<PathBuf> {
    let dir = data_dir()?.join("runs");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

// ---------------------------------------------------------------------------
// Project persistence
// ---------------------------------------------------------------------------

/// Load all tracked projects.
pub fn load_projects() -> Result<Vec<Project>> {
    let path = projects_file()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(&path)?;
    let projects: Vec<Project> = serde_json::from_str(&content)?;
    Ok(projects)
}

/// Save the full projects list.
pub fn save_projects(projects: &[Project]) -> Result<()> {
    let path = projects_file()?;
    let content = serde_json::to_string_pretty(projects)?;
    std::fs::write(&path, content)?;
    Ok(())
}

/// Add a project to the index.
pub fn add_project(project: Project) -> Result<()> {
    let mut projects = load_projects()?;
    // Avoid duplicates by path.
    if projects.iter().any(|p| p.path == project.path) {
        return Ok(());
    }
    projects.push(project);
    save_projects(&projects)
}

/// Remove a project by id.
pub fn remove_project(id: &str) -> Result<()> {
    let mut projects = load_projects()?;
    projects.retain(|p| p.id != id);
    save_projects(&projects)
}

// ---------------------------------------------------------------------------
// Run history persistence
// ---------------------------------------------------------------------------

/// Save a pipeline run record.
pub fn save_run(run: &PipelineRun) -> Result<()> {
    let dir = runs_dir()?;
    let file = dir.join(format!("{}.json", run.id));
    let content = serde_json::to_string_pretty(run)?;
    std::fs::write(&file, content)?;
    Ok(())
}

/// Load all runs, newest first.
pub fn load_runs() -> Result<Vec<PipelineRun>> {
    let dir = runs_dir()?;
    let mut runs = Vec::new();

    if !dir.exists() {
        return Ok(runs);
    }

    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            let content = std::fs::read_to_string(&path)?;
            if let Ok(run) = serde_json::from_str::<PipelineRun>(&content) {
                runs.push(run);
            }
        }
    }

    runs.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(runs)
}

/// Load runs for a specific project path.
pub fn load_runs_for_project(repo_path: &str) -> Result<Vec<PipelineRun>> {
    let all = load_runs()?;
    Ok(all.into_iter().filter(|r| r.repo_path == repo_path).collect())
}

/// Load a single run by ID.
pub fn load_run(id: &str) -> Result<Option<PipelineRun>> {
    let file = runs_dir()?.join(format!("{}.json", id));
    if !file.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&file)?;
    let run: PipelineRun = serde_json::from_str(&content)?;
    Ok(Some(run))
}

/// Delete a single run by ID.
pub fn delete_run(id: &str) -> Result<()> {
    let file = runs_dir()?.join(format!("{}.json", id));
    if file.exists() {
        std::fs::remove_file(&file)
            .with_context(|| format!("Failed to delete run file: {}", file.display()))?;
    }
    Ok(())
}

/// Delete all runs for a specific project path.
pub fn clear_runs_for_project(repo_path: &str) -> Result<u32> {
    let runs = load_runs_for_project(repo_path)?;
    let mut count = 0u32;
    for run in &runs {
        delete_run(&run.id)?;
        count += 1;
    }
    Ok(count)
}

// ---------------------------------------------------------------------------
// Phase 6: Run history queries
// ---------------------------------------------------------------------------

/// Find the last successful run for a project (optionally filtered by environment).
pub fn last_successful_run(
    repo_path: &str,
    environment: Option<&str>,
) -> Result<Option<PipelineRun>> {
    let runs = load_runs_for_project(repo_path)?;
    Ok(runs
        .into_iter()
        .find(|r| {
            r.status == RunStatus::Success
                && match environment {
                    Some(env) => r.environment.as_deref() == Some(env),
                    None => true,
                }
        }))
}

/// Get deployment history for a project filtered by environment, newest first.
pub fn deployment_history(
    repo_path: &str,
    environment: &str,
) -> Result<Vec<DeploymentRecord>> {
    let runs = load_runs_for_project(repo_path)?;
    Ok(runs
        .into_iter()
        .filter(|r| r.environment.as_deref() == Some(environment))
        .map(|r| DeploymentRecord {
            run_id: r.id,
            pipeline_name: r.pipeline_name,
            environment: environment.to_string(),
            status: r.status,
            branch: r.branch,
            commit: r.commit,
            started_at: r.started_at,
            duration_ms: r.duration_ms,
            run_kind: r.run_kind,
        })
        .collect())
}

/// Recover runs that were in-progress when the app crashed.
///
/// Any run still marked as `Running` on startup was interrupted by a crash.
/// This marks them as `Failed` so they appear correctly in history and can
/// be retried.
pub fn recover_interrupted_runs() -> Result<u32> {
    let dir = runs_dir()?;
    if !dir.exists() {
        return Ok(0);
    }

    let mut count = 0u32;
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let content = std::fs::read_to_string(&path)?;
        if let Ok(mut run) = serde_json::from_str::<PipelineRun>(&content) {
            if run.status == RunStatus::Running {
                run.status = RunStatus::Failed;
                if run.finished_at.is_none() {
                    run.finished_at = Some(chrono::Utc::now());
                }

                // Find the stage that was running when the crash happened
                // and mark it as failed with a crash message.
                for stage in &mut run.stage_results {
                    if stage.status == crate::engine::models::StageStatus::Running {
                        stage.status = crate::engine::models::StageStatus::Failed;
                        stage.finished_at = Some(chrono::Utc::now());
                        stage.stderr = format!(
                            "{}\n[chibby] App crashed during this stage. Check system logs for details.",
                            stage.stderr
                        );
                        log::info!(
                            "Recovered interrupted run {}: crashed during stage '{}'",
                            run.id, stage.stage_name
                        );
                    }
                }

                let updated = serde_json::to_string_pretty(&run)?;
                std::fs::write(&path, updated)?;
                count += 1;
                log::info!("Recovered interrupted run: {}", run.id);
            }
        }
    }
    Ok(count)
}

/// Count how many retries exist for a given parent run.
pub fn retry_count_for_run(parent_run_id: &str) -> Result<u32> {
    let runs = load_runs()?;
    let count = runs
        .iter()
        .filter(|r| r.parent_run_id.as_deref() == Some(parent_run_id))
        .count() as u32;
    Ok(count)
}
