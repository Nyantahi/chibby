import { invoke } from '@tauri-apps/api/core';
import type {
  AppSettings,
  ProjectInfo,
  Project,
  Pipeline,
  PipelineRun,
  DeploymentRecord,
  DetectedScript,
  EnvironmentsConfig,
  SecretsConfig,
  SecretStatus,
  PreflightResult,
  PipelineValidation,
  GitInfo,
  WorkflowInfo,
  Stage,
  VersionInfo,
  BumpLevel,
  BumpResult,
  ChangelogEntry,
  ArtifactConfig,
  ArtifactManifest,
  SigningConfig,
  SigningResult,
  NotifyConfig,
  CleanupConfig,
  CleanupResult,
  ProjectRecommendations,
  UpdaterConfig,
  UpdateKeyResult,
  LatestJsonResult,
  TauriLatestJson,
  UpdateSignResult,
  UpdatePublishResult,
  GatesConfig,
  GatesResult,
  SecretScanResult,
  AuditResult,
  CommitLintResult,
  PipelineTemplate,
  TemplateVariable,
} from '../types';

// ---------------------------------------------------------------------------
// App settings commands
// ---------------------------------------------------------------------------

/** Load app-level settings. */
export async function loadAppSettings(): Promise<AppSettings> {
  return invoke<AppSettings>('load_app_settings');
}

/** Save app-level settings. */
export async function saveAppSettings(settings: AppSettings): Promise<void> {
  return invoke<void>('save_app_settings', { settings });
}

/** Store an API key in the OS keychain. */
export async function setAppApiKey(provider: string, value: string): Promise<void> {
  return invoke<void>('set_app_api_key', { provider, value });
}

/** Delete an API key from the OS keychain. */
export async function deleteAppApiKey(provider: string): Promise<void> {
  return invoke<void>('delete_app_api_key', { provider });
}

/** Check if an API key exists in the OS keychain. */
export async function hasAppApiKey(provider: string): Promise<boolean> {
  return invoke<boolean>('has_app_api_key', { provider });
}

/** Get the app data directory path. */
export async function getAppDataDir(): Promise<string> {
  return invoke<string>('get_app_data_dir');
}

/** Get the app version. */
export async function getAppVersion(): Promise<string> {
  return invoke<string>('get_app_version');
}

// ---------------------------------------------------------------------------
// Project commands
// ---------------------------------------------------------------------------

/** List all tracked projects. */
export async function listProjects(): Promise<ProjectInfo[]> {
  return invoke<ProjectInfo[]>('list_projects');
}

/** Add a project by name and local path. */
export async function addProject(name: string, path: string): Promise<Project> {
  return invoke<Project>('add_project', { name, path });
}

/** Remove a project by ID. */
export async function removeProject(id: string): Promise<void> {
  return invoke<void>('remove_project', { id });
}

/** Get Git information for a repository. */
export async function getGitInfo(repoPath: string): Promise<GitInfo> {
  return invoke<GitInfo>('get_git_info', { repoPath });
}

// ---------------------------------------------------------------------------
// Pipeline commands
// ---------------------------------------------------------------------------

/** Detect scripts in a repository. */
export async function detectScripts(repoPath: string): Promise<DetectedScript[]> {
  return invoke<DetectedScript[]>('detect_scripts', { repoPath });
}

/** Generate a draft pipeline from detected scripts. */
export async function generatePipeline(repoPath: string, repoName: string): Promise<Pipeline> {
  return invoke<Pipeline>('generate_pipeline', { repoPath, repoName });
}

/** Save a pipeline to .chibby/pipeline.toml. */
export async function savePipeline(repoPath: string, p: Pipeline): Promise<void> {
  return invoke<void>('save_pipeline', { repoPath, p });
}

/** Load a pipeline from .chibby/pipeline.toml. */
export async function loadPipeline(repoPath: string): Promise<Pipeline> {
  return invoke<Pipeline>('load_pipeline', { repoPath });
}

/** Validate a pipeline against the actual project configuration. */
export async function validatePipeline(repoPath: string): Promise<PipelineValidation> {
  return invoke<PipelineValidation>('validate_pipeline', { repoPath });
}

/** Get GitHub Actions workflows from a repository. */
export async function getGithubWorkflows(repoPath: string): Promise<WorkflowInfo[]> {
  return invoke<WorkflowInfo[]>('get_github_workflows', { repoPath });
}

/** Convert GitHub workflows to Chibby pipeline stages. */
export async function workflowsToPipelineStages(repoPath: string): Promise<Stage[]> {
  return invoke<Stage[]>('workflows_to_pipeline_stages', { repoPath });
}

/** Get CI/CD file recommendations for a repository. */
export async function getRecommendations(repoPath: string): Promise<ProjectRecommendations> {
  return invoke<ProjectRecommendations>('get_recommendations', { repoPath });
}

// ---------------------------------------------------------------------------
// Run commands
// ---------------------------------------------------------------------------

/** Run a pipeline for a given repo path. Optionally run only specific stages. */
export async function runPipeline(
  repoPath: string,
  environment?: string,
  stages?: string[]
): Promise<PipelineRun> {
  return invoke<PipelineRun>('run_pipeline', { repoPath, environment, stages });
}

/** Cancel a running pipeline. */
export async function cancelPipeline(repoPath: string): Promise<void> {
  return invoke<void>('cancel_pipeline', { repoPath });
}

/** Check if a pipeline is currently running for a project. */
export async function isPipelineRunning(repoPath: string): Promise<boolean> {
  return invoke<boolean>('is_pipeline_running', { repoPath });
}

/** Get run history for a project. */
export async function getRunHistory(repoPath: string): Promise<PipelineRun[]> {
  return invoke<PipelineRun[]>('get_run_history', { repoPath });
}

/** Get all runs across all projects. */
export async function getAllRuns(): Promise<PipelineRun[]> {
  return invoke<PipelineRun[]>('get_all_runs');
}

/** Get a single run by ID. */
export async function getRun(id: string): Promise<PipelineRun | null> {
  return invoke<PipelineRun | null>('get_run', { id });
}

/** Retry a failed run, optionally from a specific stage. */
export async function retryRun(runId: string, fromStage?: string): Promise<PipelineRun> {
  return invoke<PipelineRun>('retry_run', { runId, fromStage });
}

/** Roll back to a previously successful run. */
export async function rollbackToRun(targetRunId: string): Promise<PipelineRun> {
  return invoke<PipelineRun>('rollback_to_run', { targetRunId });
}

/** Get the last successful run for a project, optionally filtered by environment. */
export async function getLastSuccessfulRun(
  repoPath: string,
  environment?: string
): Promise<PipelineRun | null> {
  return invoke<PipelineRun | null>('get_last_successful_run', { repoPath, environment });
}

/** Get deployment history for a project and environment. */
export async function getDeploymentHistory(
  repoPath: string,
  environment: string
): Promise<DeploymentRecord[]> {
  return invoke<DeploymentRecord[]>('get_deployment_history', { repoPath, environment });
}

/** Delete a run by ID. */
export async function deleteRun(id: string): Promise<void> {
  return invoke<void>('delete_run', { id });
}

/** Clear all run history for a project. Returns number of runs deleted. */
export async function clearRunHistory(repoPath: string): Promise<number> {
  return invoke<number>('clear_run_history', { repoPath });
}

// ---------------------------------------------------------------------------
// Environment & secrets commands
// ---------------------------------------------------------------------------

/** Load environments config from .chibby/environments.toml. */
export async function loadEnvironments(repoPath: string): Promise<EnvironmentsConfig> {
  return invoke<EnvironmentsConfig>('load_environments', { repoPath });
}

/** Save environments config to .chibby/environments.toml. */
export async function saveEnvironments(
  repoPath: string,
  config: EnvironmentsConfig
): Promise<void> {
  return invoke<void>('save_environments', { repoPath, config });
}

/** Load secrets config from .chibby/secrets.toml. */
export async function loadSecretsConfig(repoPath: string): Promise<SecretsConfig> {
  return invoke<SecretsConfig>('load_secrets_config', { repoPath });
}

/** Save secrets config to .chibby/secrets.toml. */
export async function saveSecretsConfig(repoPath: string, config: SecretsConfig): Promise<void> {
  return invoke<void>('save_secrets_config', { repoPath, config });
}

/** Store a secret value in the OS keychain. */
export async function setSecret(
  projectPath: string,
  envName: string,
  secretName: string,
  value: string
): Promise<void> {
  return invoke<void>('set_secret', { projectPath, envName, secretName, value });
}

/** Delete a secret from the OS keychain. */
export async function deleteSecret(
  projectPath: string,
  envName: string,
  secretName: string
): Promise<void> {
  return invoke<void>('delete_secret', { projectPath, envName, secretName });
}

/** Check which secrets are set in the keychain for an environment. */
export async function checkSecretsStatus(
  projectPath: string,
  envName: string
): Promise<SecretStatus[]> {
  return invoke<SecretStatus[]>('check_secrets_status', { projectPath, envName });
}

/** Test SSH connectivity to a host. */
export async function testSshConnection(host: string, port?: number): Promise<string> {
  return invoke<string>('test_ssh_connection', { host, port });
}

/** Run preflight validation for a pipeline against an environment. */
export async function runPreflight(
  repoPath: string,
  environment: string
): Promise<PreflightResult> {
  return invoke<PreflightResult>('run_preflight', { repoPath, environment });
}

// ---------------------------------------------------------------------------
// Version commands (Phase 5)
// ---------------------------------------------------------------------------

/** Detect version files and current version in a repo. */
export async function detectVersions(repoPath: string): Promise<VersionInfo> {
  return invoke<VersionInfo>('detect_versions', { repoPath });
}

/** Bump the version across all version files. */
export async function bumpVersion(
  repoPath: string,
  level: BumpLevel,
  explicitVersion?: string,
  createTag: boolean = true
): Promise<BumpResult> {
  return invoke<BumpResult>('bump_version', { repoPath, level, explicitVersion, createTag });
}

/** Generate a changelog from commits since a tag. */
export async function generateChangelog(
  repoPath: string,
  sinceTag?: string
): Promise<ChangelogEntry[]> {
  return invoke<ChangelogEntry[]>('generate_changelog', { repoPath, sinceTag });
}

// ---------------------------------------------------------------------------
// Artifact & signing commands (Phase 5)
// ---------------------------------------------------------------------------

/** Load artifact config from .chibby/artifacts.toml. */
export async function loadArtifactConfig(repoPath: string): Promise<ArtifactConfig> {
  return invoke<ArtifactConfig>('load_artifact_config', { repoPath });
}

/** Save artifact config to .chibby/artifacts.toml. */
export async function saveArtifactConfig(repoPath: string, config: ArtifactConfig): Promise<void> {
  return invoke<void>('save_artifact_config', { repoPath, config });
}

/** Collect artifacts matching configured patterns. */
export async function collectArtifacts(
  repoPath: string,
  projectName: string,
  version: string
): Promise<ArtifactManifest> {
  return invoke<ArtifactManifest>('collect_artifacts', { repoPath, projectName, version });
}

/** List all artifact manifests for a project. */
export async function listArtifactManifests(repoPath: string): Promise<ArtifactManifest[]> {
  return invoke<ArtifactManifest[]>('list_artifact_manifests', { repoPath });
}

/** Load signing config from .chibby/signing.toml. */
export async function loadSigningConfig(repoPath: string): Promise<SigningConfig> {
  return invoke<SigningConfig>('load_signing_config', { repoPath });
}

/** Save signing config to .chibby/signing.toml. */
export async function saveSigningConfig(repoPath: string, config: SigningConfig): Promise<void> {
  return invoke<void>('save_signing_config', { repoPath, config });
}

/** Sign an artifact file. */
export async function signArtifact(repoPath: string, artifactPath: string): Promise<SigningResult> {
  return invoke<SigningResult>('sign_artifact', { repoPath, artifactPath });
}

/** Check whether signing tools are available on this platform. */
export async function checkSigningTools(): Promise<string[]> {
  return invoke<string[]>('check_signing_tools');
}

// ---------------------------------------------------------------------------
// Notification commands (Phase 5)
// ---------------------------------------------------------------------------

/** Load notification config from .chibby/notify.toml. */
export async function loadNotifyConfig(repoPath: string): Promise<NotifyConfig> {
  return invoke<NotifyConfig>('load_notify_config', { repoPath });
}

/** Save notification config to .chibby/notify.toml. */
export async function saveNotifyConfig(repoPath: string, config: NotifyConfig): Promise<void> {
  return invoke<void>('save_notify_config', { repoPath, config });
}

/** Send a test notification using the current config. */
export async function sendTestNotification(repoPath: string): Promise<string> {
  return invoke<string>('send_test_notification', { repoPath });
}

// ---------------------------------------------------------------------------
// Cleanup commands (Phase 5)
// ---------------------------------------------------------------------------

/** Load cleanup config from .chibby/cleanup.toml. */
export async function loadCleanupConfig(repoPath: string): Promise<CleanupConfig> {
  return invoke<CleanupConfig>('load_cleanup_config', { repoPath });
}

/** Save cleanup config to .chibby/cleanup.toml. */
export async function saveCleanupConfig(repoPath: string, config: CleanupConfig): Promise<void> {
  return invoke<void>('save_cleanup_config', { repoPath, config });
}

/** Run cleanup (artifact pruning + run history pruning). */
export async function runCleanup(
  repoPath: string,
  dryRun: boolean = false
): Promise<CleanupResult> {
  return invoke<CleanupResult>('run_cleanup', { repoPath, dryRun });
}

// ---------------------------------------------------------------------------
// Updater commands (Phase 5.5)
// ---------------------------------------------------------------------------

/** Load updater config from .chibby/updater.toml. */
export async function loadUpdaterConfig(repoPath: string): Promise<UpdaterConfig> {
  return invoke<UpdaterConfig>('load_updater_config', { repoPath });
}

/** Save updater config to .chibby/updater.toml. */
export async function saveUpdaterConfig(repoPath: string, config: UpdaterConfig): Promise<void> {
  return invoke<void>('save_updater_config', { repoPath, config });
}

/** Generate a Tauri update key pair. Private key stored in OS keychain. */
export async function generateUpdateKeys(repoPath: string): Promise<UpdateKeyResult> {
  return invoke<UpdateKeyResult>('generate_update_keys', { repoPath });
}

/** Import an existing Tauri update private key into the OS keychain. */
export async function importUpdatePrivateKey(repoPath: string, privateKey: string): Promise<void> {
  return invoke<void>('import_update_private_key', { repoPath, privateKey });
}

/** Check if the Tauri update private key exists in the OS keychain. */
export async function hasUpdateKey(repoPath: string): Promise<boolean> {
  return invoke<boolean>('has_update_key', { repoPath });
}

/** Rotate the update key pair (generate new, update config). */
export async function rotateUpdateKeys(repoPath: string): Promise<UpdateKeyResult> {
  return invoke<UpdateKeyResult>('rotate_update_keys', { repoPath });
}

/** Delete the update private key from the OS keychain. */
export async function deleteUpdateKey(repoPath: string): Promise<void> {
  return invoke<void>('delete_update_key', { repoPath });
}

/** Run updater preflight checks. Returns list of issues (empty = all good). */
export async function updaterPreflight(repoPath: string): Promise<string[]> {
  return invoke<string[]>('updater_preflight', { repoPath });
}

/** Sign an update bundle with the Tauri update key. */
export async function signUpdateBundle(
  repoPath: string,
  filePath: string
): Promise<UpdateSignResult> {
  return invoke<UpdateSignResult>('sign_update_bundle', { repoPath, filePath });
}

/** Generate a Tauri-compatible latest.json from artifact manifest. */
export async function generateLatestJson(
  repoPath: string,
  version: string,
  notes?: string
): Promise<LatestJsonResult> {
  return invoke<LatestJsonResult>('generate_latest_json', { repoPath, version, notes });
}

/** Merge a per-platform latest.json fragment into an existing file. */
export async function mergeLatestJson(
  targetPath: string,
  fragment: TauriLatestJson
): Promise<TauriLatestJson> {
  return invoke<TauriLatestJson>('merge_latest_json', { targetPath, fragment });
}

/** Check if the Tauri CLI is available. */
export async function checkTauriCli(): Promise<string> {
  return invoke<string>('check_tauri_cli');
}

/** Publish update artifacts and latest.json to configured target. */
export async function publishUpdate(
  repoPath: string,
  version: string,
  dryRun: boolean = false
): Promise<UpdatePublishResult> {
  return invoke<UpdatePublishResult>('publish_update', { repoPath, version, dryRun });
}

// ---------------------------------------------------------------------------
// Security & quality gate commands (Phase 5.8)
// ---------------------------------------------------------------------------

/** Load gates config from .chibby/gates.toml. */
export async function loadGatesConfig(repoPath: string): Promise<GatesConfig> {
  return invoke<GatesConfig>('load_gates_config', { repoPath });
}

/** Save gates config to .chibby/gates.toml. */
export async function saveGatesConfig(repoPath: string, config: GatesConfig): Promise<void> {
  return invoke<void>('save_gates_config', { repoPath, config });
}

/** Run all enabled security and quality gates. */
export async function runGates(repoPath: string): Promise<GatesResult> {
  return invoke<GatesResult>('run_gates', { repoPath });
}

/** Run secret scanning only. */
export async function runSecretScan(repoPath: string): Promise<SecretScanResult> {
  return invoke<SecretScanResult>('run_secret_scan', { repoPath });
}

/** Run dependency/CVE audit only. */
export async function runDependencyAudit(repoPath: string): Promise<AuditResult> {
  return invoke<AuditResult>('run_dependency_audit', { repoPath });
}

/** Run commit message linting only. */
export async function runCommitLint(repoPath: string): Promise<CommitLintResult> {
  return invoke<CommitLintResult>('run_commit_lint', { repoPath });
}

/** Create a secret scan baseline (marks existing findings as acknowledged). */
export async function createSecretScanBaseline(repoPath: string): Promise<string> {
  return invoke<string>('create_secret_scan_baseline', { repoPath });
}

// ---------------------------------------------------------------------------
// Phase 8: Agent commands
// ---------------------------------------------------------------------------

import type {
  AgentAnalysis,
  AgentResponse,
  AgentSystemStatus,
  GeneratedPipeline,
  MemoryEntry,
  PipelineFormat,
} from '../types';

/** Get the current status of the agent system. */
export async function getAgentStatus(): Promise<AgentSystemStatus> {
  return invoke<AgentSystemStatus>('get_agent_status');
}

/** Analyze a pipeline run with the CI/CD agent. */
export async function analyzeRun(runId: string): Promise<AgentAnalysis> {
  return invoke<AgentAnalysis>('analyze_run', { runId });
}

/** Chat with the CI/CD agent. */
export async function agentChat(
  message: string,
  projectId?: string,
  runId?: string
): Promise<AgentResponse> {
  return invoke<AgentResponse>('agent_chat', { message, projectId, runId });
}

/** Generate a pipeline config for a project. */
export async function generatePipelineConfig(
  projectPath: string,
  format: PipelineFormat,
  projectInfo: string
): Promise<GeneratedPipeline> {
  return invoke<GeneratedPipeline>('generate_pipeline_config', {
    projectPath,
    format,
    projectInfo,
  });
}

/** Save a generated pipeline to disk. */
export async function saveGeneratedPipeline(
  projectPath: string,
  filePath: string,
  content: string
): Promise<void> {
  return invoke<void>('save_generated_pipeline', { projectPath, filePath, content });
}

/** Get agent memories for a project or global. */
export async function getAgentMemories(projectId?: string): Promise<MemoryEntry[]> {
  return invoke<MemoryEntry[]>('get_agent_memories', { projectId });
}

/** Delete an agent memory by key. */
export async function deleteAgentMemory(key: string, projectId?: string): Promise<void> {
  return invoke<void>('delete_agent_memory', { key, projectId });
}

/** Rebuild the agent (e.g., after changing API keys). */
export async function rebuildAgent(): Promise<AgentSystemStatus> {
  return invoke<AgentSystemStatus>('rebuild_agent');
}

// ---------------------------------------------------------------------------
// Template commands
// ---------------------------------------------------------------------------

/** Get all templates (built-in + user + project), merged and de-duplicated. */
export async function getTemplates(repoPath?: string): Promise<PipelineTemplate[]> {
  return invoke<PipelineTemplate[]>('get_templates', { repoPath: repoPath ?? null });
}

/** Get a single template by name. */
export async function getTemplate(name: string, repoPath?: string): Promise<PipelineTemplate> {
  return invoke<PipelineTemplate>('get_template', { name, repoPath: repoPath ?? null });
}

/** Extract the {{variable}} placeholders from a template. */
export async function getTemplateVariables(
  name: string,
  repoPath?: string
): Promise<TemplateVariable[]> {
  return invoke<TemplateVariable[]>('get_template_variables', {
    name,
    repoPath: repoPath ?? null,
  });
}

/** Apply a template with variable values, producing a concrete Pipeline. */
export async function applyTemplate(
  name: string,
  variables: Record<string, string>,
  repoPath?: string
): Promise<Pipeline> {
  return invoke<Pipeline>('apply_template', {
    name,
    repoPath: repoPath ?? null,
    variables,
  });
}

/** Save a custom template to user-global or project scope. */
export async function saveCustomTemplate(
  template: PipelineTemplate,
  scope: 'user' | 'project',
  repoPath?: string
): Promise<void> {
  return invoke<void>('save_custom_template', {
    template,
    scope,
    repoPath: repoPath ?? null,
  });
}

/** Delete a custom template from user-global or project scope. */
export async function deleteCustomTemplate(
  name: string,
  scope: 'user' | 'project',
  repoPath?: string
): Promise<void> {
  return invoke<void>('delete_custom_template', {
    name,
    scope,
    repoPath: repoPath ?? null,
  });
}

/** Export a template as a TOML string for sharing. */
export async function exportTemplate(name: string, repoPath?: string): Promise<string> {
  return invoke<string>('export_template', { name, repoPath: repoPath ?? null });
}

/** Import a template from a TOML string and save to the given scope. */
export async function importTemplate(
  tomlContent: string,
  scope: 'user' | 'project',
  repoPath?: string
): Promise<PipelineTemplate> {
  return invoke<PipelineTemplate>('import_template', {
    tomlContent,
    scope,
    repoPath: repoPath ?? null,
  });
}
