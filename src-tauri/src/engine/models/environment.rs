//! Environment, secret, and deployment configuration types.

#[allow(unused_imports)]
use super::*;
use serde::{Deserialize, Serialize};

/// A deployment target environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Environment {
    /// Environment name (e.g. "production", "staging").
    pub name: String,
    /// SSH host for remote stages (user@host).
    #[serde(default)]
    pub ssh_host: Option<String>,
    /// SSH port override.
    #[serde(default)]
    pub ssh_port: Option<u16>,
    /// Environment variables for this target.
    #[serde(default)]
    pub variables: std::collections::HashMap<String, String>,
}

/// Top-level environments file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentsConfig {
    #[serde(default, alias = "environment")]
    pub environments: Vec<Environment>,
}

// ---------------------------------------------------------------------------
// Deployment configuration (used during project creation)
// ---------------------------------------------------------------------------

/// The deployment method to use for CD stages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentMethod {
    /// Parse from GitHub Actions deploy workflows
    AutoDetect,

    // Docker-based
    /// Docker Compose over SSH
    DockerComposeSsh,
    /// Build/push to registry, then pull on server
    DockerRegistry,

    // Package publishing (libraries/CLIs)
    /// Publish to crates.io
    CargoPublish,
    /// Publish to npm registry
    NpmPublish,

    // Release artifacts
    /// Create GitHub release with binaries
    GithubRelease,

    // Direct deploy
    /// rsync/scp files to server
    SshRsync,

    // Platform-as-a-Service
    /// Deploy to Fly.io
    Flyio,
    /// Deploy to Render
    Render,
    /// Deploy to Railway
    Railway,

    // Static sites
    /// Deploy to Netlify
    Netlify,
    /// Deploy to Vercel
    Vercel,
    /// Deploy to S3 bucket
    S3Static,

    // Skip
    /// No deployment (CI only)
    Skip,
}

impl Default for DeploymentMethod {
    fn default() -> Self {
        Self::Skip
    }
}

/// Configuration for deployment during project creation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeploymentConfig {
    /// The deployment method to use.
    pub method: DeploymentMethod,
    /// Target environment name (e.g., "production", "staging").
    #[serde(default)]
    pub environment_name: Option<String>,
    /// SSH host for SSH-based deploys (user@hostname).
    #[serde(default)]
    pub ssh_host: Option<String>,
    /// Docker registry URL (e.g., "ghcr.io/username").
    #[serde(default)]
    pub docker_registry: Option<String>,
    /// Health check URL path (e.g., "/health").
    #[serde(default)]
    pub health_check_url: Option<String>,
    /// Docker Compose file to use (e.g., "docker-compose.prod.yml").
    #[serde(default)]
    pub compose_file: Option<String>,
    /// Platform project name for PaaS (fly app name, etc.).
    #[serde(default)]
    pub platform_project: Option<String>,
    /// Whether to run dry-run first for package publishing.
    #[serde(default = "default_true")]
    pub dry_run_first: bool,
}

// ---------------------------------------------------------------------------
// Secret references (stored as .chibby/secrets.toml — values never in file)
// ---------------------------------------------------------------------------

/// A named secret reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretRef {
    /// Secret name (e.g. "DEPLOY_TOKEN").
    pub name: String,
    /// Which environment(s) this secret applies to. Empty = all.
    #[serde(default)]
    pub environments: Vec<String>,
}

/// Top-level secrets file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretsConfig {
    #[serde(default, alias = "secret")]
    pub secrets: Vec<SecretRef>,
}

// ---------------------------------------------------------------------------
// Run history (persisted in app data directory)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_serialization() {
        let env = Environment {
            name: "production".to_string(),
            ssh_host: Some("user@server.com".to_string()),
            ssh_port: Some(22),
            variables: [("APP_ENV".to_string(), "production".to_string())]
                .into_iter()
                .collect(),
        };

        let json = serde_json::to_string(&env).unwrap();
        assert!(json.contains("production"));
        assert!(json.contains("user@server.com"));
    }
}
