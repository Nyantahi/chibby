//! Pipeline definition types (stored as .chibby/pipeline.toml).

#[allow(unused_imports)]
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
///
/// NOTE: field order is load-bearing. A stage serializes as a TOML
/// `[[stages]]` array-of-tables, and TOML requires every scalar value to be
/// emitted before any nested table. Keep all scalar fields above the
/// table-valued ones or `toml::to_string` fails at runtime.
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
    /// Wall-clock budget for one attempt at this stage (covers all its commands).
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    /// Commands that undo this stage, used by `RollbackMode::Commands`.
    /// A TOML array is a *value*, not a table, so it belongs above the divider.
    #[serde(default)]
    pub rollback_commands: Option<Vec<String>>,
    // ---- every field above is scalar; every field below is table-valued ----
    /// Optional health check to run after this stage completes.
    #[serde(default)]
    pub health_check: Option<HealthCheck>,
    /// Optional retry policy for the stage's commands.
    #[serde(default)]
    pub retry: Option<StageRetry>,
    /// Optional conditions deciding whether the stage runs at all.
    #[serde(default)]
    pub when: Option<StageWhen>,
    /// Stage-scoped environment variables, overlaid on the run's variables.
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
    /// What to do when this stage's health check fails. Wins over the
    /// pipeline-wide setting.
    #[serde(default)]
    pub on_health_failure: Option<RollbackPolicy>,
}

impl Default for Stage {
    fn default() -> Self {
        Self {
            name: String::new(),
            commands: Vec::new(),
            backend: Backend::Local,
            working_dir: None,
            fail_fast: true,
            timeout_secs: None,
            rollback_commands: None,
            health_check: None,
            retry: None,
            when: None,
            env: None,
            on_health_failure: None,
        }
    }
}

/// Retry policy for a stage's commands (not its health check — `HealthCheck`
/// carries its own retry budget).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StageRetry {
    /// Total attempts including the first. 1 = no retry.
    #[serde(default = "default_attempts")]
    pub attempts: u32,
    /// Delay before the next attempt.
    #[serde(default = "default_retry_delay")]
    pub delay_secs: u64,
    /// How the delay grows between attempts.
    #[serde(default)]
    pub backoff: Backoff,
}

impl Default for StageRetry {
    fn default() -> Self {
        Self {
            attempts: default_attempts(),
            delay_secs: default_retry_delay(),
            backoff: Backoff::default(),
        }
    }
}

/// How the retry delay grows between attempts.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Backoff {
    /// Same delay before every attempt.
    #[default]
    Fixed,
    /// Delay doubles each attempt (`delay * 2^(attempt-1)`).
    Exponential,
}

/// Declarative stage conditions. Populated fields are ANDed; entries within a
/// field are ORed. Empty/None = always run.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct StageWhen {
    /// Glob patterns matched against the git branch, e.g. ["main", "release/*"].
    #[serde(default)]
    pub branch: Vec<String>,
    /// Glob patterns that exclude the stage when the branch matches.
    #[serde(default)]
    pub branch_not: Vec<String>,
    /// Glob patterns matched against the environment name.
    #[serde(default)]
    pub environment: Vec<String>,
    /// Glob patterns that exclude the stage when the environment matches.
    #[serde(default)]
    pub environment_not: Vec<String>,
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

fn default_attempts() -> u32 {
    2
}

fn default_retry_delay() -> u64 {
    5
}

pub(crate) fn default_true() -> bool {
    true
}

fn default_one() -> u32 {
    1
}

fn default_window() -> u32 {
    60
}

/// How a failed post-deploy health check should be undone.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RollbackMode {
    /// No automatic rollback. Absent config resolves here, so behaviour is
    /// unchanged for every pipeline that does not opt in.
    #[default]
    Off,
    /// Replay the last known-good deployment's recorded pipeline snapshot.
    LastGood,
    /// Run the failed stage's own `rollback_commands`.
    Commands,
}

/// Automatic-rollback policy for a health-check failure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RollbackPolicy {
    #[serde(default)]
    pub mode: RollbackMode,
    /// Re-run the stage's health check after rolling back.
    #[serde(default = "default_true")]
    pub verify_health: bool,
    /// Send a notification describing the rollback.
    #[serde(default = "default_true")]
    pub notify: bool,
    /// Cap auto-rollbacks against one target inside `window_mins`.
    #[serde(default = "default_one")]
    pub max_attempts: u32,
    /// Throttle window for `max_attempts`, in minutes.
    #[serde(default = "default_window")]
    pub window_mins: u32,
}

impl Default for RollbackPolicy {
    fn default() -> Self {
        Self {
            mode: RollbackMode::default(),
            verify_health: default_true(),
            notify: default_true(),
            max_attempts: default_one(),
            window_mins: default_window(),
        }
    }
}

/// Full pipeline definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    /// Display name for the pipeline.
    pub name: String,
    /// Pipeline-wide rollback default; a stage's own setting wins.
    #[serde(default)]
    pub on_health_failure: Option<RollbackPolicy>,
    // `stages` is an array of tables and MUST stay the last field.
    /// Ordered list of stages.
    pub stages: Vec<Stage>,
}

impl Pipeline {
    /// Rollback policy in force for `stage`: stage setting, else pipeline
    /// default, else `Off`.
    pub fn rollback_policy_for(&self, stage: &Stage) -> RollbackPolicy {
        stage
            .on_health_failure
            .clone()
            .or_else(|| self.on_health_failure.clone())
            .unwrap_or_default()
    }
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
        assert!(stage.timeout_secs.is_none());
        assert!(stage.retry.is_none());
        assert!(stage.when.is_none());
        assert!(stage.env.is_none());
    }

    /// Every pipeline.toml written before stage hardening landed has none of
    /// the new keys — they must still parse, with the new fields defaulted.
    #[test]
    fn test_legacy_pipeline_toml_still_parses() {
        let toml_src = r#"
name = "Legacy"

[[stages]]
name = "build"
commands = ["npm run build"]

[[stages]]
name = "deploy"
commands = ["./deploy.sh"]
backend = "ssh"
working_dir = "/srv/app"
fail_fast = false

[stages.health_check]
command = "curl -f http://localhost/health"
"#;

        let pipeline: Pipeline = toml::from_str(toml_src).unwrap();

        assert_eq!(pipeline.stages.len(), 2);
        assert!(pipeline.stages[0].fail_fast);
        assert!(pipeline.stages[0].timeout_secs.is_none());
        assert!(pipeline.stages[0].retry.is_none());
        assert!(pipeline.stages[0].when.is_none());
        assert_eq!(pipeline.stages[1].backend, Backend::Ssh);
        assert!(pipeline.stages[1].health_check.is_some());
    }

    /// Guards TOML's "values must be emitted before tables" rule: adding a
    /// scalar `Stage` field below a table-valued one breaks `toml::to_string`
    /// at runtime, and only a round-trip catches it.
    #[test]
    fn test_stage_toml_roundtrip_with_all_fields() {
        let pipeline = Pipeline {
            name: "Full".to_string(),
            on_health_failure: Some(RollbackPolicy {
                mode: RollbackMode::LastGood,
                verify_health: false,
                notify: false,
                max_attempts: 3,
                window_mins: 15,
            }),
            stages: vec![Stage {
                name: "deploy".to_string(),
                commands: vec!["./deploy.sh".to_string()],
                backend: Backend::Ssh,
                working_dir: Some("/srv/app".to_string()),
                fail_fast: false,
                timeout_secs: Some(600),
                rollback_commands: Some(vec!["kubectl rollout undo deploy/api".to_string()]),
                health_check: Some(HealthCheck {
                    command: "curl -f http://localhost/health".to_string(),
                    retries: 5,
                    delay_secs: 2,
                }),
                retry: Some(StageRetry {
                    attempts: 3,
                    delay_secs: 10,
                    backoff: Backoff::Exponential,
                }),
                when: Some(StageWhen {
                    branch: vec!["main".to_string(), "release/*".to_string()],
                    branch_not: vec!["wip/*".to_string()],
                    environment: vec!["prod".to_string()],
                    environment_not: vec!["local".to_string()],
                }),
                env: Some(HashMap::from([(
                    "DEPLOY_TARGET".to_string(),
                    "blue".to_string(),
                )])),
                on_health_failure: Some(RollbackPolicy {
                    mode: RollbackMode::Commands,
                    ..Default::default()
                }),
            }],
        };

        let text = toml::to_string(&pipeline).expect("stage must serialize to TOML");
        let parsed: Pipeline = toml::from_str(&text).unwrap();
        let stage = &parsed.stages[0];

        assert_eq!(stage.timeout_secs, Some(600));
        assert_eq!(stage.backend, Backend::Ssh);
        assert!(!stage.fail_fast);
        assert_eq!(stage.retry, pipeline.stages[0].retry);
        assert_eq!(stage.when, pipeline.stages[0].when);
        assert_eq!(stage.env, pipeline.stages[0].env);
        assert_eq!(stage.health_check.as_ref().unwrap().retries, 5);
        assert_eq!(
            stage.rollback_commands.as_deref(),
            Some(["kubectl rollout undo deploy/api".to_string()].as_slice())
        );
        assert_eq!(
            stage.on_health_failure,
            pipeline.stages[0].on_health_failure
        );
        assert_eq!(parsed.on_health_failure, pipeline.on_health_failure);
    }

    /// A pipeline that never opts in must resolve to `Off` — the whole feature
    /// is inert unless configured.
    #[test]
    fn test_absent_rollback_config_resolves_to_off() {
        let pipeline: Pipeline = toml::from_str(
            r#"
name = "Legacy"

[[stages]]
name = "deploy"
commands = ["./deploy.sh"]
"#,
        )
        .unwrap();

        assert!(pipeline.on_health_failure.is_none());
        assert!(pipeline.stages[0].rollback_commands.is_none());
        assert_eq!(
            pipeline.rollback_policy_for(&pipeline.stages[0]).mode,
            RollbackMode::Off
        );
    }

    /// A stage policy must win over the pipeline-wide default.
    #[test]
    fn test_stage_rollback_policy_overrides_pipeline_default() {
        let pipeline = Pipeline {
            name: "Mixed".to_string(),
            on_health_failure: Some(RollbackPolicy {
                mode: RollbackMode::LastGood,
                ..Default::default()
            }),
            stages: vec![
                Stage {
                    name: "inherits".to_string(),
                    ..Default::default()
                },
                Stage {
                    name: "overrides".to_string(),
                    on_health_failure: Some(RollbackPolicy {
                        mode: RollbackMode::Off,
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ],
        };

        assert_eq!(
            pipeline.rollback_policy_for(&pipeline.stages[0]).mode,
            RollbackMode::LastGood
        );
        assert_eq!(
            pipeline.rollback_policy_for(&pipeline.stages[1]).mode,
            RollbackMode::Off
        );
    }

    #[test]
    fn test_rollback_policy_defaults() {
        let policy: RollbackPolicy = toml::from_str("mode = \"last_good\"").unwrap();

        assert_eq!(policy.mode, RollbackMode::LastGood);
        assert!(policy.verify_health);
        assert!(policy.notify);
        assert_eq!(policy.max_attempts, 1);
        assert_eq!(policy.window_mins, 60);
    }

    #[test]
    fn test_stage_retry_defaults() {
        let retry: StageRetry = serde_json::from_str("{}").unwrap();

        assert_eq!(retry.attempts, 2);
        assert_eq!(retry.delay_secs, 5);
        assert_eq!(retry.backoff, Backoff::Fixed);
        assert_eq!(retry, StageRetry::default());
    }

    #[test]
    fn test_backoff_serialization() {
        assert_eq!(
            serde_json::to_string(&Backoff::Exponential).unwrap(),
            r#""exponential""#
        );
        assert_eq!(Backoff::default(), Backoff::Fixed);
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
            on_health_failure: None,
            stages: vec![Stage {
                name: "build".to_string(),
                commands: vec!["npm run build".to_string()],
                ..Default::default()
            }],
        };

        let json = serde_json::to_string(&pipeline).unwrap();
        let parsed: Pipeline = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.name, "Test Pipeline");
        assert_eq!(parsed.stages.len(), 1);
        assert_eq!(parsed.stages[0].name, "build");
    }
}
