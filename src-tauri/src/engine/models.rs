use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Pipeline definition (stored as .chibby/pipeline.toml)
// ---------------------------------------------------------------------------

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

fn default_true() -> bool {
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

/// Where a template was loaded from (highest priority first).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TemplateSource {
    /// Project-local `.chibby/templates/` (highest priority).
    Project,
    /// User-global `~/.chibby/templates/`.
    User,
    /// Bundled with the application.
    BuiltIn,
}

/// Whether the template is a full pipeline or a single stage snippet.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TemplateType {
    Pipeline,
    Stage,
}

/// Metadata block for a pipeline template file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateMeta {
    /// Human-readable template name (also used as the lookup key).
    pub name: String,
    /// Short description shown in the template browser.
    pub description: String,
    /// Original author (e.g. "chibby" for built-ins).
    #[serde(default)]
    pub author: String,
    /// Semantic version of the template itself.
    #[serde(default = "default_template_version")]
    pub version: String,
    /// Primary language/domain category (e.g. "python", "deployment").
    #[serde(default)]
    pub category: String,
    /// Free-form tags for filtering/search.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Project types this template is designed for (matches detector output).
    #[serde(default)]
    pub project_types: Vec<String>,
    /// CLI tools the template expects to be available.
    #[serde(default)]
    pub required_tools: Vec<String>,
    /// `pipeline` for full pipelines, `stage` for individual snippets.
    pub template_type: TemplateType,
}

fn default_template_version() -> String {
    "1.0.0".to_string()
}

/// A `{{variable}}` placeholder found in a template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateVariable {
    /// Variable name (without braces).
    pub name: String,
    /// Human-readable label for the input field.
    #[serde(default)]
    pub description: String,
    /// Pre-filled value when the template is applied.
    #[serde(default)]
    pub default_value: String,
    /// Whether the user must supply a value.
    #[serde(default = "default_true")]
    pub required: bool,
}

/// On-disk representation of a template TOML file.
///
/// For `template_type = "pipeline"` the `pipeline` field is populated.
/// For `template_type = "stage"` the `stages` field is populated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateFile {
    pub meta: TemplateMeta,
    /// Full pipeline (only present when `template_type == pipeline`).
    #[serde(default)]
    pub pipeline: Option<Pipeline>,
    /// Individual stage snippets (only present when `template_type == stage`).
    #[serde(default)]
    pub stages: Option<Vec<Stage>>,
}

/// A template with its resolved source attached (returned to the frontend).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineTemplate {
    pub meta: TemplateMeta,
    /// Where this template was loaded from.
    pub source: TemplateSource,
    /// Full pipeline (pipeline templates only).
    #[serde(default)]
    pub pipeline: Option<Pipeline>,
    /// Stage snippets (stage templates only).
    #[serde(default)]
    pub stages: Option<Vec<Stage>>,
}

// ---------------------------------------------------------------------------
// Environment definitions (stored as .chibby/environments.toml)
// ---------------------------------------------------------------------------

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

/// Status of a single stage execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StageStatus {
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
}

/// Result of executing one stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageResult {
    pub stage_name: String,
    pub status: StageStatus,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<u64>,
    /// Whether the post-stage health check passed (None if no health check configured).
    #[serde(default)]
    pub health_check_passed: Option<bool>,
}

/// Overall run status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    Pending,
    Running,
    Success,
    Failed,
    Cancelled,
}

/// The kind of run (normal, retry, or rollback).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RunKind {
    Normal,
    Retry,
    Rollback,
}

impl Default for RunKind {
    fn default() -> Self {
        Self::Normal
    }
}

/// A single pipeline run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineRun {
    pub id: String,
    pub pipeline_name: String,
    pub repo_path: String,
    pub environment: Option<String>,
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub status: RunStatus,
    pub stage_results: Vec<StageResult>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<u64>,
    /// The exact pipeline definition executed for this run.
    #[serde(default)]
    pub pipeline_snapshot: Option<Pipeline>,
    /// The source pipeline file name used for this run (`pipeline` by default).
    #[serde(default)]
    pub pipeline_file: Option<String>,
    /// What kind of run this is (normal, retry, or rollback).
    #[serde(default)]
    pub run_kind: RunKind,
    /// If this is a retry, the ID of the original run.
    #[serde(default)]
    pub parent_run_id: Option<String>,
    /// If this is a retry, which attempt number (1-based).
    #[serde(default)]
    pub retry_number: Option<u32>,
    /// If this is a rollback, the ID of the run being rolled back to.
    #[serde(default)]
    pub rollback_target_id: Option<String>,
    /// The stage name where retry started from (stages before this were skipped).
    #[serde(default)]
    pub retry_from_stage: Option<String>,
}

impl PipelineRun {
    /// Create a new pending run.
    pub fn new(pipeline_name: &str, repo_path: &str, environment: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            pipeline_name: pipeline_name.to_string(),
            repo_path: repo_path.to_string(),
            environment,
            branch: None,
            commit: None,
            status: RunStatus::Pending,
            stage_results: Vec::new(),
            started_at: Utc::now(),
            finished_at: None,
            duration_ms: None,
            pipeline_snapshot: None,
            pipeline_file: None,
            run_kind: RunKind::Normal,
            parent_run_id: None,
            retry_number: None,
            rollback_target_id: None,
            retry_from_stage: None,
        }
    }
}

/// Summary of a deployment to a specific environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentRecord {
    /// The run that produced this deployment.
    pub run_id: String,
    /// Pipeline name.
    pub pipeline_name: String,
    /// Environment deployed to.
    pub environment: String,
    /// Run status.
    pub status: RunStatus,
    /// Git branch at deploy time.
    pub branch: Option<String>,
    /// Git commit at deploy time.
    pub commit: Option<String>,
    /// When the deploy started.
    pub started_at: DateTime<Utc>,
    /// Run duration.
    pub duration_ms: Option<u64>,
    /// Whether this was a retry or rollback.
    pub run_kind: RunKind,
}

// ---------------------------------------------------------------------------
// Project (a tracked repo in Chibby)
// ---------------------------------------------------------------------------

/// A project tracked by Chibby.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub added_at: DateTime<Utc>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_run_status: Option<RunStatus>,
}

impl Project {
    /// Create a new project from a repo path.
    pub fn new(name: &str, path: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            path: path.to_string(),
            added_at: Utc::now(),
            last_run_at: None,
            last_run_status: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Pipeline Validation (pre-run checks)
// ---------------------------------------------------------------------------

/// Severity level for a pipeline warning.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum WarningSeverity {
    /// May cause issues but could work
    Warning,
    /// Will likely fail
    Error,
}

/// A validation warning or error for a pipeline stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineWarning {
    /// The stage name this warning applies to.
    pub stage_name: String,
    /// The specific command that may fail.
    pub command: String,
    /// Human-readable description of the issue.
    pub message: String,
    /// Suggested fix for the issue.
    pub suggestion: Option<String>,
    /// Severity level.
    pub severity: WarningSeverity,
}

/// Result of validating a pipeline before execution.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PipelineValidation {
    /// List of warnings/errors found.
    pub warnings: Vec<PipelineWarning>,
    /// Detected duplicate or conflicting config files.
    pub file_conflicts: Vec<FileConflict>,
    /// Whether the pipeline is likely to succeed (no errors, only warnings).
    pub is_valid: bool,
}

/// A detected duplicate or conflicting configuration file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileConflict {
    /// Category of the conflict (e.g., "Makefile", "Docker Compose").
    pub category: String,
    /// List of conflicting file names.
    pub files: Vec<String>,
    /// Human-readable description of the issue.
    pub message: String,
    /// Which file will be used (if deterministic).
    pub active_file: Option<String>,
}

// ---------------------------------------------------------------------------
// Phase 5: Versioning, Signing, Artifacts, Notifications, Cleanup
// ---------------------------------------------------------------------------

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

/// Target platform for code signing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SigningPlatform {
    Macos,
    Windows,
    Linux,
}

/// Configuration for code signing (stored in .chibby/signing.toml).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningConfig {
    /// Whether signing is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// macOS Developer ID identity (e.g. "Developer ID Application: Name (TEAMID)").
    #[serde(default)]
    pub macos_identity: Option<String>,
    /// macOS team ID for notarization.
    #[serde(default)]
    pub macos_team_id: Option<String>,
    /// macOS bundle ID for notarization.
    #[serde(default)]
    pub macos_bundle_id: Option<String>,
    /// Windows certificate file path (relative to repo).
    #[serde(default)]
    pub windows_cert_path: Option<String>,
    /// Linux GPG key ID for package signing.
    #[serde(default)]
    pub linux_gpg_key: Option<String>,
}

impl Default for SigningConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            macos_identity: None,
            macos_team_id: None,
            macos_bundle_id: None,
            windows_cert_path: None,
            linux_gpg_key: None,
        }
    }
}

/// Result of a signing operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningResult {
    /// Whether signing succeeded.
    pub success: bool,
    /// Platform that was signed for.
    pub platform: SigningPlatform,
    /// Path to the signed artifact.
    pub artifact_path: String,
    /// Whether notarization was performed (macOS only).
    pub notarized: bool,
    /// Human-readable status message.
    pub message: String,
}

/// Artifact naming configuration (stored in .chibby/artifacts.toml).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactConfig {
    /// Output directory for artifacts (relative to repo root).
    #[serde(default = "default_artifact_dir")]
    pub output_dir: String,
    /// How many versions to retain locally.
    #[serde(default = "default_retention")]
    pub retention_count: u32,
    /// Glob patterns to collect as artifacts (e.g. "target/release/*.dmg").
    #[serde(default)]
    pub patterns: Vec<String>,
    /// Optional upload destination (e.g. "s3://bucket/path", "github-release", "scp://host:/path").
    #[serde(default)]
    pub upload_to: Option<String>,
}

fn default_artifact_dir() -> String {
    ".chibby/artifacts".to_string()
}

fn default_retention() -> u32 {
    5
}

impl Default for ArtifactConfig {
    fn default() -> Self {
        Self {
            output_dir: default_artifact_dir(),
            patterns: Vec::new(),
            retention_count: default_retention(),
            upload_to: None,
        }
    }
}

/// A collected artifact with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    /// Original file name.
    pub file_name: String,
    /// Standardized name ({project}-{version}-{platform}-{arch}.{ext}).
    pub canonical_name: String,
    /// Absolute path to the artifact.
    pub path: String,
    /// SHA256 checksum.
    pub sha256: String,
    /// File size in bytes.
    pub size_bytes: u64,
    /// When the artifact was collected.
    pub collected_at: DateTime<Utc>,
}

/// Manifest for a single artifact collection run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactManifest {
    /// Project name.
    pub project: String,
    /// Version that was built.
    pub version: String,
    /// Git commit hash.
    pub commit: Option<String>,
    /// Git branch.
    pub branch: Option<String>,
    /// When the manifest was created.
    pub created_at: DateTime<Utc>,
    /// List of artifacts in this collection.
    pub artifacts: Vec<Artifact>,
}

/// Notification channel type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NotifyChannel {
    Desktop,
    Webhook,
}

/// When to send notifications.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NotifyOn {
    Success,
    Failure,
    Always,
}

impl Default for NotifyOn {
    fn default() -> Self {
        Self::Always
    }
}

/// A single notification target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotifyTarget {
    /// Channel type.
    pub channel: NotifyChannel,
    /// Webhook URL (required for Webhook channel).
    #[serde(default)]
    pub url: Option<String>,
    /// When to fire this notification.
    #[serde(default)]
    pub on: NotifyOn,
}

/// Notification configuration (stored in .chibby/notify.toml).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotifyConfig {
    /// Whether notifications are enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Notification targets.
    #[serde(default)]
    pub targets: Vec<NotifyTarget>,
}

impl Default for NotifyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            targets: Vec::new(),
        }
    }
}

/// Payload sent with a notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotifyPayload {
    pub project: String,
    pub version: Option<String>,
    pub environment: Option<String>,
    pub status: RunStatus,
    pub duration_ms: Option<u64>,
    pub message: String,
}

/// Cleanup configuration (stored in .chibby/cleanup.toml).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupConfig {
    /// Max artifact versions to keep per project.
    #[serde(default = "default_retention")]
    pub artifact_retention: u32,
    /// Max run history entries to keep.
    #[serde(default = "default_run_retention")]
    pub run_retention: u32,
    /// Whether to prune Docker images on remote deploy targets.
    #[serde(default)]
    pub prune_remote_docker: bool,
}

fn default_run_retention() -> u32 {
    50
}

impl Default for CleanupConfig {
    fn default() -> Self {
        Self {
            artifact_retention: default_retention(),
            run_retention: default_run_retention(),
            prune_remote_docker: false,
        }
    }
}

/// Result of a cleanup operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupResult {
    /// Number of artifact versions removed.
    pub artifacts_removed: u32,
    /// Number of run history entries removed.
    pub runs_removed: u32,
    /// Bytes freed.
    pub bytes_freed: u64,
    /// Details of what was cleaned.
    pub details: Vec<String>,
}

// ---------------------------------------------------------------------------
// CI/CD Recommendations
// ---------------------------------------------------------------------------

/// Priority level for a CI/CD recommendation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum RecommendationPriority {
    /// Critical - Essential for basic CI/CD setup
    Critical,
    /// High - Important for code quality and automation
    High,
    /// Medium - Good practices for maintainability
    Medium,
    /// Low - Nice to have for mature projects
    Low,
}

/// Category of CI/CD recommendation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationCategory {
    /// Version control hygiene
    VersionControl,
    /// Documentation
    Documentation,
    /// CI/CD workflows
    CiCd,
    /// Code quality and linting
    CodeQuality,
    /// Testing
    Testing,
    /// Security
    Security,
    /// Containerization
    Container,
    /// Dependencies
    Dependencies,
}

/// A single CI/CD file recommendation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecommendation {
    /// File name or path to create.
    pub file_name: String,
    /// Human-readable title.
    pub title: String,
    /// Description of why this file is important.
    pub description: String,
    /// Priority level.
    pub priority: RecommendationPriority,
    /// Category of recommendation.
    pub category: RecommendationCategory,
    /// Link to documentation or template.
    pub docs_url: Option<String>,
    /// Whether this file already exists in the project.
    pub exists: bool,
    /// Suggested template content (if available).
    pub template_hint: Option<String>,
}

/// Full recommendations result for a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRecommendations {
    /// Detected project type(s).
    pub project_types: Vec<String>,
    /// List of all recommendations.
    pub recommendations: Vec<FileRecommendation>,
    /// Overall CI/CD readiness score (0-100).
    pub readiness_score: u8,
    /// Summary counts by priority.
    pub summary: RecommendationSummary,
}

/// Summary of recommendations by priority.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationSummary {
    pub critical_missing: u32,
    pub high_missing: u32,
    pub medium_missing: u32,
    pub low_missing: u32,
    pub total_recommendations: u32,
    pub total_present: u32,
}

// ---------------------------------------------------------------------------
// Phase 5.5: Tauri Updater Integration
// ---------------------------------------------------------------------------

/// Hosting target for update publishing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum UpdatePublishTarget {
    /// AWS S3 or S3-compatible (e.g. Cloudflare R2).
    S3,
    /// GitHub Releases.
    GithubRelease,
    /// SCP to a static file server.
    Scp,
    /// Local directory (for self-hosted or LAN distribution).
    Local,
}

/// Tauri updater configuration (stored in .chibby/updater.toml).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdaterConfig {
    /// Whether the updater integration is enabled.
    #[serde(default)]
    pub enabled: bool,

    /// Public key for Tauri update verification (stored in config, safe to commit).
    #[serde(default)]
    pub public_key: Option<String>,

    /// Base URL where update artifacts will be hosted.
    /// Used to construct download URLs in latest.json.
    #[serde(default)]
    pub base_url: Option<String>,

    /// Publish target type.
    #[serde(default)]
    pub publish_target: Option<UpdatePublishTarget>,

    /// S3 bucket name (for S3/R2 targets).
    #[serde(default)]
    pub s3_bucket: Option<String>,

    /// S3 region (for S3 targets).
    #[serde(default)]
    pub s3_region: Option<String>,

    /// S3 endpoint URL (for S3-compatible like R2).
    #[serde(default)]
    pub s3_endpoint: Option<String>,

    /// GitHub owner/repo (for GitHub Releases target).
    #[serde(default)]
    pub github_repo: Option<String>,

    /// SCP destination (user@host:/path) for SCP target.
    #[serde(default)]
    pub scp_dest: Option<String>,

    /// Local directory path (for local target).
    #[serde(default)]
    pub local_dir: Option<String>,
}

impl Default for UpdaterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            public_key: None,
            base_url: None,
            publish_target: None,
            s3_bucket: None,
            s3_region: None,
            s3_endpoint: None,
            github_repo: None,
            scp_dest: None,
            local_dir: None,
        }
    }
}

/// A per-platform entry in the Tauri latest.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePlatformEntry {
    /// Download URL for this platform's update bundle.
    pub url: String,
    /// Base64-encoded Ed25519 signature of the update bundle.
    pub signature: String,
}

/// Tauri-compatible latest.json structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TauriLatestJson {
    /// Version string (semver).
    pub version: String,
    /// Release notes (from changelog).
    #[serde(default)]
    pub notes: Option<String>,
    /// Publication date (RFC 3339).
    pub pub_date: String,
    /// Per-platform update entries keyed by Tauri platform identifiers
    /// (e.g. "darwin-aarch64", "darwin-x86_64", "linux-x86_64", "windows-x86_64").
    pub platforms: std::collections::HashMap<String, UpdatePlatformEntry>,
}

/// Result of generating a Tauri update key pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateKeyResult {
    /// The public key string.
    pub public_key: String,
    /// Whether the private key was stored in the OS keychain.
    pub private_key_stored: bool,
    /// Human-readable status message.
    pub message: String,
}

/// Result of generating latest.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestJsonResult {
    /// Path to the generated latest.json file.
    pub path: String,
    /// The generated JSON content (for preview).
    pub content: TauriLatestJson,
    /// Whether schema validation passed.
    pub valid: bool,
}

/// Result of signing an update bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSignResult {
    /// Path to the file that was signed.
    pub file_path: String,
    /// Base64-encoded signature.
    pub signature: String,
    /// Whether local verification passed.
    pub verified: bool,
}

/// Result of publishing an update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePublishResult {
    /// Whether the publish succeeded.
    pub success: bool,
    /// Target that was published to.
    pub target: UpdatePublishTarget,
    /// Files that were uploaded.
    pub uploaded_files: Vec<String>,
    /// Human-readable status message.
    pub message: String,
}

// ---------------------------------------------------------------------------
// Phase 5.8: Security and Quality Gates
// ---------------------------------------------------------------------------

/// Enforcement mode for a security/quality gate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum GateMode {
    /// Block the pipeline on findings.
    Block,
    /// Report findings but continue.
    Warn,
    /// Disable the gate entirely.
    Off,
}

impl Default for GateMode {
    fn default() -> Self {
        Self::Off
    }
}

/// Top-level security and quality gates config (stored in .chibby/gates.toml).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatesConfig {
    /// Secret scanning mode.
    #[serde(default)]
    pub secret_scanning: GateMode,

    /// Dependency/CVE scanning mode.
    #[serde(default)]
    pub dependency_scanning: GateMode,

    /// Commit message linting mode.
    #[serde(default)]
    pub commit_lint: GateMode,

    /// Paths to exclude from secret scanning (glob patterns).
    #[serde(default)]
    pub secret_scan_allowlist: Vec<String>,

    /// CVE IDs or package names to ignore in dependency scanning.
    #[serde(default)]
    pub audit_allowlist: Vec<String>,

    /// Severity threshold for dependency scanning: block on this level and above.
    /// One of "critical", "high", "medium", "low". Default: "high".
    #[serde(default = "default_severity_threshold")]
    pub audit_severity_threshold: String,

    /// Whether to use baseline mode for secret scanning (ignore existing findings).
    #[serde(default)]
    pub secret_scan_baseline: bool,

    /// Commit lint: allowed commit types.
    #[serde(default = "default_commit_types")]
    pub commit_types: Vec<String>,

    /// Commit lint: max subject line length.
    #[serde(default = "default_max_subject_len")]
    pub commit_max_subject_length: usize,

    /// Commit lint: require a scope.
    #[serde(default)]
    pub commit_require_scope: bool,
}

fn default_severity_threshold() -> String {
    "high".to_string()
}

fn default_commit_types() -> Vec<String> {
    vec![
        "feat".into(),
        "fix".into(),
        "docs".into(),
        "style".into(),
        "refactor".into(),
        "perf".into(),
        "test".into(),
        "build".into(),
        "ci".into(),
        "chore".into(),
    ]
}

fn default_max_subject_len() -> usize {
    72
}

impl Default for GatesConfig {
    fn default() -> Self {
        Self {
            secret_scanning: GateMode::Off,
            dependency_scanning: GateMode::Off,
            commit_lint: GateMode::Off,
            secret_scan_allowlist: Vec::new(),
            audit_allowlist: Vec::new(),
            audit_severity_threshold: default_severity_threshold(),
            secret_scan_baseline: false,
            commit_types: default_commit_types(),
            commit_max_subject_length: default_max_subject_len(),
            commit_require_scope: false,
        }
    }
}

/// A single secret scanning finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretFinding {
    /// File path relative to repo root.
    pub file: String,
    /// Line number where the secret was found.
    pub line: u32,
    /// Rule that matched (e.g. "aws-access-key", "generic-api-key").
    pub rule: String,
    /// Redacted preview of the match.
    pub preview: String,
}

/// Result of running secret scanning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretScanResult {
    /// Whether the scan passed (no blocking findings).
    pub passed: bool,
    /// Findings from the scan.
    pub findings: Vec<SecretFinding>,
    /// Whether gitleaks was used (vs built-in scanner).
    pub scanner: String,
    /// Human-readable summary.
    pub message: String,
}

/// Severity level for a dependency vulnerability.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
#[serde(rename_all = "lowercase")]
pub enum VulnSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// A single dependency audit finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditFinding {
    /// Package name.
    pub package: String,
    /// Installed version.
    pub installed_version: String,
    /// Fixed version (if available).
    pub fixed_version: Option<String>,
    /// CVE or advisory identifier.
    pub advisory_id: String,
    /// Severity level.
    pub severity: VulnSeverity,
    /// Short description.
    pub description: String,
    /// Suggested upgrade command (if applicable).
    pub upgrade_command: Option<String>,
}

/// Result of running dependency/CVE scanning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditResult {
    /// Whether the scan passed (no findings at or above threshold).
    pub passed: bool,
    /// Findings from the scan.
    pub findings: Vec<AuditFinding>,
    /// Which scanner was used (e.g. "cargo audit", "npm audit").
    pub scanner: String,
    /// Human-readable summary.
    pub message: String,
}

/// A single commit lint violation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitLintViolation {
    /// Git commit hash (short).
    pub hash: String,
    /// The original commit message subject.
    pub subject: String,
    /// What rule was violated.
    pub rule: String,
    /// Explanation of the expected format.
    pub expected: String,
}

/// Result of running commit message linting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitLintResult {
    /// Whether all commits passed.
    pub passed: bool,
    /// Violations found.
    pub violations: Vec<CommitLintViolation>,
    /// Total commits checked.
    pub commits_checked: u32,
    /// Human-readable summary.
    pub message: String,
}

/// Combined result of running all enabled gates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatesResult {
    /// Whether all gates passed.
    pub passed: bool,
    /// Secret scanning result (None if gate is off).
    pub secret_scan: Option<SecretScanResult>,
    /// Dependency audit result (None if gate is off).
    pub dependency_audit: Option<AuditResult>,
    /// Commit lint result (None if gate is off).
    pub commit_lint: Option<CommitLintResult>,
}

// ---------------------------------------------------------------------------
// Unit Tests
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

    #[test]
    fn test_stage_status_serialization() {
        let statuses = vec![
            (StageStatus::Pending, "pending"),
            (StageStatus::Running, "running"),
            (StageStatus::Success, "success"),
            (StageStatus::Failed, "failed"),
            (StageStatus::Skipped, "skipped"),
        ];

        for (status, expected) in statuses {
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(json, format!("\"{}\"", expected));
        }
    }

    #[test]
    fn test_run_status_serialization() {
        let statuses = vec![
            (RunStatus::Pending, "pending"),
            (RunStatus::Running, "running"),
            (RunStatus::Success, "success"),
            (RunStatus::Failed, "failed"),
            (RunStatus::Cancelled, "cancelled"),
        ];

        for (status, expected) in statuses {
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(json, format!("\"{}\"", expected));
        }
    }

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

    #[test]
    fn test_cleanup_config_defaults() {
        let config = CleanupConfig::default();

        assert_eq!(config.artifact_retention, 5);
        assert_eq!(config.run_retention, 50);
        assert!(!config.prune_remote_docker);
    }

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

    #[test]
    fn test_warning_severity() {
        let warning: WarningSeverity = serde_json::from_str(r#""warning""#).unwrap();
        let error: WarningSeverity = serde_json::from_str(r#""error""#).unwrap();

        assert_eq!(warning, WarningSeverity::Warning);
        assert_eq!(error, WarningSeverity::Error);
    }
}
