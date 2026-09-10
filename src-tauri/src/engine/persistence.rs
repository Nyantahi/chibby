use crate::engine::models::{DeploymentRecord, PipelineRun, Project, RunStatus, StageStatus};
use crate::engine::run_index;
use anyhow::{Context, Result};

// The cross-process run lock and per-trigger state live in the data directory
// too. They are implemented in their own modules to keep this file readable,
// and re-exported here so `persistence::acquire_run_lock` keeps working.
pub use crate::engine::locks::{acquire_run_lock, acquire_run_lock_with_id, RunLock, RunLockInfo};
pub use crate::engine::trigger_state::{
    get_trigger_state, load_trigger_state, mutate_trigger_state, save_trigger_state,
    TriggerStateEntry, TriggerStateMap,
};
use std::path::PathBuf;
use std::sync::Mutex;

/// Process-wide lock serializing read-modify-write of `projects.json`.
/// Concurrent run completions (and add/remove) must not clobber each other's
/// updates to the shared index.
static PROJECTS_LOCK: Mutex<()> = Mutex::new(());

/// Environment variable overriding the data directory.
///
/// Exists so run history, trigger state and locks can be exercised against a
/// scratch directory. Tests must set this; without it every persistence test
/// would read and write the developer's real Chibby data.
pub const DATA_DIR_ENV: &str = "CHIBBY_DATA_DIR";

/// Serializes tests that repoint `CHIBBY_DATA_DIR`, which is process-global.
#[cfg(test)]
static DATA_DIR_TEST_LOCK: Mutex<()> = Mutex::new(());

/// Point the data directory at a fresh scratch dir for the duration of a test.
///
/// Hold both returned values for the whole test: the guard serializes against
/// other data-dir tests, and dropping the `TempDir` deletes the scratch data.
#[cfg(test)]
pub(crate) fn scoped_test_data_dir() -> (tempfile::TempDir, std::sync::MutexGuard<'static, ()>) {
    let guard = DATA_DIR_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let temp = tempfile::TempDir::new().expect("scratch data dir");
    std::env::set_var(DATA_DIR_ENV, temp.path());
    (temp, guard)
}

/// Get the Chibby application data directory.
///
/// - macOS: ~/Library/Application Support/Chibby/
/// - Linux: ~/.local/share/chibby/
/// - Windows: %APPDATA%\Chibby\
///
/// Overridden wholesale by `CHIBBY_DATA_DIR` when set.
pub fn data_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os(DATA_DIR_ENV).filter(|v| !v.is_empty()) {
        return prepare_data_dir(PathBuf::from(dir));
    }

    let base = dirs::data_dir().context("Could not determine app data directory")?;

    #[cfg(target_os = "macos")]
    let dir = base.join("Chibby");
    #[cfg(target_os = "linux")]
    let dir = base.join("chibby");
    #[cfg(target_os = "windows")]
    let dir = base.join("Chibby");
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let dir = base.join("chibby");

    prepare_data_dir(dir)
}

/// Create `dir` if needed and lock it down to the owner.
fn prepare_data_dir(dir: PathBuf) -> Result<PathBuf> {
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create data directory: {}", dir.display()))?;

    // Ensure the data directory has restrictive permissions (owner-only)
    // to protect run logs and pipeline configs that may contain sensitive data.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o700);
        let _ = std::fs::set_permissions(&dir, perms);
    }

    Ok(dir)
}

/// Path to the projects index file.
fn projects_file() -> Result<PathBuf> {
    Ok(data_dir()?.join("projects.json"))
}

/// Path to the runs directory.
pub(crate) fn runs_dir() -> Result<PathBuf> {
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

/// Atomically load, mutate, and persist the projects list under the
/// process-wide lock, so concurrent callers can't lose each other's updates.
pub fn mutate_projects<F, R>(f: F) -> Result<R>
where
    F: FnOnce(&mut Vec<Project>) -> R,
{
    let _guard = PROJECTS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut projects = load_projects()?;
    let result = f(&mut projects);
    save_projects(&projects)?;
    Ok(result)
}

/// Add a project to the index.
pub fn add_project(project: Project) -> Result<()> {
    mutate_projects(move |projects| {
        // Avoid duplicates by path.
        if projects.iter().any(|p| p.path == project.path) {
            return;
        }
        projects.push(project);
    })
}

/// Remove a project by id.
pub fn remove_project(id: &str) -> Result<()> {
    mutate_projects(|projects| projects.retain(|p| p.id != id))
}

// ---------------------------------------------------------------------------
// Run history persistence
// ---------------------------------------------------------------------------

/// Save a pipeline run record, keeping the run index in step.
pub fn save_run(run: &PipelineRun) -> Result<()> {
    let dir = runs_dir()?;
    let file = dir.join(format!("{}.json", run.id));
    let content = serde_json::to_string_pretty(run)?;
    std::fs::write(&file, content)?;
    index_best_effort("upsert", &run.id, run_index::upsert(run));
    Ok(())
}

/// The run index is derived state for the metrics views. A failure to update
/// it must never fail a run, so it is logged and swallowed — the next
/// `run_index::load` notices the drift and rebuilds.
fn index_best_effort(action: &str, run_id: &str, result: Result<()>) {
    if let Err(e) = result {
        log::warn!("Run index {action} failed for {run_id}: {e}");
    }
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
    Ok(all
        .into_iter()
        .filter(|r| r.repo_path == repo_path)
        .collect())
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

/// Delete a single run by ID: the record *and* its index entry.
///
/// This is the explicit "delete this run" path — a run the user deleted
/// should disappear from the metrics too. Retention pruning wants the
/// opposite; see [`prune_run_payload`].
pub fn delete_run(id: &str) -> Result<()> {
    remove_run_file(id)?;
    index_best_effort("remove", id, run_index::remove(id));
    Ok(())
}

/// Delete a run's record but keep its index summary.
///
/// Used by retention pruning: the logs are the expensive part, the summary is
/// a few hundred bytes, so trends survive long after the output is gone.
pub fn prune_run_payload(id: &str) -> Result<()> {
    remove_run_file(id)?;
    index_best_effort("forget_payload", id, run_index::forget_payload(id));
    Ok(())
}

/// Remove the on-disk record, leaving the index alone.
pub(crate) fn remove_run_file(id: &str) -> Result<()> {
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
        remove_run_file(&run.id)?;
        count += 1;
    }
    index_best_effort(
        "remove_for_project",
        repo_path,
        run_index::remove_for_project(repo_path).map(|_| ()),
    );
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
    Ok(runs.into_iter().find(|r| {
        r.status == RunStatus::Success
            && match environment {
                Some(env) => r.environment.as_deref() == Some(env),
                None => true,
            }
    }))
}

/// Find the newest run that is safe to roll a deployment back to.
///
/// Deliberately stricter than [`last_successful_run`]: a run can be `Success`
/// without having deployed anything (a `--stage lint` run, or a deploy stage
/// skipped by a `when` condition). Rolling back to one of those redeploys
/// nothing while reporting success, so the named deploy stage must itself have
/// run, succeeded, and not have failed its health check.
pub fn last_good_deployment(
    repo_path: &str,
    environment: &str,
    deploy_stage: &str,
    exclude_run_id: &str,
) -> Result<Option<PipelineRun>> {
    // `load_runs_for_project` is newest-first, so the first match wins.
    let runs = load_runs_for_project(repo_path)?;
    Ok(runs
        .into_iter()
        .find(|run| is_rollback_target(run, environment, deploy_stage, exclude_run_id)))
}

/// Whether `run` qualifies as a rollback target for `deploy_stage`.
fn is_rollback_target(
    run: &PipelineRun,
    environment: &str,
    deploy_stage: &str,
    exclude_run_id: &str,
) -> bool {
    if run.id == exclude_run_id
        || run.status != RunStatus::Success
        || run.environment.as_deref() != Some(environment)
        || run.pipeline_snapshot.is_none()
    {
        return false;
    }

    let Some(stage) = run
        .stage_results
        .iter()
        .find(|s| s.stage_name == deploy_stage)
    else {
        return false;
    };

    stage.status == StageStatus::Success && stage.health_check_passed != Some(false)
}

/// Get deployment history for a project filtered by environment, newest first.
pub fn deployment_history(repo_path: &str, environment: &str) -> Result<Vec<DeploymentRecord>> {
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
                    if stage.status == StageStatus::Running {
                        stage.status = StageStatus::Failed;
                        stage.finished_at = Some(chrono::Utc::now());
                        stage.stderr = format!(
                            "{}\n[chibby] App crashed during this stage. Check system logs for details.",
                            stage.stderr
                        );
                        log::info!(
                            "Recovered interrupted run {}: crashed during stage '{}'",
                            run.id,
                            stage.stage_name
                        );
                    }
                }

                let updated = serde_json::to_string_pretty(&run)?;
                std::fs::write(&path, updated)?;
                index_best_effort("upsert", &run.id, run_index::upsert(&run));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::models::{Pipeline, Stage, StageResult};
    use chrono::{Duration, Utc};

    const REPO: &str = "/tmp/chibby-last-good";

    fn snapshot() -> Pipeline {
        Pipeline {
            name: "deploy".to_string(),
            on_health_failure: None,
            stages: vec![Stage {
                name: "deploy".to_string(),
                commands: vec!["./deploy.sh".to_string()],
                ..Default::default()
            }],
        }
    }

    fn stage_result(status: StageStatus, health_check_passed: Option<bool>) -> StageResult {
        StageResult {
            stage_name: "deploy".to_string(),
            status,
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
            started_at: None,
            finished_at: None,
            duration_ms: None,
            health_check_passed,
            attempts: Some(1),
            skip_reason: None,
        }
    }

    /// A saved run, `minutes_ago` old, with one `deploy` stage result.
    fn saved_run(
        id: &str,
        minutes_ago: i64,
        stage: StageResult,
        with_snapshot: bool,
    ) -> PipelineRun {
        let mut run = PipelineRun::new_with_id(id, "deploy", REPO, Some("prod".to_string()));
        run.status = RunStatus::Success;
        run.started_at = Utc::now() - Duration::minutes(minutes_ago);
        run.stage_results = vec![stage];
        run.pipeline_snapshot = with_snapshot.then(snapshot);
        save_run(&run).unwrap();
        run
    }

    fn last_good() -> Option<PipelineRun> {
        last_good_deployment(REPO, "prod", "deploy", "").unwrap()
    }

    #[test]
    fn test_last_good_deployment_skips_skipped_deploy_stage() {
        let (_dir, _lock) = scoped_test_data_dir();
        saved_run("skipped", 1, stage_result(StageStatus::Skipped, None), true);

        assert!(last_good().is_none());
    }

    #[test]
    fn test_last_good_deployment_skips_failed_health_check() {
        let (_dir, _lock) = scoped_test_data_dir();
        saved_run(
            "unhealthy",
            1,
            stage_result(StageStatus::Success, Some(false)),
            true,
        );

        assert!(last_good().is_none());
    }

    #[test]
    fn test_last_good_deployment_skips_run_without_snapshot() {
        let (_dir, _lock) = scoped_test_data_dir();
        saved_run(
            "no-snapshot",
            1,
            stage_result(StageStatus::Success, Some(true)),
            false,
        );

        assert!(last_good().is_none());
    }

    #[test]
    fn test_last_good_deployment_skips_other_environments_and_failures() {
        let (_dir, _lock) = scoped_test_data_dir();
        let mut other_env = saved_run(
            "staging",
            1,
            stage_result(StageStatus::Success, Some(true)),
            true,
        );
        other_env.environment = Some("staging".to_string());
        save_run(&other_env).unwrap();

        let mut failed = saved_run(
            "failed",
            2,
            stage_result(StageStatus::Success, Some(true)),
            true,
        );
        failed.status = RunStatus::Failed;
        save_run(&failed).unwrap();

        assert!(last_good().is_none());
    }

    #[test]
    fn test_last_good_deployment_picks_newest_qualifying_run() {
        let (_dir, _lock) = scoped_test_data_dir();
        saved_run(
            "older",
            30,
            stage_result(StageStatus::Success, Some(true)),
            true,
        );
        saved_run(
            "newer",
            5,
            stage_result(StageStatus::Success, Some(true)),
            true,
        );
        // Newest overall, but its deploy stage never ran.
        saved_run(
            "lint-only",
            1,
            stage_result(StageStatus::Skipped, None),
            true,
        );

        assert_eq!(last_good().map(|r| r.id).as_deref(), Some("newer"));
    }

    #[test]
    fn test_last_good_deployment_respects_exclude_run_id() {
        let (_dir, _lock) = scoped_test_data_dir();
        saved_run(
            "older",
            30,
            stage_result(StageStatus::Success, Some(true)),
            true,
        );
        saved_run(
            "newer",
            5,
            stage_result(StageStatus::Success, Some(true)),
            true,
        );

        let picked = last_good_deployment(REPO, "prod", "deploy", "newer").unwrap();

        assert_eq!(picked.map(|r| r.id).as_deref(), Some("older"));
    }

    /// A stage with no recorded health check (None) is still a valid target —
    /// only an explicit failure disqualifies it.
    #[test]
    fn test_last_good_deployment_accepts_stage_without_health_check() {
        let (_dir, _lock) = scoped_test_data_dir();
        saved_run("plain", 1, stage_result(StageStatus::Success, None), true);

        assert_eq!(last_good().map(|r| r.id).as_deref(), Some("plain"));
    }

    #[test]
    fn test_last_good_deployment_requires_named_stage_to_exist() {
        let (_dir, _lock) = scoped_test_data_dir();
        saved_run(
            "plain",
            1,
            stage_result(StageStatus::Success, Some(true)),
            true,
        );

        let picked = last_good_deployment(REPO, "prod", "release", "").unwrap();

        assert!(picked.is_none());
    }
}
