import { useEffect, useState } from 'react';
import { useParams, Link, useLocation, useNavigate } from 'react-router-dom';
import { ArrowLeft, Shield, History, Layers, Server, Rocket } from 'lucide-react';
import {
  listProjects,
  loadPipeline,
  listPipelines,
  loadPipelineByName,
  cancelPipeline,
  getRunHistory,
  removeProject,
  loadEnvironments,
  loadSecretsConfig,
  runPreflight,
  detectScripts,
  generatePipeline,
  savePipeline,
  validatePipeline,
  getGitInfo,
  getRecommendations,
  getLastSuccessfulRun,
  clearRunHistory,
} from '../services/api';
import type {
  ProjectInfo,
  Pipeline,
  PipelineRun,
  EnvironmentsConfig,
  SecretsConfig,
  PreflightResult,
  DetectedScript,
  PipelineValidation,
  GitInfo,
  ProjectRecommendations,
  StageResult,
} from '../types';
import BootstrapWizardModal from './BootstrapWizardModal';
import ImporterModal from './ImporterModal';
import ExportDotenvModal from './ExportDotenvModal';
import { useActiveRun, startRun, isRepoRunning, clearRun } from '../services/runStore';
import type { StageStatus, CmdStatus } from '../services/runStore';
import { type TabId } from './project-detail/helpers';
import ProjectHeader from './project-detail/ProjectHeader';
import ProjectAlerts from './project-detail/ProjectAlerts';
import PipelineTab from './project-detail/PipelineTab';
import HistoryTab from './project-detail/HistoryTab';
import EnvironmentsTab from './project-detail/EnvironmentsTab';
import ReleaseTab from './project-detail/ReleaseTab';
import QualityTab from './project-detail/QualityTab';
import ProjectSidebar from './project-detail/ProjectSidebar';

// Stable empty references so store-less renders don't churn identity each pass.
const EMPTY_STAGE_STATUSES: Record<string, StageStatus> = {};
const EMPTY_CMD_STATUSES: Record<string, CmdStatus> = {};
const EMPTY_LIVE_OUTPUT: Record<string, string[]> = {};

function ProjectDetail() {
  const { projectId } = useParams<{ projectId: string }>();
  const location = useLocation();
  const navigate = useNavigate();
  const incomingTab = (location.state as { tab?: TabId | 'settings' })?.tab;
  const initialTab: TabId | undefined =
    incomingTab === 'settings' ? 'environments' : (incomingTab as TabId | undefined);

  const [project, setProject] = useState<ProjectInfo | null>(null);
  const [pipeline, setPipeline] = useState<Pipeline | null>(null);
  const [pipelineNames, setPipelineNames] = useState<string[]>([]);
  const [selectedPipeline, setSelectedPipeline] = useState<string>('pipeline');
  const [runs, setRuns] = useState<PipelineRun[]>([]);
  const [error, setError] = useState<string | null>(null);

  // Environment & secrets state
  const [envsConfig, setEnvsConfig] = useState<EnvironmentsConfig>({ environments: [] });
  const [secretsConfig, setSecretsConfig] = useState<SecretsConfig>({ secrets: [] });
  const [selectedEnv, setSelectedEnv] = useState<string>('');
  const [preflightResult, setPreflightResult] = useState<PreflightResult | null>(null);
  const [preflighting, setPreflighting] = useState(false);

  // Detected CI/CD files
  const [detectedFiles, setDetectedFiles] = useState<DetectedScript[]>([]);

  // Pipeline validation
  const [validation, setValidation] = useState<PipelineValidation | null>(null);

  // Git info
  const [gitInfo, setGitInfo] = useState<GitInfo | null>(null);

  // CI/CD Recommendations
  const [recommendations, setRecommendations] = useState<ProjectRecommendations | null>(null);
  const [loadingRecommendations, setLoadingRecommendations] = useState(false);

  // Pipeline editing
  const [editingPipeline, setEditingPipeline] = useState(false);

  // Phase 6: Last known good and deployment history
  const [lastGoodRun, setLastGoodRun] = useState<PipelineRun | null>(null);
  const [historyEnvFilter, setHistoryEnvFilter] = useState<string>('');

  // Filtered runs for history tab
  const filteredRuns = historyEnvFilter
    ? runs.filter((r) => r.environment === historyEnvFilter)
    : runs;

  // Live run state comes from the global run store (survives navigation and
  // supports concurrent runs of different projects). See services/runStore.ts.
  const activeRun = useActiveRun(project?.project.path);
  const running = activeRun?.status === 'running';
  const stageStatuses = activeRun?.stageStatuses ?? EMPTY_STAGE_STATUSES;
  const cmdStatuses = activeRun?.cmdStatuses ?? EMPTY_CMD_STATUSES;
  const liveOutput = activeRun?.liveOutput ?? EMPTY_LIVE_OUTPUT;

  // Collapsible sections
  const [showEnvSection, setShowEnvSection] = useState(true);
  const [showSecretsSection, setShowSecretsSection] = useState(true);

  // Environments tab modals
  const [showBootstrap, setShowBootstrap] = useState(false);
  const [showImporter, setShowImporter] = useState(false);
  const [showExportDotenv, setShowExportDotenv] = useState(false);

  const [activeTab, setActiveTab] = useState<TabId>(initialTab ?? 'pipeline');

  // Selected stage result for inline viewing
  const [selectedStageResult, setSelectedStageResult] = useState<StageResult | null>(null);

  useEffect(() => {
    loadData();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [projectId]);

  async function loadData() {
    try {
      const projects = await listProjects();
      const p = projects.find((pi) => pi.project.id === projectId) ?? null;
      setProject(p);

      if (p?.has_pipeline) {
        // Load available pipelines
        const names = await listPipelines(p.project.path).catch(() => ['pipeline']);
        setPipelineNames(names);

        // Load the selected pipeline (or default)
        const pl =
          selectedPipeline === 'pipeline'
            ? await loadPipeline(p.project.path)
            : await loadPipelineByName(p.project.path, selectedPipeline);
        setPipeline(pl);
        // Validate the pipeline
        try {
          const val = await validatePipeline(p.project.path);
          setValidation(val);
        } catch {
          // Validation is non-critical, continue without it
          setValidation(null);
        }
      } else {
        setValidation(null);
      }

      if (p) {
        const [h, envs, secs, detected, git] = await Promise.all([
          getRunHistory(p.project.path),
          loadEnvironments(p.project.path),
          loadSecretsConfig(p.project.path),
          detectScripts(p.project.path),
          getGitInfo(p.project.path),
        ]);
        setRuns(h);
        setEnvsConfig(envs ?? { environments: [] });
        setSecretsConfig(secs ?? { secrets: [] });
        setDetectedFiles(detected ?? []);
        setGitInfo(git ?? null);

        // Load last successful run (non-blocking)
        getLastSuccessfulRun(p.project.path)
          .then((r) => setLastGoodRun(r))
          .catch(() => setLastGoodRun(null));

        // Load recommendations (non-blocking)
        setLoadingRecommendations(true);
        getRecommendations(p.project.path)
          .then((recs) => setRecommendations(recs))
          .catch(() => setRecommendations(null))
          .finally(() => setLoadingRecommendations(false));
      }
    } catch (err) {
      setError(String(err));
    }
  }

  async function handleSwitchPipeline(name: string) {
    if (!project) return;
    try {
      setSelectedPipeline(name);
      if (!running) clearRun(project.project.path);
      setSelectedStageResult(null);
      const pl =
        name === 'pipeline'
          ? await loadPipeline(project.project.path)
          : await loadPipelineByName(project.project.path, name);
      setPipeline(pl);
      // Re-validate
      try {
        const val = await validatePipeline(project.project.path);
        setValidation(val);
      } catch {
        setValidation(null);
      }
    } catch (err) {
      setError(String(err));
    }
  }

  function handleRun(stages?: string[]) {
    if (!project || !pipeline) return;
    // Guard: don't start a second concurrent run for the same project.
    if (isRepoRunning(project.project.path)) return;

    setError(null);
    setPreflightResult(null);
    setSelectedStageResult(null);

    // Fire-and-forget: the run store owns live progress and keeps running even
    // if we navigate away. Completion is reconciled via the effect below.
    startRun({
      repoPath: project.project.path,
      pipeline,
      projectId: project.project.id,
      projectName: project.project.name,
      environment: selectedEnv || undefined,
      stages,
      pipelineFile: selectedPipeline !== 'pipeline' ? selectedPipeline : undefined,
    });
  }

  // When the active run finishes, refresh history and surface any failed stage.
  const finishedRunId = activeRun?.finishedRun?.id;
  useEffect(() => {
    if (!finishedRunId) return;
    const failed = activeRun?.finishedRun?.stage_results?.find((r) => r.status === 'failed');
    // eslint-disable-next-line react-hooks/set-state-in-effect
    if (failed) setSelectedStageResult(failed);
    loadData();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [finishedRunId]);

  function handleBannerRetry(fromStage?: string) {
    if (!pipeline) return;
    if (fromStage) {
      // Run from the failed stage onwards
      const stageIdx = pipeline.stages.findIndex((s) => s.name === fromStage);
      if (stageIdx >= 0) {
        const stagesToRun = pipeline.stages.slice(stageIdx).map((s) => s.name);
        handleRun(stagesToRun);
      }
    } else {
      handleRun();
    }
  }

  async function handlePreflight() {
    if (!project || !selectedEnv) return;
    try {
      setPreflighting(true);
      setError(null);
      const result = await runPreflight(project.project.path, selectedEnv);
      setPreflightResult(result);
    } catch (err) {
      setError(String(err));
    } finally {
      setPreflighting(false);
    }
  }

  async function handleClearHistory() {
    if (!project) return;
    if (!window.confirm('Clear all run history for this project?')) return;
    try {
      await clearRunHistory(project.project.path);
      setRuns([]);
      setLastGoodRun(null);
    } catch (err) {
      setError(String(err));
    }
  }

  async function handleCancel() {
    if (!project) return;
    try {
      await cancelPipeline(project.project.path);
    } catch (err) {
      setError(String(err));
    }
  }

  async function handleRegenerate() {
    if (!project) return;
    try {
      setError(null);
      // Clear pipeline run statuses when regenerating
      if (!running) clearRun(project.project.path);
      setSelectedStageResult(null);
      const newPipeline = await generatePipeline(project.project.path, project.project.name);
      await savePipeline(project.project.path, newPipeline);
      await loadData();
    } catch (err) {
      setError(String(err));
    }
  }

  async function handleDelete() {
    if (!project) return;
    try {
      await removeProject(project.project.id);
      navigate('/');
    } catch (err) {
      setError(String(err));
    }
  }

  if (!project) {
    return (
      <div className="page">
        <div className="loading">Loading project...</div>
      </div>
    );
  }

  return (
    <div className="page page-with-sidebar">
      <div className="page-main">
        <Link to="/projects" className="back-link">
          <ArrowLeft size={16} /> Back to Projects
        </Link>

        <ProjectHeader
          project={project}
          gitInfo={gitInfo}
          pipelineNames={pipelineNames}
          selectedPipeline={selectedPipeline}
          onSwitchPipeline={handleSwitchPipeline}
          envsConfig={envsConfig}
          selectedEnv={selectedEnv}
          onSelectedEnvChange={(value) => {
            setSelectedEnv(value);
            setPreflightResult(null);
          }}
          running={running}
          preflighting={preflighting}
          onPreflight={handlePreflight}
          onRun={() => handleRun()}
          onCancel={handleCancel}
          onDelete={handleDelete}
        />

        <ProjectAlerts error={error} preflightResult={preflightResult} validation={validation} />

        {/* Tab navigation */}
        <div className="project-tabs">
          <button
            className={`project-tab ${activeTab === 'pipeline' ? 'project-tab-active' : ''}`}
            onClick={() => setActiveTab('pipeline')}
          >
            <Layers size={16} />
            Pipeline
            {pipeline && <span className="badge badge-neutral">{pipeline.stages.length}</span>}
          </button>
          <button
            className={`project-tab ${activeTab === 'history' ? 'project-tab-active' : ''}`}
            onClick={() => setActiveTab('history')}
          >
            <History size={16} />
            History
            {runs.length > 0 && <span className="badge badge-neutral">{runs.length}</span>}
          </button>
          <button
            className={`project-tab ${activeTab === 'environments' ? 'project-tab-active' : ''}`}
            onClick={() => setActiveTab('environments')}
          >
            <Server size={16} />
            Environments
            {envsConfig?.environments?.length ? (
              <span className="badge badge-neutral">{envsConfig.environments.length}</span>
            ) : null}
          </button>
          <button
            className={`project-tab ${activeTab === 'release' ? 'project-tab-active' : ''}`}
            onClick={() => setActiveTab('release')}
          >
            <Rocket size={16} />
            Release
          </button>
          <button
            className={`project-tab ${activeTab === 'quality' ? 'project-tab-active' : ''}`}
            onClick={() => setActiveTab('quality')}
          >
            <Shield size={16} />
            Quality
          </button>
        </div>

        {/* Tab content */}
        <div className="project-tab-content">
          {/* Pipeline Tab */}
          {activeTab === 'pipeline' && (
            <PipelineTab
              repoPath={project.project.path}
              pipeline={pipeline}
              runs={runs}
              running={running}
              stageStatuses={stageStatuses}
              cmdStatuses={cmdStatuses}
              liveOutput={liveOutput}
              editingPipeline={editingPipeline}
              selectedStageResult={selectedStageResult}
              onSelectStageResult={setSelectedStageResult}
              onToggleEditing={() => setEditingPipeline(!editingPipeline)}
              onRun={handleRun}
              onBannerRetry={handleBannerRetry}
              onRegenerate={handleRegenerate}
              onPipelineSaved={() => {
                setEditingPipeline(false);
                loadData();
              }}
            />
          )}

          {/* History Tab */}
          {activeTab === 'history' && (
            <HistoryTab
              runs={runs}
              filteredRuns={filteredRuns}
              lastGoodRun={lastGoodRun}
              envsConfig={envsConfig}
              historyEnvFilter={historyEnvFilter}
              onHistoryEnvFilterChange={setHistoryEnvFilter}
              onClearHistory={handleClearHistory}
              projectId={projectId}
            />
          )}

          {/* Environments Tab — env + secrets + bootstrap + importers + leaks + audit + export */}
          {activeTab === 'environments' && (
            <EnvironmentsTab
              repoPath={project.project.path}
              envsConfig={envsConfig}
              secretsConfig={secretsConfig}
              showEnvSection={showEnvSection}
              showSecretsSection={showSecretsSection}
              onToggleEnvSection={() => setShowEnvSection(!showEnvSection)}
              onToggleSecretsSection={() => setShowSecretsSection(!showSecretsSection)}
              onShowBootstrap={() => setShowBootstrap(true)}
              onShowImporter={() => setShowImporter(true)}
              onShowExportDotenv={() => setShowExportDotenv(true)}
              onReload={loadData}
            />
          )}

          {/* Release Tab */}
          {activeTab === 'release' && <ReleaseTab repoPath={project.project.path} />}

          {/* Quality Tab */}
          {activeTab === 'quality' && (
            <QualityTab repoPath={project.project.path} environments={envsConfig.environments} />
          )}
        </div>
      </div>

      {/* Right sidebar */}
      <ProjectSidebar
        project={project}
        pipeline={pipeline}
        envsConfig={envsConfig}
        secretsConfig={secretsConfig}
        runs={runs}
        gitInfo={gitInfo}
        detectedFiles={detectedFiles}
        recommendations={recommendations}
        loadingRecommendations={loadingRecommendations}
      />

      {showBootstrap && (
        <BootstrapWizardModal
          repoPath={project.project.path}
          onClose={() => setShowBootstrap(false)}
          onApplied={() => {
            setShowBootstrap(false);
            loadData();
          }}
        />
      )}
      {showImporter && (
        <ImporterModal
          repoPath={project.project.path}
          environments={envsConfig.environments.map((e) => e.name)}
          onClose={() => setShowImporter(false)}
          onDone={loadData}
        />
      )}
      {showExportDotenv && (
        <ExportDotenvModal
          repoPath={project.project.path}
          environments={envsConfig.environments}
          onClose={() => setShowExportDotenv(false)}
        />
      )}
    </div>
  );
}

export default ProjectDetail;
