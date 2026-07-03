//! Pipeline definition types (stored as .chibby/pipeline.toml).

#[allow(unused_imports)]
use super::*;
use serde::{Deserialize, Serialize};

/// The execution backend for a pipeline stage.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    Local,
    Ssh,
}

impl Default for Backend {
    fn default() -> Self {
        Self::Local
    }
}

/// A single stage in a pipeline (e.g. "build", "test", "deploy").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stage {
    /// Human-readable stage name.
    pub name: String,
    /// Ordered list of shell commands in the stage.
    pub commands: Vec<String>,
    /// Execution backend for this stage.
    #[serde(default)]
    pub backend: Backend,
    /// Working directory override (relative to repo root for local, absolute for SSH).
    #[serde(default)]
    pub working_dir: Option<String>,
    /// Whether failures in this stage should stop the pipeline.
    #[serde(default = "default_true")]
    pub fail_fast: bool,
    /// Optional health check to run after this stage completes.
    #[serde(default)]
    pub health_check: Option<HealthCheck>,
}

/// Health check configuration for post-deploy validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    /// Command to run (uses the same backend as the parent stage).
    pub command: String,
    /// Number of retries before declaring failure.
    #[serde(default = "default_retries")]
    pub retries: u32,
    /// Delay in seconds between retries.
    #[serde(default = "default_delay")]
    pub delay_secs: u32,
}

fn default_retries() -> u32 {
    3
}

fn default_delay() -> u32 {
    5
}

pub(crate) fn default_true() -> bool {
    true
}

/// Full pipeline definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    /// Display name for the pipeline.
    pub name: String,
    /// Ordered list of stages.
    pub stages: Vec<Stage>,
}

// ---------------------------------------------------------------------------
// Pipeline templates
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_default() {
        assert_eq!(Backend::default(), Backend::Local);
    }

    #[test]
    fn test_backend_serialization() {
        let local = Backend::Local;
        let ssh = Backend::Ssh;

        let local_json = serde_json::to_string(&local).unwrap();
        let ssh_json = serde_json::to_string(&ssh).unwrap();

        assert_eq!(local_json, r#""local""#);
        assert_eq!(ssh_json, r#""ssh""#);
    }

    #[test]
    fn test_backend_deserialization() {
        let local: Backend = serde_json::from_str(r#""local""#).unwrap();
        let ssh: Backend = serde_json::from_str(r#""ssh""#).unwrap();

        assert_eq!(local, Backend::Local);
        assert_eq!(ssh, Backend::Ssh);
    }

    #[test]
    fn test_stage_defaults() {
        let stage: Stage = serde_json::from_str(
            r#"{
            "name": "test",
            "commands": ["echo hello"]
        }"#,
        )
        .unwrap();

        assert_eq!(stage.backend, Backend::Local);
        assert!(stage.fail_fast);
        assert!(stage.working_dir.is_none());
        assert!(stage.health_check.is_none());
    }

    #[test]
    fn test_health_check_defaults() {
        let hc: HealthCheck = serde_json::from_str(
            r#"{
            "command": "curl http://localhost:8080/health"
        }"#,
        )
        .unwrap();

        assert_eq!(hc.retries, 3);
        assert_eq!(hc.delay_secs, 5);
    }

    #[test]
    fn test_pipeline_serialization_roundtrip() {
        let pipeline = Pipeline {
            name: "Test Pipeline".to_string(),
            stages: vec![Stage {
                name: "build".to_string(),
                commands: vec!["npm run build".to_string()],
                backend: Backend::Local,
                working_dir: None,
                fail_fast: true,
                health_check: None,
            }],
        };

        let json = serde_json::to_string(&pipeline).unwrap();
        let parsed: Pipeline = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.name, "Test Pipeline");
        assert_eq!(parsed.stages.len(), 1);
        assert_eq!(parsed.stages[0].name, "build");
    }
}
