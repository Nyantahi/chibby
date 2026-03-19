use crate::engine::models::{ArtifactConfig, CleanupConfig, CleanupResult};
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

    let toml_str = toml::to_string_pretty(config)
        .context("Failed to serialize cleanup config")?;

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

    // 2. Prune old run history
    prune_run_history(cleanup_config.run_retention, dry_run, &mut result)?;

    if dry_run {
        log::info!(
            "Cleanup dry run: would remove {} artifacts, {} runs, free {} bytes",
            result.artifacts_removed,
            result.runs_removed,
            result.bytes_freed
        );
    } else {
        log::info!(
            "Cleanup complete: removed {} artifacts, {} runs, freed {} bytes",
            result.artifacts_removed,
            result.runs_removed,
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
            result.details.push(format!("Removed artifact version: {dir_name}"));
        }

        result.artifacts_removed += 1;
        result.bytes_freed += dir_size;
    }

    Ok(())
}

/// Prune old run history entries beyond the retention limit.
fn prune_run_history(
    retention: u32,
    dry_run: bool,
    result: &mut CleanupResult,
) -> Result<()> {
    let runs = persistence::load_runs()?;

    if runs.len() <= retention as usize {
        return Ok(());
    }

    let to_remove = runs.len() - retention as usize;

    // Runs are sorted newest-first, so we remove from the end
    for run in runs.iter().skip(retention as usize).take(to_remove) {
        if dry_run {
            result.details.push(format!(
                "Would remove run: {} ({})",
                run.id,
                run.started_at.format("%Y-%m-%d %H:%M")
            ));
        } else {
            if let Err(e) = persistence::delete_run(&run.id) {
                log::warn!("Failed to delete run {}: {e}", run.id);
                continue;
            }
            result
                .details
                .push(format!("Removed run: {}", run.id));
        }

        result.runs_removed += 1;
    }

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
