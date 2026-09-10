use crate::engine::app_settings;
use crate::engine::models::{ArtifactConfig, CleanupConfig, CleanupResult};
use crate::engine::run_index;
use crate::engine::{artifacts, persistence};
use anyhow::{Context, Result};
use std::path::Path;

// ---------------------------------------------------------------------------
// Cleanup config persistence (.chibby/cleanup.toml)
// ---------------------------------------------------------------------------

/// Save cleanup config to .chibby/cleanup.toml.
pub fn save_cleanup_config(repo_path: &Path, config: &CleanupConfig) -> Result<()> {
    let chibby_dir = repo_path.join(".chibby");
    std::fs::create_dir_all(&chibby_dir)?;

    let toml_str = toml::to_string_pretty(config).context("Failed to serialize cleanup config")?;

    let file_path = chibby_dir.join("cleanup.toml");
    std::fs::write(&file_path, &toml_str)?;

    log::info!("Saved cleanup config to {}", file_path.display());
    Ok(())
}

/// Load cleanup config from .chibby/cleanup.toml.
pub fn load_cleanup_config(repo_path: &Path) -> Result<CleanupConfig> {
    let file_path = repo_path.join(".chibby").join("cleanup.toml");
    if !file_path.exists() {
        return Ok(CleanupConfig::default());
    }
    let content = std::fs::read_to_string(&file_path)
        .with_context(|| format!("Failed to read {}", file_path.display()))?;

    let config: CleanupConfig = toml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", file_path.display()))?;

    Ok(config)
}

/// Resolve cleanup config for a repo, falling back to app-level defaults.
///
/// If a per-repo `.chibby/cleanup.toml` exists, it is used as-is.
/// Otherwise, the app-level settings (`default_artifact_retention` /
/// `default_run_retention`) are used.
pub fn resolve_cleanup_config(repo_path: &Path) -> Result<CleanupConfig> {
    let file_path = repo_path.join(".chibby").join("cleanup.toml");
    if file_path.exists() {
        return load_cleanup_config(repo_path);
    }

    // No per-repo config — use app-level defaults.
    let app = app_settings::load_app_settings().unwrap_or_default();

    Ok(CleanupConfig {
        artifact_retention: app.default_artifact_retention,
        run_retention: app.default_run_retention,
        prune_remote_docker: false,
        ..CleanupConfig::default()
    })
}

// ---------------------------------------------------------------------------
// Cleanup operations
// ---------------------------------------------------------------------------

/// Run cleanup: prune old artifacts and run history.
/// If `dry_run` is true, compute what would be cleaned without deleting.
pub fn run_cleanup(
    repo_path: &Path,
    cleanup_config: &CleanupConfig,
    artifact_config: &ArtifactConfig,
    dry_run: bool,
) -> Result<CleanupResult> {
    let mut result = CleanupResult {
        artifacts_removed: 0,
        runs_removed: 0,
        bytes_freed: 0,
        index_entries_pruned: 0,
        index_entries: 0,
        index_bytes: 0,
        details: Vec::new(),
    };

    // 1. Prune old artifact versions
    prune_artifacts(
        repo_path,
        artifact_config,
        cleanup_config.artifact_retention,
        dry_run,
        &mut result,
    )?;

    // 2. Prune old run records for this project
    prune_run_history(
        repo_path,
        cleanup_config.run_retention,
        dry_run,
        &mut result,
    )?;

    // 3. Prune the run index by its own, much longer, bounds
    prune_run_index(cleanup_config, dry_run, &mut result)?;

    if dry_run {
        log::info!(
            "Cleanup dry run: would remove {} artifacts, {} runs, {} index entries, free {} bytes",
            result.artifacts_removed,
            result.runs_removed,
            result.index_entries_pruned,
            result.bytes_freed
        );
    } else {
        log::info!(
            "Cleanup complete: removed {} artifacts, {} runs, {} index entries, freed {} bytes",
            result.artifacts_removed,
            result.runs_removed,
            result.index_entries_pruned,
            result.bytes_freed
        );
    }

    Ok(result)
}

/// Prune artifact directories beyond the retention limit.
fn prune_artifacts(
    repo_path: &Path,
    config: &ArtifactConfig,
    retention: u32,
    dry_run: bool,
    result: &mut CleanupResult,
) -> Result<()> {
    let dirs = artifacts::get_artifact_dirs_sorted(repo_path, config)?;

    if dirs.len() <= retention as usize {
        return Ok(());
    }

    let to_remove = dirs.len() - retention as usize;

    for dir in dirs.iter().take(to_remove) {
        let dir_size = dir_size(dir)?;
        let dir_name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        if dry_run {
            result.details.push(format!(
                "Would remove artifact version: {dir_name} ({} bytes)",
                dir_size
            ));
        } else {
            std::fs::remove_dir_all(dir)
                .with_context(|| format!("Failed to remove {}", dir.display()))?;
            result
                .details
                .push(format!("Removed artifact version: {dir_name}"));
        }

        result.artifacts_removed += 1;
        result.bytes_freed += dir_size;
    }

    Ok(())
}

/// Prune run records beyond the retention limit, **per project**.
///
/// Per project, not globally: a project run fifty times in an afternoon must
/// not evict every other project's history. The run index drives this — it
/// carries `repo_path` and `started_at` without the logs, so deciding what to
/// drop costs one small JSON parse instead of deserializing every run.
///
/// Only the heavy record is deleted; the summary stays in the index so trends
/// survive (see [`persistence::prune_run_payload`]).
fn prune_run_history(
    repo_path: &Path,
    retention: u32,
    dry_run: bool,
    result: &mut CleanupResult,
) -> Result<()> {
    // Only this project. `retention` comes from this project's cleanup.toml,
    // so applying it to other projects would let whichever project ran last
    // dictate everyone else's history depth.
    let repo = repo_path.to_string_lossy();
    let summaries = run_index::load()?;
    let mut kept = 0u32;

    // Newest first, so the quota is filled by the newest runs.
    for summary in summaries
        .iter()
        .filter(|s| !s.logs_pruned && s.repo_path == repo)
    {
        if kept < retention {
            kept += 1;
            continue;
        }

        if dry_run {
            result.details.push(format!(
                "Would remove run: {} ({})",
                summary.id,
                summary.started_at.format("%Y-%m-%d %H:%M")
            ));
        } else {
            if let Err(e) = persistence::prune_run_payload(&summary.id) {
                log::warn!("Failed to prune run {}: {e}", summary.id);
                continue;
            }
            result
                .details
                .push(format!("Removed run logs: {} (summary kept)", summary.id));
        }

        result.runs_removed += 1;
    }

    Ok(())
}

/// Apply the index's own retention bounds and report its size, so the file
/// never grows unnoticed.
fn prune_run_index(
    config: &CleanupConfig,
    dry_run: bool,
    result: &mut CleanupResult,
) -> Result<()> {
    let (days, max) = (config.index_retention_days, config.index_max_entries);

    result.index_entries_pruned = match dry_run {
        true => run_index::prune_preview(days, max)?,
        false => run_index::prune(days, max)?,
    };
    if dry_run && result.index_entries_pruned > 0 {
        result.details.push(format!(
            "Would remove {} run index entries",
            result.index_entries_pruned
        ));
    }

    let stats = run_index::stats()?;
    result.index_entries = stats.entries;
    result.index_bytes = stats.bytes;
    Ok(())
}

/// Calculate total size of a directory recursively.
fn dir_size(path: &Path) -> Result<u64> {
    let mut total: u64 = 0;

    if path.is_file() {
        return Ok(std::fs::metadata(path)?.len());
    }

    for entry in std::fs::read_dir(path)?.flatten() {
        let ft = entry.file_type()?;
        if ft.is_file() {
            total += entry.metadata()?.len();
        } else if ft.is_dir() {
            total += dir_size(&entry.path())?;
        }
    }

    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::models::{PipelineRun, RunStatus};
    use crate::engine::persistence::scoped_test_data_dir;
    use chrono::{Duration, Utc};

    fn empty_result() -> CleanupResult {
        CleanupResult {
            artifacts_removed: 0,
            runs_removed: 0,
            bytes_freed: 0,
            index_entries_pruned: 0,
            index_entries: 0,
            index_bytes: 0,
            details: Vec::new(),
        }
    }

    const REPO: &str = "/tmp/chibby-cleanup";

    fn save(id: &str, minutes_ago: i64) {
        save_in(id, REPO, minutes_ago);
    }

    fn save_in(id: &str, repo_path: &str, minutes_ago: i64) {
        let mut run = PipelineRun::new_with_id(id, "ci", repo_path, None);
        run.status = RunStatus::Success;
        run.started_at = Utc::now() - Duration::minutes(minutes_ago);
        persistence::save_run(&run).unwrap();
    }

    fn surviving_ids() -> Vec<String> {
        run_index::load()
            .unwrap()
            .into_iter()
            .filter(|s| !s.logs_pruned)
            .map(|s| s.id)
            .collect()
    }

    /// The starvation bug: a busy project used to consume the whole global
    /// quota and delete every other project's history.
    #[test]
    fn test_run_retention_is_per_project() {
        let (_dir, _lock) = scoped_test_data_dir();
        for (i, id) in ["busy1", "busy2", "busy3", "busy4"].iter().enumerate() {
            save_in(id, "/tmp/busy", i as i64);
        }
        save_in("quiet1", "/tmp/quiet", 10);
        save_in("quiet2", "/tmp/quiet", 20);

        // A third quiet run, so the quiet project is ABOVE the busy project's
        // retention. If retention were applied globally rather than to the
        // project being cleaned, this run would be pruned too.
        save_in("quiet3", "/tmp/quiet", 30);

        let mut result = empty_result();
        prune_run_history(Path::new("/tmp/busy"), 2, false, &mut result).unwrap();

        let mut survivors = surviving_ids();
        survivors.sort();
        assert_eq!(result.runs_removed, 2, "only the busy project is pruned");
        assert_eq!(
            survivors,
            vec![
                "busy1".to_string(),
                "busy2".to_string(),
                "quiet1".to_string(),
                "quiet2".to_string(),
                "quiet3".to_string()
            ],
            "cleaning one project must not touch another project's history"
        );
    }

    #[test]
    fn test_pruning_keeps_the_summary_and_drops_only_the_record() {
        let (_dir, _lock) = scoped_test_data_dir();
        save("new", 1);
        save("old", 100);

        let mut result = empty_result();
        prune_run_history(Path::new(REPO), 1, false, &mut result).unwrap();

        assert_eq!(result.runs_removed, 1);
        assert!(persistence::load_run("old").unwrap().is_none());
        let summaries = run_index::load().unwrap();
        assert_eq!(summaries.len(), 2);
        assert!(summaries.iter().any(|s| s.id == "old" && s.logs_pruned));
    }

    #[test]
    fn test_a_project_below_the_limit_is_untouched() {
        let (_dir, _lock) = scoped_test_data_dir();
        save("a", 1);
        save("b", 2);

        let mut result = empty_result();
        prune_run_history(Path::new(REPO), 50, false, &mut result).unwrap();

        assert_eq!(result.runs_removed, 0);
        assert_eq!(surviving_ids().len(), 2);
    }

    #[test]
    fn test_dry_run_reports_without_deleting() {
        let (_dir, _lock) = scoped_test_data_dir();
        save("new", 1);
        save("old", 100);

        let mut result = empty_result();
        prune_run_history(Path::new(REPO), 1, true, &mut result).unwrap();

        assert_eq!(result.runs_removed, 1);
        assert!(result.details[0].starts_with("Would remove run: old"));
        assert!(persistence::load_run("old").unwrap().is_some());
    }

    #[test]
    fn test_index_pruning_reports_size_and_respects_its_own_bounds() {
        let (_dir, _lock) = scoped_test_data_dir();
        save("recent", 1);
        save("ancient", 60 * 24 * 400);

        let config = CleanupConfig {
            index_retention_days: 180,
            ..CleanupConfig::default()
        };

        let mut preview = empty_result();
        prune_run_index(&config, true, &mut preview).unwrap();
        assert_eq!(preview.index_entries_pruned, 1);
        assert_eq!(preview.index_entries, 2);
        assert!(preview.index_bytes > 0);
        assert!(preview
            .details
            .iter()
            .any(|d| d == "Would remove 1 run index entries"));

        let mut applied = empty_result();
        prune_run_index(&config, false, &mut applied).unwrap();
        assert_eq!(applied.index_entries_pruned, 1);
        assert_eq!(applied.index_entries, 1);
    }
}
