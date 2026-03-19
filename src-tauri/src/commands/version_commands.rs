use crate::engine::models::{BumpLevel, BumpResult, ChangelogEntry, VersionInfo};
use crate::engine::version;
use std::path::Path;

/// Detect version files and current version in a repo.
#[tauri::command]
pub fn detect_versions(repo_path: String) -> Result<VersionInfo, String> {
    version::detect_versions(Path::new(&repo_path)).map_err(|e| e.to_string())
}

/// Bump the version across all version files.
/// level: "patch" | "minor" | "major"
/// explicit_version: optional override (ignores level)
/// create_tag: whether to create a git tag
#[tauri::command]
pub fn bump_version(
    repo_path: String,
    level: BumpLevel,
    explicit_version: Option<String>,
    create_tag: bool,
) -> Result<BumpResult, String> {
    version::bump_version(
        Path::new(&repo_path),
        &level,
        explicit_version.as_deref(),
        create_tag,
    )
    .map_err(|e| e.to_string())
}

/// Generate a changelog from commits since a tag.
#[tauri::command]
pub fn generate_changelog(
    repo_path: String,
    since_tag: Option<String>,
) -> Result<Vec<ChangelogEntry>, String> {
    version::generate_changelog(Path::new(&repo_path), since_tag.as_deref())
        .map_err(|e| e.to_string())
}
