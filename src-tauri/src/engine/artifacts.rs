use crate::engine::models::{Artifact, ArtifactConfig, ArtifactManifest};
use anyhow::{Context, Result};
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Artifact config persistence (.chibby/artifacts.toml)
// ---------------------------------------------------------------------------

/// Save artifact config to .chibby/artifacts.toml.
pub fn save_artifact_config(repo_path: &Path, config: &ArtifactConfig) -> Result<()> {
    let chibby_dir = repo_path.join(".chibby");
    std::fs::create_dir_all(&chibby_dir)?;

    let toml_str = toml::to_string_pretty(config)
        .context("Failed to serialize artifact config")?;

    let file_path = chibby_dir.join("artifacts.toml");
    std::fs::write(&file_path, &toml_str)?;

    log::info!("Saved artifact config to {}", file_path.display());
    Ok(())
}

/// Load artifact config from .chibby/artifacts.toml.
pub fn load_artifact_config(repo_path: &Path) -> Result<ArtifactConfig> {
    let file_path = repo_path.join(".chibby").join("artifacts.toml");
    if !file_path.exists() {
        return Ok(ArtifactConfig::default());
    }
    let content = std::fs::read_to_string(&file_path)
        .with_context(|| format!("Failed to read {}", file_path.display()))?;

    let config: ArtifactConfig = toml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", file_path.display()))?;

    Ok(config)
}

// ---------------------------------------------------------------------------
// Artifact collection
// ---------------------------------------------------------------------------

/// Collect artifacts matching the configured glob patterns.
pub fn collect_artifacts(
    repo_path: &Path,
    config: &ArtifactConfig,
    project_name: &str,
    version: &str,
) -> Result<ArtifactManifest> {
    let output_dir = repo_path.join(&config.output_dir).join(version);
    std::fs::create_dir_all(&output_dir)
        .with_context(|| format!("Failed to create artifact directory {}", output_dir.display()))?;

    let mut artifacts = Vec::new();

    for pattern in &config.patterns {
        let full_pattern = repo_path.join(pattern);
        let pattern_str = full_pattern
            .to_str()
            .context("Invalid pattern path")?;

        let matches = glob::glob(pattern_str)
            .with_context(|| format!("Invalid glob pattern: {pattern}"))?;

        for entry in matches.flatten() {
            if entry.is_file() {
                let artifact = collect_single_artifact(
                    &entry,
                    &output_dir,
                    project_name,
                    version,
                )?;
                artifacts.push(artifact);
            }
        }
    }

    // Get git info for the manifest
    let commit = get_git_commit(repo_path);
    let branch = get_git_branch(repo_path);

    let manifest = ArtifactManifest {
        project: project_name.to_string(),
        version: version.to_string(),
        commit,
        branch,
        created_at: Utc::now(),
        artifacts,
    };

    // Write the manifest
    let manifest_path = output_dir.join("manifest.json");
    let manifest_json = serde_json::to_string_pretty(&manifest)
        .context("Failed to serialize artifact manifest")?;
    std::fs::write(&manifest_path, &manifest_json)?;

    // Also write checksums file
    write_checksums_file(&output_dir, &manifest)?;

    log::info!(
        "Collected {} artifacts for {project_name} v{version}",
        manifest.artifacts.len()
    );

    Ok(manifest)
}

/// Collect a single file as an artifact: copy to output dir, compute checksum.
fn collect_single_artifact(
    source_path: &Path,
    output_dir: &Path,
    project_name: &str,
    version: &str,
) -> Result<Artifact> {
    let file_name = source_path
        .file_name()
        .and_then(|n| n.to_str())
        .context("Invalid file name")?
        .to_string();

    let canonical_name = build_canonical_name(project_name, version, &file_name);
    let dest_path = output_dir.join(&canonical_name);

    // Copy the file
    std::fs::copy(source_path, &dest_path)
        .with_context(|| format!("Failed to copy {} to {}", source_path.display(), dest_path.display()))?;

    // Compute SHA256
    let sha256 = compute_sha256(&dest_path)?;

    let metadata = std::fs::metadata(&dest_path)?;

    Ok(Artifact {
        file_name,
        canonical_name,
        path: dest_path.display().to_string(),
        sha256,
        size_bytes: metadata.len(),
        collected_at: Utc::now(),
    })
}

/// Build a canonical artifact name: {project}-{version}-{platform}-{arch}.{ext}
fn build_canonical_name(project_name: &str, version: &str, original_name: &str) -> String {
    let platform = current_platform();
    let arch = current_arch();

    let ext = Path::new(original_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");

    format!("{project_name}-{version}-{platform}-{arch}.{ext}")
}

fn current_platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

fn current_arch() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "unknown"
    }
}

/// Compute SHA256 checksum of a file.
pub fn compute_sha256(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let hash = hasher.finalize();
    Ok(format!("{:x}", hash))
}

/// Write a SHA256SUMS file alongside the artifacts.
fn write_checksums_file(output_dir: &Path, manifest: &ArtifactManifest) -> Result<()> {
    let mut content = String::new();
    for artifact in &manifest.artifacts {
        content.push_str(&format!("{}  {}\n", artifact.sha256, artifact.canonical_name));
    }
    let path = output_dir.join("SHA256SUMS");
    std::fs::write(&path, &content)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Artifact listing and lookup
// ---------------------------------------------------------------------------

/// List all artifact manifests for a project.
pub fn list_artifact_manifests(repo_path: &Path, config: &ArtifactConfig) -> Result<Vec<ArtifactManifest>> {
    let artifacts_dir = repo_path.join(&config.output_dir);
    if !artifacts_dir.exists() {
        return Ok(Vec::new());
    }

    let mut manifests = Vec::new();

    let entries = std::fs::read_dir(&artifacts_dir)
        .with_context(|| format!("Failed to read {}", artifacts_dir.display()))?;

    for entry in entries.flatten() {
        if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
            let manifest_path = entry.path().join("manifest.json");
            if manifest_path.exists() {
                let content = std::fs::read_to_string(&manifest_path)?;
                if let Ok(manifest) = serde_json::from_str::<ArtifactManifest>(&content) {
                    manifests.push(manifest);
                }
            }
        }
    }

    // Sort by creation time, newest first
    manifests.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(manifests)
}

/// Get artifact directories sorted oldest-first for pruning.
pub fn get_artifact_dirs_sorted(repo_path: &Path, config: &ArtifactConfig) -> Result<Vec<PathBuf>> {
    let artifacts_dir = repo_path.join(&config.output_dir);
    if !artifacts_dir.exists() {
        return Ok(Vec::new());
    }

    let mut dirs: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();

    for entry in std::fs::read_dir(&artifacts_dir)?.flatten() {
        if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
            let modified = entry.metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            dirs.push((entry.path(), modified));
        }
    }

    // Sort oldest first (for pruning)
    dirs.sort_by_key(|(_, t)| *t);

    Ok(dirs.into_iter().map(|(p, _)| p).collect())
}

// ---------------------------------------------------------------------------
// Git helpers
// ---------------------------------------------------------------------------

fn get_git_commit(repo_path: &Path) -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(repo_path)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn get_git_branch(repo_path: &Path) -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo_path)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}
