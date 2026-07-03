//! Version bump and changelog types.

#[allow(unused_imports)]
use super::*;
use serde::{Deserialize, Serialize};

/// Semver bump level.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BumpLevel {
    Patch,
    Minor,
    Major,
}

/// A file that contains a version string.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionFile {
    /// Relative path from repo root (e.g. "package.json").
    pub path: String,
    /// The current version found in this file.
    pub version: String,
}

/// Result of scanning a repo for version files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    /// All files that contain a version string.
    pub files: Vec<VersionFile>,
    /// The resolved "current" version (highest found, or None if inconsistent).
    pub current_version: Option<String>,
    /// Whether all version files agree on the same version.
    pub is_consistent: bool,
    /// Latest git tag matching vX.Y.Z or X.Y.Z pattern.
    pub latest_tag: Option<String>,
}

/// Result of a version bump operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BumpResult {
    /// Previous version.
    pub old_version: String,
    /// New version.
    pub new_version: String,
    /// Files that were updated.
    pub updated_files: Vec<String>,
    /// Git tag that was created (if tagging was requested).
    pub git_tag: Option<String>,
}

/// A single changelog entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogEntry {
    pub hash: String,
    pub subject: String,
    pub author: String,
    pub date: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bump_level_serialization() {
        let levels = vec![
            (BumpLevel::Patch, "patch"),
            (BumpLevel::Minor, "minor"),
            (BumpLevel::Major, "major"),
        ];

        for (level, expected) in levels {
            let json = serde_json::to_string(&level).unwrap();
            assert_eq!(json, format!("\"{}\"", expected));
        }
    }
}
