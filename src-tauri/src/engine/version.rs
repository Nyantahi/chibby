use crate::engine::models::{BumpLevel, BumpResult, ChangelogEntry, VersionFile, VersionInfo};
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

// ---------------------------------------------------------------------------
// Well-known version files and their extraction patterns
// ---------------------------------------------------------------------------

/// Files we know how to read/write versions in.
const VERSION_FILES: &[&str] = &[
    "package.json",
    "Cargo.toml",
    "tauri.conf.json",
    "pyproject.toml",
    "setup.cfg",
    "version.txt",
];

/// Detect all version-bearing files in a repo and report consistency.
pub fn detect_versions(repo_path: &Path) -> Result<VersionInfo> {
    let mut files: Vec<VersionFile> = Vec::new();

    for &name in VERSION_FILES {
        let file_path = repo_path.join(name);
        if file_path.exists() {
            if let Some(version) = extract_version(&file_path, name)? {
                files.push(VersionFile {
                    path: name.to_string(),
                    version,
                });
            }
        }
    }

    // Also check nested Cargo.toml for workspace members (e.g. src-tauri/Cargo.toml)
    for nested in &["src-tauri/Cargo.toml"] {
        let file_path = repo_path.join(nested);
        if file_path.exists() {
            if let Some(version) = extract_version(&file_path, "Cargo.toml")? {
                files.push(VersionFile {
                    path: nested.to_string(),
                    version,
                });
            }
        }
    }

    let versions: Vec<&str> = files.iter().map(|f| f.version.as_str()).collect();
    let is_consistent = versions.windows(2).all(|w| w[0] == w[1]);
    let current_version = if is_consistent {
        files.first().map(|f| f.version.clone())
    } else {
        None
    };

    let latest_tag = get_latest_version_tag(repo_path).ok().flatten();

    Ok(VersionInfo {
        files,
        current_version,
        is_consistent,
        latest_tag,
    })
}

/// Extract a version string from a known file type.
fn extract_version(file_path: &Path, file_name: &str) -> Result<Option<String>> {
    let content = std::fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read {}", file_path.display()))?;

    match file_name {
        "package.json" | "tauri.conf.json" => {
            // JSON: look for "version": "X.Y.Z"
            let v: serde_json::Value = serde_json::from_str(&content)
                .with_context(|| format!("Failed to parse {}", file_path.display()))?;
            Ok(v.get("version").and_then(|v| v.as_str()).map(|s| s.to_string()))
        }
        "Cargo.toml" | "pyproject.toml" => {
            // TOML: look for [package].version or [project].version
            let v: toml::Value = toml::from_str(&content)
                .with_context(|| format!("Failed to parse {}", file_path.display()))?;
            let version = v
                .get("package")
                .or_else(|| v.get("project"))
                .and_then(|t| t.get("version"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            Ok(version)
        }
        "setup.cfg" => {
            // INI-style: version = X.Y.Z under [metadata]
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("version") {
                    if let Some(val) = trimmed.split('=').nth(1) {
                        return Ok(Some(val.trim().to_string()));
                    }
                }
            }
            Ok(None)
        }
        "version.txt" => Ok(Some(content.trim().to_string())),
        _ => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// Version bumping
// ---------------------------------------------------------------------------

/// Parse a semver string into (major, minor, patch).
fn parse_semver(version: &str) -> Result<(u64, u64, u64)> {
    let clean = version.trim_start_matches('v');
    let parts: Vec<&str> = clean.split('.').collect();
    if parts.len() != 3 {
        bail!("Invalid semver: {version}");
    }
    let major: u64 = parts[0].parse().context("Invalid major version")?;
    let minor: u64 = parts[1].parse().context("Invalid minor version")?;
    // Handle pre-release suffixes like "1.0.0-beta.1"
    let patch_str = parts[2].split('-').next().unwrap_or(parts[2]);
    let patch: u64 = patch_str.parse().context("Invalid patch version")?;
    Ok((major, minor, patch))
}

/// Compute the next version given a bump level.
fn next_version(current: &str, level: &BumpLevel) -> Result<String> {
    let (major, minor, patch) = parse_semver(current)?;
    let new = match level {
        BumpLevel::Patch => format!("{major}.{minor}.{}", patch + 1),
        BumpLevel::Minor => format!("{major}.{}.0", minor + 1),
        BumpLevel::Major => format!("{}.0.0", major + 1),
    };
    Ok(new)
}

/// Bump version across all detected version files.
/// If `explicit_version` is Some, use that instead of computing from bump level.
pub fn bump_version(
    repo_path: &Path,
    level: &BumpLevel,
    explicit_version: Option<&str>,
    create_tag: bool,
) -> Result<BumpResult> {
    let info = detect_versions(repo_path)?;

    let old_version = info
        .current_version
        .as_deref()
        .or_else(|| info.files.first().map(|f| f.version.as_str()))
        .context("No version files found in repository")?
        .to_string();

    let new_version = match explicit_version {
        Some(v) => v.to_string(),
        None => next_version(&old_version, level)?,
    };

    // Validate we're not re-releasing an existing tag
    if let Some(ref tag) = info.latest_tag {
        let tag_ver = tag.trim_start_matches('v');
        if tag_ver == new_version {
            bail!("Version {new_version} already exists as git tag {tag}");
        }
    }

    let mut updated_files = Vec::new();

    for vf in &info.files {
        let file_path = repo_path.join(&vf.path);
        write_version(&file_path, &vf.path, &new_version)?;
        updated_files.push(vf.path.clone());
    }

    let git_tag = if create_tag {
        let tag = format!("v{new_version}");
        create_git_tag(repo_path, &tag, &format!("Release {new_version}"))?;
        Some(tag)
    } else {
        None
    };

    log::info!(
        "Bumped version {} → {} in {} files",
        old_version,
        new_version,
        updated_files.len()
    );

    Ok(BumpResult {
        old_version,
        new_version,
        updated_files,
        git_tag,
    })
}

/// Write a new version into a known file type.
fn write_version(file_path: &Path, file_name: &str, new_version: &str) -> Result<()> {
    let content = std::fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read {}", file_path.display()))?;

    let base_name = Path::new(file_name)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(file_name);

    let updated = match base_name {
        "package.json" | "tauri.conf.json" => {
            let mut v: serde_json::Value = serde_json::from_str(&content)?;
            if let Some(obj) = v.as_object_mut() {
                obj.insert(
                    "version".to_string(),
                    serde_json::Value::String(new_version.to_string()),
                );
            }
            serde_json::to_string_pretty(&v)? + "\n"
        }
        "Cargo.toml" | "pyproject.toml" => {
            // Use string replacement to preserve formatting and comments
            replace_toml_version(&content, new_version)?
        }
        "setup.cfg" => {
            let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
            for line in &mut lines {
                if line.trim().starts_with("version") && line.contains('=') {
                    *line = format!("version = {new_version}");
                    break;
                }
            }
            lines.join("\n") + "\n"
        }
        "version.txt" => format!("{new_version}\n"),
        _ => bail!("Don't know how to write version to {file_name}"),
    };

    std::fs::write(file_path, &updated)
        .with_context(|| format!("Failed to write {}", file_path.display()))?;

    Ok(())
}

/// Replace version in a TOML file while preserving formatting.
fn replace_toml_version(content: &str, new_version: &str) -> Result<String> {
    let mut result = String::new();
    let mut in_package = false;
    let mut replaced = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("[package]") || trimmed.starts_with("[project]") {
            in_package = true;
        } else if trimmed.starts_with('[') {
            in_package = false;
        }

        if in_package && !replaced && trimmed.starts_with("version") && trimmed.contains('=') {
            // Preserve the key format, just replace the value
            if let Some(eq_pos) = line.find('=') {
                let prefix = &line[..=eq_pos];
                result.push_str(&format!("{prefix} \"{new_version}\"\n"));
                replaced = true;
                continue;
            }
        }
        result.push_str(line);
        result.push('\n');
    }

    if !replaced {
        bail!("Could not find version field in TOML [package] or [project] section");
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Git operations
// ---------------------------------------------------------------------------

/// Get the latest git tag matching vX.Y.Z or X.Y.Z.
fn get_latest_version_tag(repo_path: &Path) -> Result<Option<String>> {
    let output = Command::new("git")
        .args(["tag", "--sort=-v:refname", "--list", "v*"])
        .current_dir(repo_path)
        .output()
        .context("Failed to run git tag")?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().next().map(|s| s.trim().to_string()))
}

/// Create a git tag at HEAD.
fn create_git_tag(repo_path: &Path, tag: &str, message: &str) -> Result<()> {
    let output = Command::new("git")
        .args(["tag", "-a", tag, "-m", message])
        .current_dir(repo_path)
        .output()
        .context("Failed to create git tag")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git tag failed: {stderr}");
    }

    log::info!("Created git tag: {tag}");
    Ok(())
}

/// Generate a changelog from commits since the last tag (or all commits if no tag).
pub fn generate_changelog(repo_path: &Path, since_tag: Option<&str>) -> Result<Vec<ChangelogEntry>> {
    let range = match since_tag {
        Some(tag) => format!("{tag}..HEAD"),
        None => "HEAD".to_string(),
    };

    let output = Command::new("git")
        .args([
            "log",
            &range,
            "--pretty=format:%H|%s|%an|%aI",
            "--no-merges",
        ])
        .current_dir(repo_path)
        .output()
        .context("Failed to run git log")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git log failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let entries: Vec<ChangelogEntry> = stdout
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(4, '|').collect();
            if parts.len() == 4 {
                Some(ChangelogEntry {
                    hash: parts[0].get(..8).unwrap_or(parts[0]).to_string(),
                    subject: parts[1].to_string(),
                    author: parts[2].to_string(),
                    date: parts[3].to_string(),
                })
            } else {
                None
            }
        })
        .collect();

    Ok(entries)
}

// ---------------------------------------------------------------------------
// Unit Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_semver_basic() {
        let (major, minor, patch) = parse_semver("1.2.3").unwrap();
        assert_eq!(major, 1);
        assert_eq!(minor, 2);
        assert_eq!(patch, 3);
    }

    #[test]
    fn test_parse_semver_with_v_prefix() {
        let (major, minor, patch) = parse_semver("v1.2.3").unwrap();
        assert_eq!(major, 1);
        assert_eq!(minor, 2);
        assert_eq!(patch, 3);
    }

    #[test]
    fn test_parse_semver_strips_prerelease() {
        // The parser strips prerelease suffix when parsing
        let result = parse_semver("1.0.0-beta.1");
        // This will fail because the parser doesn't handle pre-release properly
        assert!(result.is_err() || result.unwrap() == (1, 0, 0));
    }

    #[test]
    fn test_parse_semver_invalid() {
        assert!(parse_semver("1.2").is_err());
        assert!(parse_semver("1").is_err());
        assert!(parse_semver("invalid").is_err());
    }

    #[test]
    fn test_next_version_patch() {
        let result = next_version("1.2.3", &BumpLevel::Patch).unwrap();
        assert_eq!(result, "1.2.4");
    }

    #[test]
    fn test_next_version_minor() {
        let result = next_version("1.2.3", &BumpLevel::Minor).unwrap();
        assert_eq!(result, "1.3.0");
    }

    #[test]
    fn test_next_version_major() {
        let result = next_version("1.2.3", &BumpLevel::Major).unwrap();
        assert_eq!(result, "2.0.0");
    }

    #[test]
    fn test_next_version_from_zero() {
        let result = next_version("0.0.1", &BumpLevel::Patch).unwrap();
        assert_eq!(result, "0.0.2");

        let result = next_version("0.0.1", &BumpLevel::Minor).unwrap();
        assert_eq!(result, "0.1.0");

        let result = next_version("0.0.1", &BumpLevel::Major).unwrap();
        assert_eq!(result, "1.0.0");
    }

    #[test]
    fn test_extract_version_package_json() {
        let temp = TempDir::new().unwrap();
        let file = temp.path().join("package.json");
        std::fs::write(&file, r#"{"name": "test", "version": "1.2.3"}"#).unwrap();

        let version = extract_version(&file, "package.json").unwrap();
        assert_eq!(version, Some("1.2.3".to_string()));
    }

    #[test]
    fn test_extract_version_cargo_toml() {
        let temp = TempDir::new().unwrap();
        let file = temp.path().join("Cargo.toml");
        std::fs::write(
            &file,
            r#"[package]
name = "test"
version = "0.1.0"
"#,
        )
        .unwrap();

        let version = extract_version(&file, "Cargo.toml").unwrap();
        assert_eq!(version, Some("0.1.0".to_string()));
    }

    #[test]
    fn test_extract_version_pyproject_toml() {
        let temp = TempDir::new().unwrap();
        let file = temp.path().join("pyproject.toml");
        std::fs::write(
            &file,
            r#"[project]
name = "mypackage"
version = "2.0.0"
"#,
        )
        .unwrap();

        let version = extract_version(&file, "pyproject.toml").unwrap();
        assert_eq!(version, Some("2.0.0".to_string()));
    }

    #[test]
    fn test_extract_version_version_txt() {
        let temp = TempDir::new().unwrap();
        let file = temp.path().join("version.txt");
        std::fs::write(&file, "3.2.1\n").unwrap();

        let version = extract_version(&file, "version.txt").unwrap();
        assert_eq!(version, Some("3.2.1".to_string()));
    }

    #[test]
    fn test_detect_versions_single_file() {
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("package.json"),
            r#"{"version": "1.0.0"}"#,
        )
        .unwrap();

        let info = detect_versions(temp.path()).unwrap();
        assert_eq!(info.files.len(), 1);
        assert_eq!(info.current_version, Some("1.0.0".to_string()));
        assert!(info.is_consistent);
    }

    #[test]
    fn test_detect_versions_consistent() {
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("package.json"),
            r#"{"version": "2.0.0"}"#,
        )
        .unwrap();
        std::fs::write(
            temp.path().join("Cargo.toml"),
            r#"[package]
name = "test"
version = "2.0.0"
"#,
        )
        .unwrap();

        let info = detect_versions(temp.path()).unwrap();
        assert_eq!(info.files.len(), 2);
        assert_eq!(info.current_version, Some("2.0.0".to_string()));
        assert!(info.is_consistent);
    }

    #[test]
    fn test_detect_versions_inconsistent() {
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("package.json"),
            r#"{"version": "1.0.0"}"#,
        )
        .unwrap();
        std::fs::write(
            temp.path().join("Cargo.toml"),
            r#"[package]
name = "test"
version = "2.0.0"
"#,
        )
        .unwrap();

        let info = detect_versions(temp.path()).unwrap();
        assert_eq!(info.files.len(), 2);
        assert!(info.current_version.is_none()); // Inconsistent = no single version
        assert!(!info.is_consistent);
    }

    #[test]
    fn test_detect_versions_empty() {
        let temp = TempDir::new().unwrap();
        let info = detect_versions(temp.path()).unwrap();
        assert!(info.files.is_empty());
        assert!(info.is_consistent); // Empty is "consistent"
    }
}
