import { useEffect, useRef, useState } from 'react';
import { useParams, Link, useLocation, useNavigate } from 'react-router-dom';
import {
  Play,
  Clock,
  GitBranch,
  Trash2,
  ArrowLeft,
  ChevronDown,
  ChevronRight,
  CheckCircle,
  AlertTriangle,
  Shield,
  Settings,
  FileCode,
  Cog,
  RefreshCw,
  Copy,
  Loader2,
  XCircle,
  Circle,
  History,
  Layers,
  Key,
  Server,
  Square,
  Eye,
  RotateCcw,
  Undo2,
  Trophy,
} from 'lucide-react';
import {
  listProjects,
  loadPipeline,
  runPipeline,
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
  isPipelineRunning,
} from '../services/api';
import { formatDate, formatDuration, statusClass, capitalize } from '../utils/format';
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
import EnvironmentEditor from './EnvironmentEditor';
import PipelineEditor from './PipelineEditor';
import SecretsManager from './SecretsManager';
import RecommendationsPanel from './RecommendationsPanel';
import LogViewer from './LogViewer';
import { FileTypeIcon } from './FileTypeIcon';
import { listen } from '@tauri-apps/api/event';

/** Strip ANSI escape sequences from a string. */
// eslint-disable-next-line no-control-regex
const ANSI_RE = /\x1b\[[0-9;]*[A-Za-z]|\x1b\].*?\x07/g;
function stripAnsi(text: string): string {
  return text.replace(ANSI_RE, '');
}

type TabId = 'pipeline' | 'history' | 'settings';

function ProjectDetail() {
  const { projectId } = useParams<{ projectId: string }>();
  const location = useLocation();
  const navigate = useNavigate();
  const initialTab = (location.state as { tab?: TabId })?.tab;

  const [project, setProject] = useState<ProjectInfo | null>(null);
  const [pipeline, setPipeline] = useState<Pipeline | null>(null);
  const [runs, setRuns] = useState<PipelineRun[]>([]);
  const [running, setRunning] = useState(false);
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

  // Determine whether a run was full pipeline or partial (single/few stages)
  function runScopeLabel(run: PipelineRun): { label: string; isPartial: boolean } {
    const executed = run.stage_results.filter((s) => s.status !== 'skipped');
    const total = run.stage_results.length;
    if (total === 0 || executed.length === total) {
      return { label: run.pipeline_name, isPartial: false };
    }
    if (executed.length === 1) {
      return { label: executed[0].stage_name, isPartial: true };
    }
    return { label: `${executed.length} of ${total} stages`, isPartial: true };
  }

  // Stage execution tracking
  type StageStatus = 'pending' | 'running' | 'success' | 'failed' | 'skipped';
  const [stageStatuses, setStageStatuses] = useState<Record<string, StageStatus>>({});
  const [runningStageName, setRunningStageName] = useState<string | null>(null);

  // Per-command status tracking: "stageName:cmdIndex" -> status
  type CmdStatus = 'pending' | 'running' | 'done' | 'failed';
  const [cmdStatuses, setCmdStatuses] = useState<Record<string, CmdStatus>>({});
  const cmdStatusRef = useRef<Record<string, CmdStatus>>({});

  // Collapsible sections
  const [showEnvSection, setShowEnvSection] = useState(false);
  const [showSecretsSection, setShowSecretsSection] = useState(false);

  const [activeTab, setActiveTab] = useState<TabId>(initialTab ?? 'pipeline');

  // Selected stage result for inline viewing
  const [selectedStageResult, setSelectedStageResult] = useState<StageResult | null>(null);

  // Live output lines for running stages (last 5 lines)
  const [liveOutput, setLiveOutput] = useState<Record<string, string[]>>({});
  const liveOutputRef = useRef<Record<string, string[]>>({});

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
        const pl = await loadPipeline(p.project.path);
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

        // Check if a pipeline is already running (e.g. retry from RunDetail)
        isPipelineRunning(p.project.path)
          .then((isRunning) => {
            if (isRunning && !running) {
              setRunning(true);
              // Poll until the pipeline finishes
              const interval = setInterval(async () => {
                const stillRunning = await isPipelineRunning(p.project.path).catch(() => false);
                if (!stillRunning) {
                  clearInterval(interval);
                  setRunning(false);
                  // Refresh data to show new run results
                  loadData();
                }
              }, 2000);
            }
          })
          .catch(() => {});
      }
    } catch (err) {
      setError(String(err));
    }
  }

  async function handleRun(stages?: string[]) {
    if (!project || !pipeline) return;
    try {
      setRunning(true);
      setError(null);
      setPreflightResult(null);
      setSelectedStageResult(null);
      setLiveOutput({});
      liveOutputRef.current = {};

      // Initialize stage statuses
      const stagesToRun = stages ?? pipeline.stages.map((s) => s.name);
      const initialStatuses: Record<string, StageStatus> = {};
      pipeline.stages.forEach((s) => {
        if (stagesToRun.includes(s.name)) {
          initialStatuses[s.name] = 'pending';
        }
      });
      setStageStatuses(initialStatuses);

      // Initialize per-command statuses
      const initialCmds: Record<string, CmdStatus> = {};
      pipeline.stages.forEach((s) => {
        if (stagesToRun.includes(s.name)) {
          s.commands.forEach((_, ci) => {
            initialCmds[`${s.name}:${ci}`] = 'pending';
          });
        }
      });
      setCmdStatuses(initialCmds);
      cmdStatusRef.current = { ...initialCmds };

      // Track which command index we're on per stage
      const stageCmdIdx: Record<string, number> = {};

      // Listen for real-time pipeline log events
      const unlisten = await listen<{ stage: string; type: string; message: string }>(
        'pipeline:log',
        (event) => {
          const { stage, type: logType, message } = event.payload;

          if (logType === 'info' && message.startsWith('--- Starting stage:')) {
            // Stage is starting
            setRunningStageName(stage);
            setStageStatuses((prev) => {
              const updated = { ...prev, [stage]: 'running' as StageStatus };
              // Mark previous running stages as success (they completed if we moved on)
              // and finalize their command statuses
              for (const key of Object.keys(updated)) {
                if (key !== stage && updated[key] === 'running') {
                  updated[key] = 'success';
                  // Mark all commands for the completed stage as done
                  const completedStage = pipeline?.stages.find((s) => s.name === key);
                  if (completedStage) {
                    completedStage.commands.forEach((_, ci) => {
                      const cmdKey = `${key}:${ci}`;
                      if (
                        cmdStatusRef.current[cmdKey] === 'running' ||
                        cmdStatusRef.current[cmdKey] === 'pending'
                      ) {
                        cmdStatusRef.current[cmdKey] = 'done';
                      }
                    });
                    setCmdStatuses({ ...cmdStatusRef.current });
                  }
                }
              }
              return updated;
            });
            stageCmdIdx[stage] = 0;
          } else if (logType === 'cmd') {
            // A command is about to run — mark it as running
            const idx = stageCmdIdx[stage] ?? 0;
            const key = `${stage}:${idx}`;

            // Mark previous command as done if there was one
            if (idx > 0) {
              const prevKey = `${stage}:${idx - 1}`;
              cmdStatusRef.current[prevKey] = 'done';
            }
            cmdStatusRef.current[key] = 'running';
            setCmdStatuses({ ...cmdStatusRef.current });

            stageCmdIdx[stage] = idx + 1;
          } else if (logType === 'error') {
            // Command failed
            const idx = (stageCmdIdx[stage] ?? 1) - 1;
            const key = `${stage}:${idx}`;
            cmdStatusRef.current[key] = 'failed';
            setCmdStatuses({ ...cmdStatusRef.current });
          } else if (logType === 'stdout' || logType === 'stderr') {
            // Capture live output lines (keep last MAX_LIVE_LINES), strip ANSI
            const clean = stripAnsi(message);
            const currentLines = liveOutputRef.current[stage] || [];
            const newLines = [...currentLines, clean].slice(-5);
            liveOutputRef.current[stage] = newLines;
            setLiveOutput({ ...liveOutputRef.current });
          }
        }
      );

      // Run the pipeline
      await runPipeline(project.project.path, selectedEnv || undefined, stages);

      // Clean up listener
      unlisten();

      // After completion, fetch results and update statuses
      const history = await getRunHistory(project.project.path);
      setRuns(history);

      // Get latest run to check stage results and finalize command statuses
      if (history.length > 0) {
        const latestRun = history[0];
        const finalStatuses: Record<string, StageStatus> = {};
        const finalCmds = { ...cmdStatusRef.current };
        latestRun.stage_results?.forEach((result) => {
          finalStatuses[result.stage_name] = result.status as StageStatus;
          // Finalize command statuses for this stage
          const stageObj = pipeline.stages.find((s) => s.name === result.stage_name);
          if (stageObj) {
            stageObj.commands.forEach((_, ci) => {
              const key = `${result.stage_name}:${ci}`;
              if (result.status === 'success') {
                finalCmds[key] = 'done';
              } else if (result.status === 'skipped') {
                finalCmds[key] = 'pending';
              }
              // 'failed' commands keep their status from the event stream
            });
          }
        });
        setStageStatuses(finalStatuses);
        setCmdStatuses(finalCmds);
        cmdStatusRef.current = finalCmds;
      }

      setRunningStageName(null);
      await loadData();
    } catch (err) {
      setError(String(err));
      // Mark currently running stage as failed
      if (runningStageName) {
        setStageStatuses((prev) => ({ ...prev, [runningStageName]: 'failed' }));
      }
      setRunningStageName(null);
    } finally {
      setRunning(false);
      // Keep stage statuses visible - they'll be cleared on next run or regenerate
    }
  }

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
      setStageStatuses({});
      setCmdStatuses({});
      cmdStatusRef.current = {};
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

        <header className="page-header">
          <div>
            <h2 className="page-title">{project.project.name}</h2>
            <p className="page-subtitle">{project.project.path}</p>
            {gitInfo?.branch && (
              <div className="git-info">
                <GitBranch size={14} />
                <span className="git-branch">{gitInfo.branch}</span>
                {gitInfo.commit && <span className="git-commit">{gitInfo.commit}</span>}
                {gitInfo.is_dirty && (
                  <span className="git-dirty" title="Uncommitted changes">
                    modified
                  </span>
                )}
                {gitInfo.ahead !== undefined && gitInfo.ahead > 0 && (
                  <span className="git-ahead" title={`${gitInfo.ahead} commits ahead of remote`}>
                    +{gitInfo.ahead}
                  </span>
                )}
                {gitInfo.behind !== undefined && gitInfo.behind > 0 && (
                  <span className="git-behind" title={`${gitInfo.behind} commits behind remote`}>
                    -{gitInfo.behind}
                  </span>
                )}
              </div>
            )}
          </div>
          <div className="header-actions">
            {/* Environment selector */}
            {envsConfig?.environments?.length > 0 && (
              <select
                className="input input-sm env-select"
                value={selectedEnv}
                onChange={(e) => {
                  setSelectedEnv(e.target.value);
                  setPreflightResult(null);
                }}
              >
                <option value="">No environment</option>
                {envsConfig?.environments?.map((env) => (
                  <option key={env.name} value={env.name}>
                    {env.name}
                  </option>
                ))}
              </select>
            )}

            {/* Preflight button */}
            {selectedEnv && (
              <button
                className="btn btn-secondary"
                onClick={handlePreflight}
                disabled={preflighting}
              >
                <Shield size={16} />
                {preflighting ? 'Checking...' : 'Preflight'}
              </button>
            )}

            <button
              className="btn btn-primary"
              onClick={() => handleRun()}
              disabled={running || !project.has_pipeline}
            >
              {running ? <Loader2 size={16} className="spin" /> : <Play size={16} />}
              {running ? 'Running...' : 'Run Pipeline'}
            </button>
            {running && (
              <button className="btn btn-danger" onClick={handleCancel} title="Stop pipeline">
                <Square size={16} />
                Stop
              </button>
            )}
            <button className="btn btn-danger-outline" onClick={handleDelete}>
              <Trash2 size={16} />
            </button>
          </div>
        </header>

        {error && <div className="alert alert-error">{error}</div>}

        {/* Preflight result */}
        {preflightResult && (
          <div
            className={`preflight-result ${
              preflightResult.passed ? 'preflight-pass' : 'preflight-fail'
            }`}
          >
            <div className="preflight-header">
              {preflightResult.passed ? (
                <>
                  <CheckCircle size={16} /> Preflight passed
                </>
              ) : (
                <>
                  <AlertTriangle size={16} /> Preflight failed
                </>
              )}
            </div>
            {preflightResult.errors.length > 0 && (
              <ul className="preflight-errors">
                {preflightResult.errors.map((err, i) => (
                  <li key={i}>{formatPreflightError(err)}</li>
                ))}
              </ul>
            )}
            {preflightResult.warnings.length > 0 && (
              <ul className="preflight-warnings">
                {preflightResult.warnings.map((w, i) => (
                  <li key={i}>{w}</li>
                ))}
              </ul>
            )}
          </div>
        )}

        {/* Pipeline validation warnings */}
        {validation && validation.warnings.length > 0 && (
          <div className="validation-warnings">
            <div className="validation-header">
              <AlertTriangle size={16} />
              Pipeline Issues Detected
            </div>
            <ul className="validation-list">
              {validation.warnings.map((w, i) => (
                <li key={i} className={`validation-item validation-${w.severity}`}>
                  <div className="validation-stage">Stage: {w.stage_name}</div>
                  <div className="validation-command">
                    <code>{w.command}</code>
                  </div>
                  <div className="validation-message">{w.message}</div>
                  {w.suggestion && <div className="validation-suggestion">{w.suggestion}</div>}
                </li>
              ))}
            </ul>
          </div>
        )}

        {/* File conflicts */}
        {validation && validation.file_conflicts && validation.file_conflicts.length > 0 && (
          <div className="file-conflicts">
            <div className="conflicts-header">
              <Copy size={16} />
              Duplicate Config Files Detected
            </div>
            <ul className="conflicts-list">
              {validation.file_conflicts.map((conflict, i) => (
                <li key={i} className="conflict-item">
                  <div className="conflict-category">{conflict.category}</div>
                  <div className="conflict-files">
                    {conflict.files.map((file, fi) => (
                      <span
                        key={fi}
                        className={`conflict-file ${conflict.active_file === file ? 'conflict-file-active' : ''}`}
                      >
                        {file}
                        {conflict.active_file === file && (
                          <span className="active-badge">active</span>
                        )}
                      </span>
                    ))}
                  </div>
                  <div className="conflict-message">{conflict.message}</div>
                </li>
              ))}
            </ul>
          </div>
        )}

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
            className={`project-tab ${activeTab === 'settings' ? 'project-tab-active' : ''}`}
            onClick={() => setActiveTab('settings')}
          >
            <Settings size={16} />
            Project Settings
          </button>
        </div>

        {/* Tab content */}
        <div className="project-tab-content">
          {/* Pipeline Tab */}
          {activeTab === 'pipeline' && (
            <>
              {/* Last failed run banner */}
              {!running &&
                runs.length > 0 &&
                runs[0].status === 'failed' &&
                (() => {
                  const lastFailed = runs[0];
                  const failedStage = lastFailed.stage_results.find((s) => s.status === 'failed');
                  return (
                    <div className="failed-run-banner">
                      <div className="failed-run-banner-info">
                        <XCircle size={14} />
                        <span>Failed{failedStage ? ` at ${failedStage.stage_name}` : ''}</span>
                        <Link to={`/run/${lastFailed.id}`} className="failed-run-banner-link">
                          View log
                        </Link>
                      </div>
                      <div className="failed-run-banner-actions">
                        {failedStage && (
                          <button
                            className="btn btn-sm"
                            onClick={() => handleBannerRetry(failedStage.stage_name)}
                            disabled={running}
                          >
                            <RotateCcw size={12} />
                            Retry from {failedStage.stage_name}
                          </button>
                        )}
                        <button
                          className="btn btn-sm btn-primary"
                          onClick={() => handleBannerRetry()}
                          disabled={running}
                        >
                          <RotateCcw size={12} />
                          Retry All
                        </button>
                      </div>
                    </div>
                  );
                })()}

              {pipeline ? (
                <section className="section">
                  <div className="section-header-row">
                    <h3 className="section-title">Pipeline: {pipeline.name}</h3>
                    <div className="section-header-actions">
                      <button
                        className="btn btn-secondary btn-sm"
                        onClick={handleRegenerate}
                        title="Re-detect files and regenerate pipeline from scratch"
                      >
                        <RefreshCw size={14} />
                        Regenerate
                      </button>
                      <button
                        className="btn btn-secondary btn-sm"
                        onClick={() => setEditingPipeline(!editingPipeline)}
                      >
                        <Settings size={14} />
                        {editingPipeline ? 'Done Editing' : 'Edit Pipeline'}
                      </button>
                    </div>
                  </div>

                  {editingPipeline ? (
                    <PipelineEditor
                      repoPath={project.project.path}
                      pipeline={pipeline}
                      onSaved={() => {
                        setEditingPipeline(false);
                        loadData();
                      }}
                    />
                  ) : (
                    <>
                      <div className="pipeline-stages">
                        {pipeline.stages.map((stage, idx) => {
                          const status = stageStatuses[stage.name];
                          const isRunning = status === 'running';
                          const isSuccess = status === 'success';
                          const isFailed = status === 'failed';
                          const isPending = status === 'pending';
                          const isSkipped = status === 'skipped';
                          const hasResult = isSuccess || isFailed;
                          const isSelected = selectedStageResult?.stage_name === stage.name;

                          // Click handler to show stage result
                          const handleStageClick = () => {
                            if (!hasResult || runs.length === 0) return;
                            const latestRun = runs[0];
                            const result = latestRun.stage_results?.find(
                              (r) => r.stage_name === stage.name
                            );
                            if (result) {
                              // Toggle selection - click again to close
                              if (isSelected) {
                                setSelectedStageResult(null);
                              } else {
                                setSelectedStageResult(result);
                              }
                            }
                          };

                          return (
                            <div key={idx} className="stage-card-wrapper">
                              <div
                                className={`stage-card ${
                                  isRunning ? 'stage-running' : ''
                                } ${isSuccess ? 'stage-success' : ''} ${
                                  isFailed ? 'stage-failed' : ''
                                } ${isPending ? 'stage-pending' : ''} ${
                                  isSkipped ? 'stage-skipped' : ''
                                } ${hasResult ? 'stage-clickable' : ''} ${
                                  isSelected ? 'stage-selected' : ''
                                }`}
                                onClick={hasResult ? handleStageClick : undefined}
                                role={hasResult ? 'button' : undefined}
                                tabIndex={hasResult ? 0 : undefined}
                                onKeyDown={
                                  hasResult
                                    ? (e) => {
                                        if (e.key === 'Enter' || e.key === ' ') {
                                          handleStageClick();
                                        }
                                      }
                                    : undefined
                                }
                              >
                                <div className="stage-header">
                                  <span
                                    className={`stage-number ${
                                      isRunning ? 'stage-number-running' : ''
                                    } ${isSuccess ? 'stage-number-success' : ''} ${
                                      isFailed ? 'stage-number-failed' : ''
                                    }`}
                                  >
                                    {isRunning ? (
                                      <Loader2 size={14} className="spin" />
                                    ) : isSuccess ? (
                                      <CheckCircle size={14} />
                                    ) : isFailed ? (
                                      <XCircle size={14} />
                                    ) : isSkipped ? (
                                      <Circle size={14} />
                                    ) : (
                                      idx + 1
                                    )}
                                  </span>
                                  <strong>{stage.name}</strong>
                                  <span className="badge badge-neutral">{stage.backend}</span>
                                  {stage.health_check && (
                                    <span className="badge badge-neutral" title="Has health check">
                                      HC
                                    </span>
                                  )}
                                  {stage.working_dir && (
                                    <span className="text-muted text-xs" title="Working directory">
                                      {stage.working_dir}
                                    </span>
                                  )}
                                  {hasResult && (
                                    <span className="stage-view-hint" title="Click to view output">
                                      <Eye size={14} />
                                    </span>
                                  )}
                                  <button
                                    className={`btn btn-icon btn-stage-run ${
                                      isRunning ? 'btn-stage-running' : ''
                                    }`}
                                    title={`Run "${stage.name}" only`}
                                    disabled={running}
                                    onClick={(e) => {
                                      e.stopPropagation();
                                      handleRun([stage.name]);
                                    }}
                                  >
                                    {isRunning ? (
                                      <Loader2 size={14} className="spin" />
                                    ) : (
                                      <Play size={14} />
                                    )}
                                  </button>
                                </div>
                                <div className="stage-commands">
                                  {stage.commands.map((cmd, ci) => {
                                    const cmdKey = `${stage.name}:${ci}`;
                                    const cmdStatus = cmdStatuses[cmdKey];
                                    return (
                                      <div
                                        key={ci}
                                        className={`command-line ${cmdStatus ? `cmd-${cmdStatus}` : ''}`}
                                      >
                                        {cmdStatus === 'done' && (
                                          <CheckCircle
                                            size={12}
                                            className="cmd-icon cmd-icon-done"
                                          />
                                        )}
                                        {cmdStatus === 'running' && (
                                          <Loader2
                                            size={12}
                                            className="cmd-icon cmd-icon-running spin"
                                          />
                                        )}
                                        {cmdStatus === 'failed' && (
                                          <XCircle size={12} className="cmd-icon cmd-icon-failed" />
                                        )}
                                        {cmdStatus === 'pending' && running && (
                                          <Circle size={12} className="cmd-icon cmd-icon-pending" />
                                        )}
                                        <code>{cmd}</code>
                                      </div>
                                    );
                                  })}
                                </div>
                                {/* Live output preview for running stage */}
                                {isRunning && liveOutput[stage.name]?.length > 0 && (
                                  <div className="live-output-preview">
                                    <div className="live-output-header">
                                      <span className="live-output-dot" />
                                      <span>Live Output</span>
                                    </div>
                                    <pre className="live-output-lines">
                                      {liveOutput[stage.name].map((line, i) => (
                                        <span key={i} className="live-output-line">
                                          {line}
                                          {'\n'}
                                        </span>
                                      ))}
                                    </pre>
                                  </div>
                                )}
                              </div>
                              {/* Inline stage result viewer — appears right below the clicked stage */}
                              {isSelected && selectedStageResult && (
                                <div className="stage-result-viewer stage-result-inline">
                                  <div className="stage-result-header">
                                    <span className="stage-result-title">
                                      Output: {selectedStageResult.stage_name}
                                    </span>
                                    <button
                                      className="btn btn-icon btn-sm"
                                      onClick={(e) => {
                                        e.stopPropagation();
                                        setSelectedStageResult(null);
                                      }}
                                      title="Close"
                                    >
                                      <XCircle size={14} />
                                    </button>
                                  </div>
                                  <LogViewer stage={selectedStageResult} />
                                </div>
                              )}
                            </div>
                          );
                        })}
                      </div>

                      {/* Stage result viewer removed from here — now rendered inline below each stage card */}
                    </>
                  )}
                </section>
              ) : (
                <section className="section">
                  <div className="empty-state-small">
                    <p>No pipeline configured yet.</p>
                    <button className="btn btn-primary" onClick={handleRegenerate}>
                      <RefreshCw size={14} />
                      Auto-detect & Generate Pipeline
                    </button>
                  </div>
                </section>
              )}
            </>
          )}

          {/* History Tab */}
          {activeTab === 'history' && (
            <section className="section">
              <div className="section-header-row">
                <h3 className="section-title">
                  <History size={16} />
                  Run History
                </h3>
                <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--space-sm)' }}>
                  {/* Environment filter for history */}
                  {envsConfig?.environments?.length > 0 && (
                    <select
                      className="input input-sm"
                      value={historyEnvFilter}
                      onChange={(e) => setHistoryEnvFilter(e.target.value)}
                    >
                      <option value="">All environments</option>
                      {envsConfig.environments.map((env) => (
                        <option key={env.name} value={env.name}>
                          {env.name}
                        </option>
                      ))}
                    </select>
                  )}
                  {runs.length > 0 && (
                    <button
                      className="btn btn-sm btn-ghost"
                      onClick={handleClearHistory}
                      title="Clear all run history"
                    >
                      <Trash2 size={14} />
                      Clear
                    </button>
                  )}
                </div>
              </div>

              {/* Last known good deployment */}
              {lastGoodRun && (
                <div className="last-good-run">
                  <div className="last-good-header">
                    <Trophy size={14} />
                    <span>Last Successful Deployment</span>
                  </div>
                  <Link
                    to={`/run/${lastGoodRun.id}`}
                    state={{ projectId, tab: 'history' }}
                    className="last-good-link"
                  >
                    {(() => {
                      const scope = runScopeLabel(lastGoodRun);
                      return (
                        <>
                          <span className="run-pipeline-name">{scope.label}</span>
                          {scope.isPartial && (
                            <span className="badge badge-neutral" title="Single stage run">
                              Step
                            </span>
                          )}
                        </>
                      );
                    })()}
                    {lastGoodRun.environment && (
                      <span className="badge badge-neutral">{lastGoodRun.environment}</span>
                    )}
                    {lastGoodRun.branch && (
                      <span className="run-branch">
                        <GitBranch size={12} /> {lastGoodRun.branch}
                      </span>
                    )}
                    <span className="run-date">{formatDate(lastGoodRun.started_at)}</span>
                    <span className="badge badge-success">Success</span>
                  </Link>
                </div>
              )}

              {filteredRuns.length === 0 ? (
                <div className="empty-state-small">
                  <p>No runs yet. Run the pipeline to see history.</p>
                </div>
              ) : (
                <div className="run-list">
                  {filteredRuns.map((run) => {
                    const isLastGood = lastGoodRun?.id === run.id;
                    const scope = runScopeLabel(run);
                    return (
                      <Link
                        key={run.id}
                        to={`/run/${run.id}`}
                        state={{ projectId, tab: 'history' }}
                        className={`run-row ${isLastGood ? 'run-row-last-good' : ''}`}
                      >
                        <span className={`status-dot status-${statusClass(run.status)}`} />
                        <span className="run-pipeline-name">{scope.label}</span>
                        {scope.isPartial && (
                          <span className="badge badge-neutral run-scope-badge" title="Partial run">
                            Step
                          </span>
                        )}
                        {!scope.isPartial && run.stage_results.length > 0 && (
                          <span
                            className="badge badge-neutral run-scope-badge"
                            title="Full pipeline run"
                          >
                            Full
                          </span>
                        )}
                        {run.environment && (
                          <span className="badge badge-neutral">{run.environment}</span>
                        )}
                        {run.run_kind === 'retry' && (
                          <span
                            className="badge badge-info"
                            title={`Retry #${run.retry_number ?? 1}`}
                          >
                            <RotateCcw size={10} /> Retry
                          </span>
                        )}
                        {run.run_kind === 'rollback' && (
                          <span className="badge badge-warning" title="Rollback">
                            <Undo2 size={10} /> Rollback
                          </span>
                        )}
                        {isLastGood && (
                          <span className="badge badge-success" title="Last known good deployment">
                            <Trophy size={10} />
                          </span>
                        )}
                        <span className="run-meta">
                          {run.branch && (
                            <span className="run-branch">
                              <GitBranch size={12} /> {run.branch}
                            </span>
                          )}
                          <span className="run-duration">
                            <Clock size={12} /> {formatDuration(run.duration_ms)}
                          </span>
                          <span className="run-date">{formatDate(run.started_at)}</span>
                        </span>
                        <span className={`badge badge-${statusClass(run.status)}`}>
                          {capitalize(run.status)}
                        </span>
                      </Link>
                    );
                  })}
                </div>
              )}
            </section>
          )}

          {/* Settings Tab */}
          {activeTab === 'settings' && (
            <>
              {/* Environments section */}
              <section className="section">
                <button
                  className="section-toggle"
                  onClick={() => setShowEnvSection(!showEnvSection)}
                >
                  {showEnvSection ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
                  <Server size={16} />
                  Environments
                  <span className="badge badge-neutral">{envsConfig?.environments?.length}</span>
                </button>
                {showEnvSection && (
                  <EnvironmentEditor
                    repoPath={project.project.path}
                    config={envsConfig}
                    onSaved={loadData}
                  />
                )}
              </section>

              {/* Secrets section */}
              <section className="section">
                <button
                  className="section-toggle"
                  onClick={() => setShowSecretsSection(!showSecretsSection)}
                >
                  {showSecretsSection ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
                  <Key size={16} />
                  Secrets
                  <span className="badge badge-neutral">{secretsConfig?.secrets?.length}</span>
                </button>
                {showSecretsSection && (
                  <SecretsManager
                    repoPath={project.project.path}
                    config={secretsConfig}
                    environments={envsConfig.environments}
                    onSaved={loadData}
                  />
                )}
              </section>
            </>
          )}
        </div>
      </div>

      {/* Right sidebar */}
      <aside className="project-sidebar">
        {/* Project Info */}
        <div className="project-sidebar-card">
          <h4 className="project-sidebar-title">
            <Cog size={14} />
            Project Info
          </h4>
          <div className="project-stats">
            <div className="project-stat-row">
              <span className="project-stat-label">Pipeline</span>
              <span className={`badge ${project.has_pipeline ? 'badge-success' : 'badge-pending'}`}>
                {project.has_pipeline ? 'Configured' : 'Not set'}
              </span>
            </div>
            <div className="project-stat-row">
              <span className="project-stat-label">Stages</span>
              <span className="project-stat-value">{pipeline?.stages.length ?? 0}</span>
            </div>
            <div className="project-stat-row">
              <span className="project-stat-label">Environments</span>
              <span className="project-stat-value">{envsConfig?.environments?.length ?? 0}</span>
            </div>
            <div className="project-stat-row">
              <span className="project-stat-label">Secrets</span>
              <span className="project-stat-value">{secretsConfig?.secrets?.length ?? 0}</span>
            </div>
            <div className="project-stat-row">
              <span className="project-stat-label">Total Runs</span>
              <span className="project-stat-value">{runs.length}</span>
            </div>
            {project.project.last_run_status && (
              <div className="project-stat-row">
                <span className="project-stat-label">Last Run</span>
                <span className={`badge badge-${statusClass(project.project.last_run_status)}`}>
                  {capitalize(project.project.last_run_status)}
                </span>
              </div>
            )}
          </div>
        </div>

        {/* CI/CD Recommendations */}
        <RecommendationsPanel recommendations={recommendations} loading={loadingRecommendations} />

        {/* Detected CI/CD Files */}
        <div className="project-sidebar-card">
          <h4 className="project-sidebar-title">
            <FileCode size={14} />
            Detected Files
            <span className="badge badge-neutral">{detectedFiles.length}</span>
          </h4>
          {detectedFiles.length === 0 ? (
            <p className="text-muted text-xs">No CI/CD files detected.</p>
          ) : (
            <ul className="detected-files-list">
              {detectedFiles.map((script, i) => (
                <li key={i} className="detected-file-item">
                  <span className="detected-file-icon">
                    <FileTypeIcon scriptType={script.script_type} />
                  </span>
                  <span className="detected-file-name">{script.file_name}</span>
                  <span className="detected-file-type">{formatScriptType(script.script_type)}</span>
                </li>
              ))}
            </ul>
          )}
        </div>
      </aside>
    </div>
  );
}

function formatScriptType(scriptType: string): string {
  switch (scriptType) {
    // Node / JS
    case 'PackageJson':
      return 'npm';
    case 'Turborepo':
      return 'turbo';
    case 'Nx':
      return 'nx';
    case 'Deno':
      return 'deno';
    case 'Grunt':
      return 'grunt';
    case 'Gulp':
      return 'gulp';
    // Rust
    case 'CargoToml':
      return 'cargo';
    // Go
    case 'GoMod':
      return 'go';
    // Python
    case 'PythonProject':
      return 'python';
    case 'PythonRequirements':
      return 'pip';
    case 'Tox':
      return 'tox';
    // Ruby
    case 'Gemfile':
      return 'ruby';
    case 'Rakefile':
      return 'rake';
    // Java
    case 'Maven':
      return 'maven';
    case 'Gradle':
      return 'gradle';
    // .NET
    case 'DotNet':
      return '.net';
    // PHP
    case 'Composer':
      return 'php';
    // C / C++
    case 'CMake':
      return 'cmake';
    case 'Meson':
      return 'meson';
    // Make / tasks
    case 'Makefile':
      return 'make';
    case 'Justfile':
      return 'just';
    case 'Taskfile':
      return 'task';
    // Containers
    case 'Dockerfile':
      return 'docker';
    case 'DockerCompose':
      return 'compose';
    case 'Skaffold':
      return 'skaffold';
    case 'Vagrantfile':
      return 'vagrant';
    // Shell / env
    case 'ShellScript':
      return 'shell';
    case 'EnvFile':
      return 'env';
    case 'Procfile':
      return 'proc';
    // CI platforms
    case 'GithubActions':
      return 'github';
    case 'GitlabCi':
      return 'gitlab';
    case 'Jenkinsfile':
      return 'jenkins';
    case 'TravisCi':
      return 'travis';
    case 'DroneCi':
      return 'drone';
    case 'CircleCi':
      return 'circleci';
    case 'AzurePipelines':
      return 'azure';
    case 'BitbucketPipelines':
      return 'bitbucket';
    // Deploy / infra
    case 'Netlify':
      return 'netlify';
    case 'Vercel':
      return 'vercel';
    // Quality
    case 'PreCommit':
      return 'hooks';
    default:
      return scriptType.toLowerCase();
  }
}

function formatPreflightError(err: { type: string; detail: unknown }): string {
  switch (err.type) {
    case 'MissingSecret': {
      const d = err.detail as { name: string; environment: string };
      return `Missing secret "${d.name}" for environment "${d.environment}"`;
    }
    case 'MissingSshHost': {
      const d = err.detail as { stage: string };
      return `Stage "${d.stage}" uses SSH but no host is configured`;
    }
    case 'MissingEnvironment': {
      const d = err.detail as { name: string };
      return `Environment "${d.name}" not found`;
    }
    case 'SshConnectivityFailed': {
      const d = err.detail as { host: string; error: string };
      return `SSH to ${d.host} failed: ${d.error}`;
    }
    case 'SshNotAvailable':
      return 'SSH client not found on PATH';
    default:
      return JSON.stringify(err);
  }
}

export default ProjectDetail;
