// ---------------------------------------------------------------------------
// App Settings
// ---------------------------------------------------------------------------

/** App-level settings stored in the platform data directory. */
export interface AppSettings {
  default_notify_on_success: boolean;
  default_notify_on_failure: boolean;
  default_artifact_retention: number;
  default_run_retention: number;
}

/** Execution backend for a pipeline stage. */
export type Backend = 'local' | 'ssh';

/** A single stage in a pipeline. */
export interface Stage {
  name: string;
  commands: string[];
  backend: Backend;
  working_dir?: string;
  fail_fast: boolean;
  health_check?: HealthCheck;
}

/** Full pipeline definition. */
export interface Pipeline {
  name: string;
  stages: Stage[];
}

/** A project tracked by Chibby. */
export interface Project {
  id: string;
  name: string;
  path: string;
  added_at: string;
  last_run_at?: string;
  last_run_status?: RunStatus;
}

/** Project info returned from the backend. */
export interface ProjectInfo {
  project: Project;
  has_pipeline: boolean;
}

/** Git repository information. */
export interface GitInfo {
  branch?: string;
  commit?: string;
  is_dirty: boolean;
  ahead?: number;
  behind?: number;
}

/** Status of a single stage execution. */
export type StageStatus = 'pending' | 'running' | 'success' | 'failed' | 'skipped';

/** Result of executing one stage. */
export interface StageResult {
  stage_name: string;
  status: StageStatus;
  exit_code?: number;
  stdout: string;
  stderr: string;
  started_at?: string;
  finished_at?: string;
  duration_ms?: number;
  health_check_passed?: boolean;
}

/** Overall run status. */
export type RunStatus = 'pending' | 'running' | 'success' | 'failed' | 'cancelled';

/** The kind of run. */
export type RunKind = 'normal' | 'retry' | 'rollback';

/** A single pipeline run. */
export interface PipelineRun {
  id: string;
  pipeline_name: string;
  repo_path: string;
  environment?: string;
  branch?: string;
  commit?: string;
  status: RunStatus;
  stage_results: StageResult[];
  started_at: string;
  finished_at?: string;
  duration_ms?: number;
  /** What kind of run this is (normal, retry, or rollback). */
  run_kind?: RunKind;
  /** If this is a retry, the ID of the original run. */
  parent_run_id?: string;
  /** If this is a retry, which attempt number (1-based). */
  retry_number?: number;
  /** If this is a rollback, the ID of the run being rolled back to. */
  rollback_target_id?: string;
  /** The stage name where retry started from. */
  retry_from_stage?: string;
}

/** Summary of a deployment to a specific environment. */
export interface DeploymentRecord {
  run_id: string;
  pipeline_name: string;
  environment: string;
  status: RunStatus;
  branch?: string;
  commit?: string;
  started_at: string;
  duration_ms?: number;
  run_kind: RunKind;
}

/** Detected script info from repo scanning. */
export interface DetectedScript {
  file_name: string;
  file_path: string;
  script_type: string;
}

// ---------------------------------------------------------------------------
// Environments & Secrets (Phase 4)
// ---------------------------------------------------------------------------

/** A deployment target environment. */
export interface Environment {
  name: string;
  ssh_host?: string;
  ssh_port?: number;
  variables: Record<string, string>;
}

/** Top-level environments config from .chibby/environments.toml. */
export interface EnvironmentsConfig {
  environments: Environment[];
}

/** A named secret reference (values never stored in config). */
export interface SecretRef {
  name: string;
  environments: string[];
}

/** Top-level secrets config from .chibby/secrets.toml. */
export interface SecretsConfig {
  secrets: SecretRef[];
}

/** Status of a single secret in the OS keychain. */
export interface SecretStatus {
  name: string;
  is_set: boolean;
}

/** Health check configuration for post-deploy validation. */
export interface HealthCheck {
  command: string;
  retries: number;
  delay_secs: number;
}

/** A preflight validation error. */
export interface PreflightError {
  type:
    | 'MissingSecret'
    | 'MissingSshHost'
    | 'MissingEnvironment'
    | 'SshConnectivityFailed'
    | 'SshNotAvailable';
  detail: unknown;
}

/** Result of preflight validation. */
export interface PreflightResult {
  passed: boolean;
  errors: PreflightError[];
  warnings: string[];
}

// ---------------------------------------------------------------------------
// Pipeline Validation
// ---------------------------------------------------------------------------

/** Severity level for a pipeline warning. */
export type WarningSeverity = 'warning' | 'error';

/** A validation warning or error for a pipeline stage. */
export interface PipelineWarning {
  stage_name: string;
  command: string;
  message: string;
  suggestion?: string;
  severity: WarningSeverity;
}

/** A detected duplicate or conflicting configuration file. */
export interface FileConflict {
  category: string;
  files: string[];
  message: string;
  active_file?: string;
}

/** Result of validating a pipeline before execution. */
export interface PipelineValidation {
  warnings: PipelineWarning[];
  file_conflicts: FileConflict[];
  is_valid: boolean;
}

// ---------------------------------------------------------------------------
// CI Workflow Import
// ---------------------------------------------------------------------------

/** A single step from a CI workflow job. */
export interface WorkflowStep {
  name?: string;
  run: string;
}

/** A job from a CI workflow. */
export interface WorkflowJob {
  name: string;
  steps: WorkflowStep[];
}

/** A parsed CI workflow file (e.g., GitHub Actions). */
export interface WorkflowInfo {
  name: string;
  file_name: string;
  jobs: WorkflowJob[];
}

// ---------------------------------------------------------------------------
// Deployment Configuration
// ---------------------------------------------------------------------------

/** Deployment method for CD stages. */
export type DeploymentMethod =
  | 'auto_detect' // Parse from GitHub Actions deploy workflows
  | 'docker_compose_ssh' // Docker Compose over SSH
  | 'docker_registry' // Build/push to registry, then pull on server
  | 'cargo_publish' // Publish to crates.io
  | 'npm_publish' // Publish to npm registry
  | 'github_release' // Create GitHub release with binaries
  | 'ssh_rsync' // rsync/scp files to server
  | 'flyio' // Deploy to Fly.io
  | 'render' // Deploy to Render
  | 'railway' // Deploy to Railway
  | 'netlify' // Deploy to Netlify
  | 'vercel' // Deploy to Vercel
  | 's3_static' // Deploy to S3 bucket
  | 'skip'; // No deployment (CI only)

/** Configuration for deployment during project creation. */
export interface DeploymentConfig {
  method: DeploymentMethod;
  /** Target environment name (e.g., "production", "staging"). */
  environment_name?: string;
  /** SSH host for SSH-based deploys (user@hostname). */
  ssh_host?: string;
  /** Docker registry URL (e.g., "ghcr.io/username"). */
  docker_registry?: string;
  /** Health check URL path (e.g., "/health"). */
  health_check_url?: string;
  /** Docker Compose file to use (e.g., "docker-compose.prod.yml"). */
  compose_file?: string;
  /** Platform project name for PaaS (fly app name, etc.). */
  platform_project?: string;
  /** Whether to run dry-run first for package publishing. */
  dry_run_first?: boolean;
}

/** Detected project type from repository analysis. */
export type ProjectType =
  | 'Rust'
  | 'RustLibrary'
  | 'Tauri'
  | 'Node'
  | 'NodeLibrary'
  | 'Python'
  | 'Go'
  | 'StaticSite'
  | 'DockerCompose'
  | 'Unknown';

/** Display info for a deployment method option. */
export interface DeploymentMethodInfo {
  method: DeploymentMethod;
  label: string;
  description: string;
  icon: string;
  requiresConfig: boolean;
}

// ---------------------------------------------------------------------------
// Phase 5: Versioning, Signing, Artifacts, Notifications, Cleanup
// ---------------------------------------------------------------------------

/** Semver bump level. */
export type BumpLevel = 'patch' | 'minor' | 'major';

/** A file that contains a version string. */
export interface VersionFile {
  path: string;
  version: string;
}

/** Result of scanning a repo for version files. */
export interface VersionInfo {
  files: VersionFile[];
  current_version?: string;
  is_consistent: boolean;
  latest_tag?: string;
}

/** Result of a version bump operation. */
export interface BumpResult {
  old_version: string;
  new_version: string;
  updated_files: string[];
  git_tag?: string;
}

/** A single changelog entry. */
export interface ChangelogEntry {
  hash: string;
  subject: string;
  author: string;
  date: string;
}

/** Target platform for code signing. */
export type SigningPlatform = 'macos' | 'windows' | 'linux';

/** Configuration for code signing. */
export interface SigningConfig {
  enabled: boolean;
  macos_identity?: string;
  macos_team_id?: string;
  macos_bundle_id?: string;
  windows_cert_path?: string;
  linux_gpg_key?: string;
}

/** Result of a signing operation. */
export interface SigningResult {
  success: boolean;
  platform: SigningPlatform;
  artifact_path: string;
  notarized: boolean;
  message: string;
}

/** Artifact naming and storage configuration. */
export interface ArtifactConfig {
  output_dir: string;
  retention_count: number;
  patterns: string[];
  upload_to?: string;
}

/** A collected artifact with metadata. */
export interface Artifact {
  file_name: string;
  canonical_name: string;
  path: string;
  sha256: string;
  size_bytes: number;
  collected_at: string;
}

/** Manifest for a single artifact collection run. */
export interface ArtifactManifest {
  project: string;
  version: string;
  commit?: string;
  branch?: string;
  created_at: string;
  artifacts: Artifact[];
}

/** Notification channel type. */
export type NotifyChannel = 'desktop' | 'webhook';

/** When to send notifications. */
export type NotifyOn = 'success' | 'failure' | 'always';

/** A single notification target. */
export interface NotifyTarget {
  channel: NotifyChannel;
  url?: string;
  on: NotifyOn;
}

/** Notification configuration. */
export interface NotifyConfig {
  enabled: boolean;
  targets: NotifyTarget[];
}

/** Cleanup configuration. */
export interface CleanupConfig {
  artifact_retention: number;
  run_retention: number;
  prune_remote_docker: boolean;
}

/** Result of a cleanup operation. */
export interface CleanupResult {
  artifacts_removed: number;
  runs_removed: number;
  bytes_freed: number;
  details: string[];
}

// ---------------------------------------------------------------------------
// Phase 5.5: Tauri Updater Integration
// ---------------------------------------------------------------------------

/** Hosting target for update publishing. */
export type UpdatePublishTarget = 's3' | 'github_release' | 'scp' | 'local';

/** Tauri updater configuration from .chibby/updater.toml. */
export interface UpdaterConfig {
  enabled: boolean;
  public_key?: string;
  base_url?: string;
  publish_target?: UpdatePublishTarget;
  s3_bucket?: string;
  s3_region?: string;
  s3_endpoint?: string;
  github_repo?: string;
  scp_dest?: string;
  local_dir?: string;
}

/** A per-platform entry in the Tauri latest.json. */
export interface UpdatePlatformEntry {
  url: string;
  signature: string;
}

/** Tauri-compatible latest.json structure. */
export interface TauriLatestJson {
  version: string;
  notes?: string;
  pub_date: string;
  platforms: Record<string, UpdatePlatformEntry>;
}

/** Result of generating a Tauri update key pair. */
export interface UpdateKeyResult {
  public_key: string;
  private_key_stored: boolean;
  message: string;
}

/** Result of generating latest.json. */
export interface LatestJsonResult {
  path: string;
  content: TauriLatestJson;
  valid: boolean;
}

/** Result of signing an update bundle. */
export interface UpdateSignResult {
  file_path: string;
  signature: string;
  verified: boolean;
}

/** Result of publishing an update. */
export interface UpdatePublishResult {
  success: boolean;
  target: UpdatePublishTarget;
  uploaded_files: string[];
  message: string;
}

// ---------------------------------------------------------------------------
// Phase 5.8: Security and Quality Gates
// ---------------------------------------------------------------------------

/** Enforcement mode for a security/quality gate. */
export type GateMode = 'block' | 'warn' | 'off';

/** Security and quality gates config from .chibby/gates.toml. */
export interface GatesConfig {
  secret_scanning: GateMode;
  dependency_scanning: GateMode;
  commit_lint: GateMode;
  secret_scan_allowlist: string[];
  audit_allowlist: string[];
  audit_severity_threshold: string;
  secret_scan_baseline: boolean;
  commit_types: string[];
  commit_max_subject_length: number;
  commit_require_scope: boolean;
}

/** A single secret scanning finding. */
export interface SecretFinding {
  file: string;
  line: number;
  rule: string;
  preview: string;
}

/** Result of running secret scanning. */
export interface SecretScanResult {
  passed: boolean;
  findings: SecretFinding[];
  scanner: string;
  message: string;
}

/** Severity level for a dependency vulnerability. */
export type VulnSeverity = 'low' | 'medium' | 'high' | 'critical';

/** A single dependency audit finding. */
export interface AuditFinding {
  package: string;
  installed_version: string;
  fixed_version?: string;
  advisory_id: string;
  severity: VulnSeverity;
  description: string;
  upgrade_command?: string;
}

/** Result of running dependency/CVE scanning. */
export interface AuditResult {
  passed: boolean;
  findings: AuditFinding[];
  scanner: string;
  message: string;
}

/** A single commit lint violation. */
export interface CommitLintViolation {
  hash: string;
  subject: string;
  rule: string;
  expected: string;
}

/** Result of running commit message linting. */
export interface CommitLintResult {
  passed: boolean;
  violations: CommitLintViolation[];
  commits_checked: number;
  message: string;
}

/** Combined result of running all enabled gates. */
export interface GatesResult {
  passed: boolean;
  secret_scan?: SecretScanResult;
  dependency_audit?: AuditResult;
  commit_lint?: CommitLintResult;
}

// ---------------------------------------------------------------------------
// CI/CD Recommendations (for new/amateur developers)
// ---------------------------------------------------------------------------

/** Priority level for a file recommendation. */
export type RecommendationPriority = 'critical' | 'high' | 'medium' | 'low';

/** Category of a file recommendation. */
export type RecommendationCategory =
  | 'version_control'
  | 'documentation'
  | 'ci_cd'
  | 'code_quality'
  | 'testing'
  | 'security'
  | 'container'
  | 'dependencies';

/** A single file recommendation. */
export interface FileRecommendation {
  file_name: string;
  priority: RecommendationPriority;
  category: RecommendationCategory;
  reason: string;
  docs_url?: string;
}

/** Summary counts of recommendations by priority. */
export interface RecommendationSummary {
  critical_missing: number;
  high_missing: number;
  medium_missing: number;
  low_missing: number;
  total_recommendations: number;
  total_present: number;
}

/** Full recommendations for a project. */
export interface ProjectRecommendations {
  project_types: string[];
  recommendations: FileRecommendation[];
  readiness_score: number;
  summary: RecommendationSummary;
}

// ---------------------------------------------------------------------------
// Phase 8: Agent System
// ---------------------------------------------------------------------------

/** Skill modes for the CI/CD agent. */
export type SkillMode =
  | 'failure_analysis'
  | 'pipeline_optimization'
  | 'security_review'
  | 'deploy_troubleshoot'
  | 'project_setup'
  | 'pipeline_generate'
  | 'execute'
  | 'general';

/** Severity level for agent findings. */
export type Severity = 'critical' | 'warning' | 'info';

/** A single finding from agent analysis. */
export interface Finding {
  severity: Severity;
  title: string;
  detail: string;
  suggested_command?: string;
}

/** Result of agent analysis on a pipeline run. */
export interface AgentAnalysis {
  summary: string;
  findings: Finding[];
  suggested_actions: string[];
  skill_used: SkillMode;
}

/** Agent chat response. */
export interface AgentResponse {
  message: string;
  suggestions: string[];
  skill_used: SkillMode;
}

/** Pipeline format for generation. */
export type PipelineFormat = 'chibby' | 'github_actions' | 'circle_ci' | 'drone';

/** Generated pipeline config. */
export interface GeneratedPipeline {
  format: PipelineFormat;
  file_path: string;
  content: string;
  explanation: string;
}

/** Agent system status. */
export interface AgentSystemStatus {
  available: boolean;
  has_anthropic_key: boolean;
  has_openai_key: boolean;
  error?: string;
}

/** Agent memory entry. */
export interface MemoryEntry {
  key: string;
  value: string;
  memory_type: string;
  created_at: string;
  project_id?: string;
}

// ---------------------------------------------------------------------------
// Pipeline Templates
// ---------------------------------------------------------------------------

/** Where a template was loaded from (highest priority first). */
export type TemplateSource = 'project' | 'user' | 'built_in';

/** Whether the template is a full pipeline or a single stage snippet. */
export type TemplateType = 'pipeline' | 'stage';

/** Metadata block for a pipeline template. */
export interface TemplateMeta {
  name: string;
  description: string;
  author: string;
  version: string;
  category: string;
  tags: string[];
  project_types: string[];
  required_tools: string[];
  template_type: TemplateType;
}

/** A {{variable}} placeholder found in a template. */
export interface TemplateVariable {
  name: string;
  description: string;
  default_value: string;
  required: boolean;
}

/** A resolved pipeline template with its source. */
export interface PipelineTemplate {
  meta: TemplateMeta;
  source: TemplateSource;
  pipeline?: Pipeline;
  stages?: Stage[];
}
