//! Recognizes CI/CD config files the agent is allowed to edit, and validates
//! that candidate content is well-formed for the format before it is written.

use serde::Serialize;

/// A recognized CI/CD config format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CiFormat {
    Chibby,
    GithubActions,
    CircleCi,
    Drone,
    GitLab,
}

impl CiFormat {
    /// Canonical default file path for a config of this format — the single
    /// source of truth shared by CI editing and pipeline generation.
    pub fn default_path(&self) -> &'static str {
        match self {
            CiFormat::Chibby => ".chibby/pipeline.toml",
            CiFormat::GithubActions => ".github/workflows/ci.yml",
            CiFormat::CircleCi => ".circleci/config.yml",
            CiFormat::Drone => ".drone.yml",
            CiFormat::GitLab => ".gitlab-ci.yml",
        }
    }
}

/// Classify a project-relative path as an editable CI/CD file, if it is one.
pub fn is_ci_file(rel_path: &str) -> Option<CiFormat> {
    let p = rel_path.replace('\\', "/");
    let p = p.trim_start_matches("./");
    let is_yaml = p.ends_with(".yml") || p.ends_with(".yaml");

    if p.starts_with(".chibby/") && p.ends_with(".toml") {
        return Some(CiFormat::Chibby);
    }
    if p.starts_with(".github/workflows/") && is_yaml {
        return Some(CiFormat::GithubActions);
    }
    if p == ".circleci/config.yml" || p == ".circleci/config.yaml" {
        return Some(CiFormat::CircleCi);
    }
    if p == ".drone.yml" || p == ".drone.yaml" {
        return Some(CiFormat::Drone);
    }
    if p == ".gitlab-ci.yml" || p == ".gitlab-ci.yaml" {
        return Some(CiFormat::GitLab);
    }
    None
}

/// Validate that `content` parses as the format's syntax. Blocks writing a
/// broken config; deeper schema checks are layered on top by the pipeline
/// validator for Chibby files.
pub fn validate_content(format: CiFormat, content: &str) -> Result<(), String> {
    if content.trim().is_empty() {
        return Err("content is empty".to_string());
    }
    match format {
        CiFormat::Chibby => {
            toml::from_str::<toml::Value>(content).map_err(|e| format!("invalid TOML: {e}"))?;
        }
        _ => {
            serde_yaml::from_str::<serde_yaml::Value>(content)
                .map_err(|e| format!("invalid YAML: {e}"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_ci_files() {
        assert_eq!(is_ci_file(".chibby/pipeline.toml"), Some(CiFormat::Chibby));
        assert_eq!(is_ci_file(".chibby/release.toml"), Some(CiFormat::Chibby));
        assert_eq!(
            is_ci_file(".github/workflows/ci.yml"),
            Some(CiFormat::GithubActions)
        );
        assert_eq!(
            is_ci_file(".github/workflows/release.yaml"),
            Some(CiFormat::GithubActions)
        );
        assert_eq!(is_ci_file(".circleci/config.yml"), Some(CiFormat::CircleCi));
        assert_eq!(is_ci_file(".drone.yml"), Some(CiFormat::Drone));
        assert_eq!(is_ci_file(".gitlab-ci.yml"), Some(CiFormat::GitLab));
    }

    #[test]
    fn rejects_non_ci_files() {
        assert_eq!(is_ci_file("src/main.rs"), None);
        assert_eq!(is_ci_file("package.json"), None);
        assert_eq!(is_ci_file(".chibby/notes.txt"), None);
        assert_eq!(is_ci_file(".github/README.md"), None);
        assert_eq!(is_ci_file("random.yml"), None);
    }

    #[test]
    fn validates_syntax() {
        assert!(validate_content(CiFormat::Chibby, "name = \"x\"").is_ok());
        assert!(validate_content(CiFormat::Chibby, "name = = broken").is_err());
        assert!(validate_content(CiFormat::GithubActions, "on: push\njobs: {}").is_ok());
        assert!(validate_content(CiFormat::GithubActions, "key: value:\n  - bad: : :").is_err());
        assert!(validate_content(CiFormat::Drone, "").is_err());
    }
}
