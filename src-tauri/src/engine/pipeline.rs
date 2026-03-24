use crate::engine::models::{EnvironmentsConfig, Pipeline, SecretsConfig};
use anyhow::{Context, Result};
use std::path::Path;

/// Serialize a Pipeline to TOML and write it to .chibby/pipeline.toml.
pub fn save_pipeline(repo_path: &Path, pipeline: &Pipeline) -> Result<()> {
    let chibby_dir = repo_path.join(".chibby");
    std::fs::create_dir_all(&chibby_dir)
        .with_context(|| format!("Failed to create .chibby directory in {}", repo_path.display()))?;

    let toml_str = toml::to_string_pretty(pipeline)
        .context("Failed to serialize pipeline to TOML")?;

    let file_path = chibby_dir.join("pipeline.toml");
    std::fs::write(&file_path, &toml_str)
        .with_context(|| format!("Failed to write {}", file_path.display()))?;

    log::info!("Saved pipeline to {}", file_path.display());
    Ok(())
}

/// Load a Pipeline from .chibby/pipeline.toml.
pub fn load_pipeline(repo_path: &Path) -> Result<Pipeline> {
    let file_path = repo_path.join(".chibby").join("pipeline.toml");
    let content = std::fs::read_to_string(&file_path)
        .with_context(|| format!("Failed to read {}", file_path.display()))?;

    let pipeline: Pipeline = toml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", file_path.display()))?;

    Ok(pipeline)
}

/// Check whether a .chibby/pipeline.toml exists for a repo.
pub fn has_pipeline(repo_path: &Path) -> bool {
    repo_path.join(".chibby").join("pipeline.toml").exists()
}

/// List all pipeline TOML files in .chibby/ directory.
/// Returns a list of file stems (e.g. ["pipeline", "release"]).
pub fn list_pipelines(repo_path: &Path) -> Vec<String> {
    let chibby_dir = repo_path.join(".chibby");
    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&chibby_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                // Skip non-pipeline config files
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if stem == "environments" || stem == "secrets" {
                    continue;
                }
                // Try to parse as a pipeline to confirm it's valid
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if toml::from_str::<Pipeline>(&content).is_ok() {
                        names.push(stem.to_string());
                    }
                }
            }
        }
    }
    // Sort with "pipeline" first, then alphabetically
    names.sort_by(|a, b| {
        if a == "pipeline" { std::cmp::Ordering::Less }
        else if b == "pipeline" { std::cmp::Ordering::Greater }
        else { a.cmp(b) }
    });
    names
}

/// Load a specific pipeline by file stem (e.g. "release" loads .chibby/release.toml).
pub fn load_pipeline_by_name(repo_path: &Path, name: &str) -> Result<Pipeline> {
    let file_path = repo_path.join(".chibby").join(format!("{}.toml", name));
    let content = std::fs::read_to_string(&file_path)
        .with_context(|| format!("Failed to read {}", file_path.display()))?;
    let pipeline: Pipeline = toml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", file_path.display()))?;
    Ok(pipeline)
}

/// Save a pipeline to a specific file in .chibby/ (e.g. "release" saves to .chibby/release.toml).
pub fn save_pipeline_by_name(repo_path: &Path, name: &str, pipeline: &Pipeline) -> Result<()> {
    let chibby_dir = repo_path.join(".chibby");
    std::fs::create_dir_all(&chibby_dir)
        .with_context(|| format!("Failed to create .chibby directory in {}", repo_path.display()))?;
    let toml_str = toml::to_string_pretty(pipeline)
        .context("Failed to serialize pipeline to TOML")?;
    let file_path = chibby_dir.join(format!("{}.toml", name));
    std::fs::write(&file_path, &toml_str)
        .with_context(|| format!("Failed to write {}", file_path.display()))?;
    log::info!("Saved pipeline to {}", file_path.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// Environments persistence (.chibby/environments.toml)
// ---------------------------------------------------------------------------

/// Save environments config to .chibby/environments.toml.
pub fn save_environments(repo_path: &Path, config: &EnvironmentsConfig) -> Result<()> {
    let chibby_dir = repo_path.join(".chibby");
    std::fs::create_dir_all(&chibby_dir)
        .with_context(|| format!("Failed to create .chibby directory in {}", repo_path.display()))?;

    let toml_str = toml::to_string_pretty(config)
        .context("Failed to serialize environments to TOML")?;

    let file_path = chibby_dir.join("environments.toml");
    std::fs::write(&file_path, &toml_str)
        .with_context(|| format!("Failed to write {}", file_path.display()))?;

    log::info!("Saved environments to {}", file_path.display());
    Ok(())
}

/// Load environments config from .chibby/environments.toml.
pub fn load_environments(repo_path: &Path) -> Result<EnvironmentsConfig> {
    let file_path = repo_path.join(".chibby").join("environments.toml");
    if !file_path.exists() {
        return Ok(EnvironmentsConfig {
            environments: Vec::new(),
        });
    }
    let content = std::fs::read_to_string(&file_path)
        .with_context(|| format!("Failed to read {}", file_path.display()))?;

    let config: EnvironmentsConfig = toml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", file_path.display()))?;

    Ok(config)
}

/// Check whether a .chibby/environments.toml exists.
pub fn has_environments(repo_path: &Path) -> bool {
    repo_path.join(".chibby").join("environments.toml").exists()
}

// ---------------------------------------------------------------------------
// Secrets config persistence (.chibby/secrets.toml — values never stored)
// ---------------------------------------------------------------------------

/// Save secrets config to .chibby/secrets.toml.
pub fn save_secrets_config(repo_path: &Path, config: &SecretsConfig) -> Result<()> {
    let chibby_dir = repo_path.join(".chibby");
    std::fs::create_dir_all(&chibby_dir)
        .with_context(|| format!("Failed to create .chibby directory in {}", repo_path.display()))?;

    let toml_str = toml::to_string_pretty(config)
        .context("Failed to serialize secrets config to TOML")?;

    let file_path = chibby_dir.join("secrets.toml");
    std::fs::write(&file_path, &toml_str)
        .with_context(|| format!("Failed to write {}", file_path.display()))?;

    log::info!("Saved secrets config to {}", file_path.display());
    Ok(())
}

/// Load secrets config from .chibby/secrets.toml.
pub fn load_secrets_config(repo_path: &Path) -> Result<SecretsConfig> {
    let file_path = repo_path.join(".chibby").join("secrets.toml");
    if !file_path.exists() {
        return Ok(SecretsConfig {
            secrets: Vec::new(),
        });
    }
    let content = std::fs::read_to_string(&file_path)
        .with_context(|| format!("Failed to read {}", file_path.display()))?;

    let config: SecretsConfig = toml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", file_path.display()))?;

    Ok(config)
}

/// Check whether a .chibby/secrets.toml exists.
pub fn has_secrets_config(repo_path: &Path) -> bool {
    repo_path.join(".chibby").join("secrets.toml").exists()
}

// ---------------------------------------------------------------------------
// Unit Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::models::{Backend, Environment, SecretRef, Stage};
    use tempfile::TempDir;

    fn sample_pipeline() -> Pipeline {
        Pipeline {
            name: "Test Pipeline".to_string(),
            stages: vec![
                Stage {
                    name: "build".to_string(),
                    commands: vec!["npm run build".to_string()],
                    backend: Backend::Local,
                    working_dir: None,
                    fail_fast: true,
                    health_check: None,
                },
                Stage {
                    name: "test".to_string(),
                    commands: vec!["npm test".to_string()],
                    backend: Backend::Local,
                    working_dir: None,
                    fail_fast: true,
                    health_check: None,
                },
            ],
        }
    }

    #[test]
    fn test_save_and_load_pipeline() {
        let temp = TempDir::new().unwrap();
        let pipeline = sample_pipeline();

        // Save
        save_pipeline(temp.path(), &pipeline).unwrap();
        assert!(has_pipeline(temp.path()));

        // Load
        let loaded = load_pipeline(temp.path()).unwrap();
        assert_eq!(loaded.name, "Test Pipeline");
        assert_eq!(loaded.stages.len(), 2);
        assert_eq!(loaded.stages[0].name, "build");
        assert_eq!(loaded.stages[1].name, "test");
    }

    #[test]
    fn test_has_pipeline_false() {
        let temp = TempDir::new().unwrap();
        assert!(!has_pipeline(temp.path()));
    }

    #[test]
    fn test_load_pipeline_missing() {
        let temp = TempDir::new().unwrap();
        let result = load_pipeline(temp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_pipeline_creates_chibby_dir() {
        let temp = TempDir::new().unwrap();
        let chibby_dir = temp.path().join(".chibby");
        assert!(!chibby_dir.exists());

        save_pipeline(temp.path(), &sample_pipeline()).unwrap();
        assert!(chibby_dir.exists());
    }

    #[test]
    fn test_save_and_load_environments() {
        let temp = TempDir::new().unwrap();

        let config = EnvironmentsConfig {
            environments: vec![
                Environment {
                    name: "production".to_string(),
                    ssh_host: Some("prod.example.com".to_string()),
                    ssh_port: Some(22),
                    variables: [("APP_ENV".to_string(), "production".to_string())]
                        .into_iter()
                        .collect(),
                },
                Environment {
                    name: "staging".to_string(),
                    ssh_host: Some("staging.example.com".to_string()),
                    ssh_port: None,
                    variables: std::collections::HashMap::new(),
                },
            ],
        };

        save_environments(temp.path(), &config).unwrap();
        assert!(has_environments(temp.path()));

        let loaded = load_environments(temp.path()).unwrap();
        assert_eq!(loaded.environments.len(), 2);
        assert_eq!(loaded.environments[0].name, "production");
        assert_eq!(loaded.environments[1].name, "staging");
    }

    #[test]
    fn test_load_environments_missing() {
        let temp = TempDir::new().unwrap();

        // Should return empty config, not error
        let config = load_environments(temp.path()).unwrap();
        assert!(config.environments.is_empty());
    }

    #[test]
    fn test_has_environments_false() {
        let temp = TempDir::new().unwrap();
        assert!(!has_environments(temp.path()));
    }

    #[test]
    fn test_save_and_load_secrets_config() {
        let temp = TempDir::new().unwrap();

        let config = SecretsConfig {
            secrets: vec![
                SecretRef {
                    name: "API_KEY".to_string(),
                    environments: vec!["production".to_string()],
                },
                SecretRef {
                    name: "OPTIONAL_TOKEN".to_string(),
                    environments: vec![],
                },
            ],
        };

        save_secrets_config(temp.path(), &config).unwrap();
        assert!(has_secrets_config(temp.path()));

        let loaded = load_secrets_config(temp.path()).unwrap();
        assert_eq!(loaded.secrets.len(), 2);
        assert_eq!(loaded.secrets[0].name, "API_KEY");
        assert_eq!(loaded.secrets[0].environments, vec!["production".to_string()]);
    }

    #[test]
    fn test_load_secrets_config_missing() {
        let temp = TempDir::new().unwrap();

        // Should return empty config, not error
        let config = load_secrets_config(temp.path()).unwrap();
        assert!(config.secrets.is_empty());
    }

    #[test]
    fn test_has_secrets_config_false() {
        let temp = TempDir::new().unwrap();
        assert!(!has_secrets_config(temp.path()));
    }

    #[test]
    fn test_pipeline_roundtrip_preserves_all_fields() {
        let temp = TempDir::new().unwrap();

        let pipeline = Pipeline {
            name: "Complex Pipeline".to_string(),
            stages: vec![Stage {
                name: "deploy".to_string(),
                commands: vec![
                    "docker build -t app .".to_string(),
                    "docker push app".to_string(),
                ],
                backend: Backend::Ssh,
                working_dir: Some("./services".to_string()),
                fail_fast: false,
                health_check: None,
            }],
        };

        save_pipeline(temp.path(), &pipeline).unwrap();
        let loaded = load_pipeline(temp.path()).unwrap();

        assert_eq!(loaded.stages[0].backend, Backend::Ssh);
        assert_eq!(loaded.stages[0].working_dir, Some("./services".to_string()));
        assert!(!loaded.stages[0].fail_fast);
        assert_eq!(loaded.stages[0].commands.len(), 2);
    }
}
