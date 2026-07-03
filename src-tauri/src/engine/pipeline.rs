use crate::engine::leak_scanner::{self, LeakMatch};
use crate::engine::models::{Environment, EnvironmentsConfig, Pipeline, SecretRef, SecretsConfig};
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::path::Path;

/// Serialize a Pipeline to TOML and write it to .chibby/pipeline.toml.
pub fn save_pipeline(repo_path: &Path, pipeline: &Pipeline) -> Result<()> {
    let chibby_dir = repo_path.join(".chibby");
    std::fs::create_dir_all(&chibby_dir).with_context(|| {
        format!(
            "Failed to create .chibby directory in {}",
            repo_path.display()
        )
    })?;

    let toml_str =
        toml::to_string_pretty(pipeline).context("Failed to serialize pipeline to TOML")?;

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
        if a == "pipeline" {
            std::cmp::Ordering::Less
        } else if b == "pipeline" {
            std::cmp::Ordering::Greater
        } else {
            a.cmp(b)
        }
    });
    names
}

/// Validate that a pipeline name is safe (no path traversal or special characters).
fn validate_pipeline_name(name: &str) -> Result<()> {
    if name.is_empty() {
        anyhow::bail!("Pipeline name cannot be empty");
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        anyhow::bail!("Pipeline name contains invalid path characters");
    }
    if name.starts_with('.') || name.starts_with('-') {
        anyhow::bail!("Pipeline name cannot start with '.' or '-'");
    }
    // Allow only alphanumeric, dash, underscore
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        anyhow::bail!(
            "Pipeline name may only contain alphanumeric characters, dashes, and underscores"
        );
    }
    Ok(())
}

/// Load a specific pipeline by file stem (e.g. "release" loads .chibby/release.toml).
pub fn load_pipeline_by_name(repo_path: &Path, name: &str) -> Result<Pipeline> {
    validate_pipeline_name(name)?;
    let file_path = repo_path.join(".chibby").join(format!("{}.toml", name));
    let content = std::fs::read_to_string(&file_path)
        .with_context(|| format!("Failed to read {}", file_path.display()))?;
    let pipeline: Pipeline = toml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", file_path.display()))?;
    Ok(pipeline)
}

/// Save a pipeline to a specific file in .chibby/ (e.g. "release" saves to .chibby/release.toml).
pub fn save_pipeline_by_name(repo_path: &Path, name: &str, pipeline: &Pipeline) -> Result<()> {
    validate_pipeline_name(name)?;
    let chibby_dir = repo_path.join(".chibby");
    std::fs::create_dir_all(&chibby_dir).with_context(|| {
        format!(
            "Failed to create .chibby directory in {}",
            repo_path.display()
        )
    })?;
    let toml_str =
        toml::to_string_pretty(pipeline).context("Failed to serialize pipeline to TOML")?;
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
    std::fs::create_dir_all(&chibby_dir).with_context(|| {
        format!(
            "Failed to create .chibby directory in {}",
            repo_path.display()
        )
    })?;

    let toml_str =
        toml::to_string_pretty(config).context("Failed to serialize environments to TOML")?;

    // Pre-save warning: detect variable *values* that look like real
    // credentials. We never block the save — the leak might be intentional
    // and the user can act on the warning. Gating belongs in gates.rs.
    let hits = scan_environments_for_leaks_in_config(config);
    if !hits.is_empty() {
        log::warn!(
            "environments.toml contains {} value(s) that look like real credentials. \
             Consider declaring them in secrets.toml instead. Run `chibby env scan-leaks` for details.",
            hits.len()
        );
    }

    let file_path = chibby_dir.join("environments.toml");
    std::fs::write(&file_path, &toml_str)
        .with_context(|| format!("Failed to write {}", file_path.display()))?;

    log::info!("Saved environments to {}", file_path.display());
    Ok(())
}

/// Scan all variable values in an environments config for token-shaped strings.
/// Returns per-variable hits.
pub fn scan_environments_for_leaks_in_config(config: &EnvironmentsConfig) -> Vec<EnvLeakHit> {
    let mut hits = Vec::new();
    for env in &config.environments {
        for (key, value) in &env.variables {
            for m in leak_scanner::scan(value) {
                hits.push(EnvLeakHit {
                    env: env.name.clone(),
                    variable: key.clone(),
                    match_: m,
                });
            }
        }
    }
    hits
}

/// Load environments.toml (committed file only) and scan for leaked credentials.
pub fn scan_environments_for_leaks(repo_path: &Path) -> Result<Vec<EnvLeakHit>> {
    let config = load_environments(repo_path)?;
    Ok(scan_environments_for_leaks_in_config(&config))
}

/// A single token-shaped value found inside an environments.toml variable.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EnvLeakHit {
    pub env: String,
    pub variable: String,
    #[serde(flatten)]
    pub match_: LeakMatch,
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

/// Load the per-developer override file `.chibby/environments.local.toml`.
/// Returns an empty config when absent.
pub fn load_environments_local(repo_path: &Path) -> Result<EnvironmentsConfig> {
    let file_path = repo_path.join(".chibby").join("environments.local.toml");
    if !file_path.exists() {
        return Ok(EnvironmentsConfig {
            environments: Vec::new(),
        });
    }
    let content = std::fs::read_to_string(&file_path)
        .with_context(|| format!("Failed to read {}", file_path.display()))?;
    toml::from_str(&content).with_context(|| format!("Failed to parse {}", file_path.display()))
}

/// Save the per-developer override file `.chibby/environments.local.toml`.
/// Ensures `.gitignore` is updated to keep this file out of git.
pub fn save_environments_local(repo_path: &Path, config: &EnvironmentsConfig) -> Result<()> {
    let chibby_dir = repo_path.join(".chibby");
    std::fs::create_dir_all(&chibby_dir).with_context(|| {
        format!(
            "Failed to create .chibby directory in {}",
            repo_path.display()
        )
    })?;
    let toml_str =
        toml::to_string_pretty(config).context("Failed to serialize environments.local to TOML")?;
    let file_path = chibby_dir.join("environments.local.toml");
    std::fs::write(&file_path, &toml_str)
        .with_context(|| format!("Failed to write {}", file_path.display()))?;
    ensure_gitignore_entries(repo_path)?;
    log::info!("Saved environments.local to {}", file_path.display());
    Ok(())
}

/// Check whether `.chibby/environments.local.toml` exists.
pub fn has_environments_local(repo_path: &Path) -> bool {
    repo_path
        .join(".chibby")
        .join("environments.local.toml")
        .exists()
}

/// Load environments with per-developer overrides applied.
///
/// `environments.toml` (committed) is the base; `environments.local.toml`
/// (gitignored) is layered on top using `merge_environments`.
pub fn load_environments_layered(repo_path: &Path) -> Result<EnvironmentsConfig> {
    let base = load_environments(repo_path)?;
    let local = load_environments_local(repo_path)?;
    Ok(merge_environments(base, local))
}

/// Pure merge function — local overrides base by environment name.
/// For matched envs: ssh_host/ssh_port use local-if-Some-else-base; variables
/// are key-merged (local wins on collision). Envs only in local are appended.
pub fn merge_environments(
    mut base: EnvironmentsConfig,
    local: EnvironmentsConfig,
) -> EnvironmentsConfig {
    for local_env in local.environments {
        if let Some(existing) = base
            .environments
            .iter_mut()
            .find(|e| e.name == local_env.name)
        {
            if local_env.ssh_host.is_some() {
                existing.ssh_host = local_env.ssh_host;
            }
            if local_env.ssh_port.is_some() {
                existing.ssh_port = local_env.ssh_port;
            }
            for (k, v) in local_env.variables {
                existing.variables.insert(k, v);
            }
        } else {
            base.environments.push(local_env);
        }
    }
    base
}

// ---------------------------------------------------------------------------
// Granular environment helpers (preserve other entries — surgical edits)
// ---------------------------------------------------------------------------

/// Append a new environment to `.chibby/environments.toml`.
/// Errors if an environment with the same name already exists.
pub fn add_environment(repo_path: &Path, env: Environment) -> Result<()> {
    let mut config = load_environments(repo_path)?;
    if config.environments.iter().any(|e| e.name == env.name) {
        return Err(anyhow!(
            "Environment '{}' already exists in environments.toml",
            env.name
        ));
    }
    config.environments.push(env);
    save_environments(repo_path, &config)
}

/// Remove an environment by name from `.chibby/environments.toml`.
/// Errors if the environment does not exist.
pub fn remove_environment(repo_path: &Path, name: &str) -> Result<()> {
    let mut config = load_environments(repo_path)?;
    let before = config.environments.len();
    config.environments.retain(|e| e.name != name);
    if config.environments.len() == before {
        return Err(anyhow!("Environment '{}' not found", name));
    }
    save_environments(repo_path, &config)
}

/// Set a variable on an environment, creating the env if missing.
pub fn set_env_variable(repo_path: &Path, env_name: &str, key: &str, value: &str) -> Result<()> {
    if !is_valid_env_var_name(key) {
        return Err(anyhow!(
            "Invalid variable name '{}': must match [A-Za-z_][A-Za-z0-9_]*",
            key
        ));
    }
    let mut config = load_environments(repo_path)?;
    if let Some(env) = config.environments.iter_mut().find(|e| e.name == env_name) {
        env.variables.insert(key.to_string(), value.to_string());
    } else {
        let mut variables = HashMap::new();
        variables.insert(key.to_string(), value.to_string());
        config.environments.push(Environment {
            name: env_name.to_string(),
            ssh_host: None,
            ssh_port: None,
            variables,
        });
    }
    save_environments(repo_path, &config)
}

/// Remove a variable from an environment. No-op (Ok) if env or key missing.
pub fn remove_env_variable(repo_path: &Path, env_name: &str, key: &str) -> Result<()> {
    let mut config = load_environments(repo_path)?;
    if let Some(env) = config.environments.iter_mut().find(|e| e.name == env_name) {
        env.variables.remove(key);
    }
    save_environments(repo_path, &config)
}

/// Validate that an environment variable name is safe for shell use.
/// Mirrors the rule enforced by the executor.
fn is_valid_env_var_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let first = name.as_bytes()[0];
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return false;
    }
    name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

// ---------------------------------------------------------------------------
// Secrets config persistence (.chibby/secrets.toml — values never stored)
// ---------------------------------------------------------------------------

/// Save secrets config to .chibby/secrets.toml.
pub fn save_secrets_config(repo_path: &Path, config: &SecretsConfig) -> Result<()> {
    let chibby_dir = repo_path.join(".chibby");
    std::fs::create_dir_all(&chibby_dir).with_context(|| {
        format!(
            "Failed to create .chibby directory in {}",
            repo_path.display()
        )
    })?;

    let toml_str =
        toml::to_string_pretty(config).context("Failed to serialize secrets config to TOML")?;

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
// Granular secrets-config helpers
// ---------------------------------------------------------------------------

/// Append a new secret reference. Errors if one with the same name exists.
pub fn add_secret_ref(repo_path: &Path, secret: SecretRef) -> Result<()> {
    let mut config = load_secrets_config(repo_path)?;
    if config.secrets.iter().any(|s| s.name == secret.name) {
        return Err(anyhow!(
            "Secret reference '{}' already exists in secrets.toml",
            secret.name
        ));
    }
    config.secrets.push(secret);
    save_secrets_config(repo_path, &config)
}

/// Remove a secret reference by name.
pub fn remove_secret_ref(repo_path: &Path, name: &str) -> Result<()> {
    let mut config = load_secrets_config(repo_path)?;
    let before = config.secrets.len();
    config.secrets.retain(|s| s.name != name);
    if config.secrets.len() == before {
        return Err(anyhow!("Secret reference '{}' not found", name));
    }
    save_secrets_config(repo_path, &config)
}

// ---------------------------------------------------------------------------
// .gitignore management
// ---------------------------------------------------------------------------

const GITIGNORE_MARKER: &str = "# Chibby — local overrides (never commit)";
const GITIGNORE_LINES: &[&str] = &[
    ".chibby/environments.local.toml",
    ".chibby/secrets.local.toml",
];

/// Ensure the repo's `.gitignore` contains entries for Chibby-managed local
/// override files. Idempotent — only appends if entries are missing.
///
/// Called automatically by `save_environments_local` and `save_secrets_config`.
pub fn ensure_gitignore_entries(repo_path: &Path) -> Result<()> {
    let gitignore_path = repo_path.join(".gitignore");
    let existing = std::fs::read_to_string(&gitignore_path).unwrap_or_default();

    let missing: Vec<&&str> = GITIGNORE_LINES
        .iter()
        .filter(|line| !existing.lines().any(|l| l.trim() == **line))
        .collect();

    if missing.is_empty() {
        return Ok(());
    }

    let mut new_content = existing.clone();
    if !new_content.is_empty() && !new_content.ends_with('\n') {
        new_content.push('\n');
    }
    if !new_content.contains(GITIGNORE_MARKER) {
        new_content.push('\n');
        new_content.push_str(GITIGNORE_MARKER);
        new_content.push('\n');
    }
    for line in missing {
        new_content.push_str(line);
        new_content.push('\n');
    }
    std::fs::write(&gitignore_path, new_content)
        .with_context(|| format!("Failed to update {}", gitignore_path.display()))?;
    log::info!("Updated {} with Chibby entries", gitignore_path.display());
    Ok(())
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
        assert_eq!(
            loaded.secrets[0].environments,
            vec!["production".to_string()]
        );
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

    // -----------------------------------------------------------------------
    // Layered environments + granular helpers + gitignore (Iteration 1)
    // -----------------------------------------------------------------------

    fn env_with_vars(name: &str, vars: &[(&str, &str)]) -> Environment {
        Environment {
            name: name.to_string(),
            ssh_host: None,
            ssh_port: None,
            variables: vars
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }

    #[test]
    fn test_merge_environments_overrides_ssh_and_vars() {
        let base = EnvironmentsConfig {
            environments: vec![Environment {
                name: "production".to_string(),
                ssh_host: Some("base@host".to_string()),
                ssh_port: Some(22),
                variables: [("A".to_string(), "base".to_string())]
                    .into_iter()
                    .collect(),
            }],
        };
        let local = EnvironmentsConfig {
            environments: vec![Environment {
                name: "production".to_string(),
                ssh_host: Some("local@host".to_string()),
                ssh_port: None, // None means "don't override"
                variables: [
                    ("A".to_string(), "local".to_string()),
                    ("B".to_string(), "new".to_string()),
                ]
                .into_iter()
                .collect(),
            }],
        };
        let merged = merge_environments(base, local);
        assert_eq!(merged.environments.len(), 1);
        let env = &merged.environments[0];
        assert_eq!(env.ssh_host.as_deref(), Some("local@host"));
        assert_eq!(env.ssh_port, Some(22)); // base preserved
        assert_eq!(env.variables.get("A").map(String::as_str), Some("local"));
        assert_eq!(env.variables.get("B").map(String::as_str), Some("new"));
    }

    #[test]
    fn test_merge_environments_appends_local_only_env() {
        let base = EnvironmentsConfig {
            environments: vec![env_with_vars("production", &[("A", "1")])],
        };
        let local = EnvironmentsConfig {
            environments: vec![env_with_vars("dev", &[("B", "2")])],
        };
        let merged = merge_environments(base, local);
        assert_eq!(merged.environments.len(), 2);
        assert!(merged.environments.iter().any(|e| e.name == "dev"));
    }

    #[test]
    fn test_load_environments_layered() {
        let temp = TempDir::new().unwrap();
        save_environments(
            temp.path(),
            &EnvironmentsConfig {
                environments: vec![env_with_vars("production", &[("HOST", "base")])],
            },
        )
        .unwrap();
        save_environments_local(
            temp.path(),
            &EnvironmentsConfig {
                environments: vec![env_with_vars("production", &[("HOST", "local")])],
            },
        )
        .unwrap();
        let layered = load_environments_layered(temp.path()).unwrap();
        assert_eq!(
            layered.environments[0].variables.get("HOST").unwrap(),
            "local"
        );
    }

    #[test]
    fn test_add_environment_rejects_duplicates() {
        let temp = TempDir::new().unwrap();
        add_environment(temp.path(), env_with_vars("production", &[])).unwrap();
        let err = add_environment(temp.path(), env_with_vars("production", &[])).unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn test_remove_environment_errors_when_missing() {
        let temp = TempDir::new().unwrap();
        let err = remove_environment(temp.path(), "ghost").unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_set_env_variable_creates_env_if_missing() {
        let temp = TempDir::new().unwrap();
        set_env_variable(temp.path(), "staging", "API_URL", "https://x").unwrap();
        let cfg = load_environments(temp.path()).unwrap();
        let env = cfg
            .environments
            .iter()
            .find(|e| e.name == "staging")
            .unwrap();
        assert_eq!(env.variables.get("API_URL").unwrap(), "https://x");
    }

    #[test]
    fn test_set_env_variable_rejects_invalid_name() {
        let temp = TempDir::new().unwrap();
        let err = set_env_variable(temp.path(), "prod", "1bad", "x").unwrap_err();
        assert!(err.to_string().contains("Invalid variable name"));
    }

    #[test]
    fn test_add_secret_ref_rejects_duplicates() {
        let temp = TempDir::new().unwrap();
        add_secret_ref(
            temp.path(),
            SecretRef {
                name: "API_KEY".to_string(),
                environments: vec![],
            },
        )
        .unwrap();
        let err = add_secret_ref(
            temp.path(),
            SecretRef {
                name: "API_KEY".to_string(),
                environments: vec![],
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn test_ensure_gitignore_creates_when_missing() {
        let temp = TempDir::new().unwrap();
        ensure_gitignore_entries(temp.path()).unwrap();
        let content = std::fs::read_to_string(temp.path().join(".gitignore")).unwrap();
        assert!(content.contains(".chibby/environments.local.toml"));
        assert!(content.contains(".chibby/secrets.local.toml"));
    }

    #[test]
    fn test_ensure_gitignore_appends_only_missing() {
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join(".gitignore"),
            "node_modules/\n.chibby/environments.local.toml\n",
        )
        .unwrap();
        ensure_gitignore_entries(temp.path()).unwrap();
        let content = std::fs::read_to_string(temp.path().join(".gitignore")).unwrap();
        // Should only add the missing one, not duplicate the existing entry
        let env_count = content.matches(".chibby/environments.local.toml").count();
        assert_eq!(env_count, 1);
        assert!(content.contains(".chibby/secrets.local.toml"));
        assert!(content.starts_with("node_modules/\n"));
    }

    #[test]
    fn test_save_environments_local_writes_gitignore() {
        let temp = TempDir::new().unwrap();
        save_environments_local(
            temp.path(),
            &EnvironmentsConfig {
                environments: vec![env_with_vars("dev", &[("X", "y")])],
            },
        )
        .unwrap();
        let gi = std::fs::read_to_string(temp.path().join(".gitignore")).unwrap();
        assert!(gi.contains(".chibby/environments.local.toml"));
    }
}
