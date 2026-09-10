import { useEffect, useRef, useState } from 'react';
import { useParams, Link, useLocation, useNavigate } from 'react-router-dom';
import {
  ArrowLeft,
  Clock,
  GitBranch,
  CircleCheck,
  CircleX,
  Circle,
  SkipForward,
  RotateCcw,
  RefreshCw,
  Loader2,
  Server,
  Undo2,
  Square,
  FolderOpen,
  GitCommit,
  TimerOff,
  RotateCw,
  TriangleAlert,
  Archive,
} from 'lucide-react';
import { getAppDataDir, getRun, retryRun, rollbackToRun, cancelPipeline } from '../services/api';
import { openPath } from '../services/openExternal';
import {
  formatDate,
  formatDuration,
  statusClass,
  capitalize,
  isFailureStatus,
  isAutoRollback,
  isUnattendedKind,
  runKindLabel,
} from '../utils/format';
import type { PipelineRun, StageResult } from '../types';
import TriggerBadge from './TriggerBadge';
import RollbackNotice from './run-detail/RollbackNotice';
import LogViewer from './LogViewer';
import AgentPanel from './AgentPanel';
import { listen } from '@tauri-apps/api/event';

// Strip ANSI escape sequences from live output
// eslint-disable-next-line no-control-regex
const ANSI_RE = /\x1b\[[0-9;]*[A-Za-z]|\x1b\].*?\x07/g;
function stripAnsi(text: string): string {
  return text.replace(ANSI_RE, '');
}

function RunDetail() {
  const { runId } = useParams<{ runId: string }>();
  const location = useLocation();
  const navigate = useNavigate();
  const { projectId, tab } = (location.state as { projectId?: string; tab?: string }) ?? {};
  const [run, setRun] = useState<PipelineRun | null>(null);
  const [selectedStage, setSelectedStage] = useState<StageResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Run summaries outlive their logs: a link from Insights can point at a run
  // whose record has been pruned. That is expected history, not a crash.
  const [notFound, setNotFound] = useState(false);
  const [retrying, setRetrying] = useState(false);
  const [rollingBack, setRollingBack] = useState(false);

  // Live progress state for retry/rollback
  const [liveStage, setLiveStage] = useState<string | null>(null);
  const [liveLines, setLiveLines] = useState<string[]>([]);
  const liveLinesRef = useRef<string[]>([]);
  const liveLogEndRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!runId) return;
    let ignore = false;
    getRun(runId)
      .then((data) => {
        if (ignore) return;
        setNotFound(data == null);
        setRun(data);
        if (data && data.stage_results.length > 0) {
          setSelectedStage(data.stage_results[0]);
        }
      })
      .catch((err) => {
        if (!ignore) setError(String(err));
      });
    return () => {
      ignore = true;
    };
  }, [runId]);

  // Auto-scroll live log to bottom
  useEffect(() => {
    liveLogEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [liveLines]);

  function appendLiveLine(text: string) {
    const clean = stripAnsi(text);
    liveLinesRef.current = [...liveLinesRef.current, clean];
    setLiveLines([...liveLinesRef.current]);
  }

  function setupLogListener() {
    const repoPath = run?.repo_path;
    return listen<{
      run_id: string;
      repo_path: string;
      stage: string;
      type: string;
      message: string;
    }>('pipeline:log', (event) => {
      // Only react to this project's run — other projects may run concurrently.
      if (repoPath && event.payload.repo_path !== repoPath) return;
      const { stage, type: logType, message } = event.payload;

      if (logType === 'info' && message.startsWith('--- Starting stage:')) {
        setLiveStage(stage);
        appendLiveLine(`\n--- Stage: ${stage} ---`);
      } else if (logType === 'cmd') {
        appendLiveLine(`$ ${message}`);
      } else if (logType === 'stdout' || logType === 'stderr') {
        appendLiveLine(message);
      } else if (logType === 'error') {
        appendLiveLine(`ERROR: ${message}`);
      }
    });
  }

  async function handleRetry(fromStage?: string) {
    if (!run) return;
    try {
      setRetrying(true);
      setError(null);
      setLiveStage(null);
      setLiveLines([]);
      liveLinesRef.current = [];

      const unlisten = await setupLogListener();
      const newRun = await retryRun(run.id, fromStage);
      unlisten();
      navigate(`/run/${newRun.id}`, { state: { projectId, tab } });
    } catch (err) {
      setError(String(err));
      setLiveStage(null);
      setLiveLines([]);
      liveLinesRef.current = [];
    } finally {
      setRetrying(false);
    }
  }

  async function handleRollback() {
    if (!run || run.status !== 'success') return;
    try {
      setRollingBack(true);
      setError(null);
      setLiveStage(null);
      setLiveLines([]);
      liveLinesRef.current = [];

      const unlisten = await setupLogListener();
      const newRun = await rollbackToRun(run.id);
      unlisten();
      navigate(`/run/${newRun.id}`, { state: { projectId, tab } });
    } catch (err) {
      setError(String(err));
      setLiveStage(null);
      setLiveLines([]);
      liveLinesRef.current = [];
    } finally {
      setRollingBack(false);
    }
  }

  async function handleCancel() {
    if (!run) return;
    try {
      await cancelPipeline(run.repo_path);
    } catch (err) {
      setError(String(err));
    }
  }

  // Show running state for both locally initiated retries/rollbacks AND runs that are already in progress
  const isRunning = retrying || rollingBack || run?.status === 'running';

  // Find the first failed stage for smart retry.
  const firstFailedStage = run?.stage_results.find((s) => isFailureStatus(s.status));

  function stageIcon(status: string) {
    switch (status) {
      case 'success':
        return <CircleCheck size={16} className="status-icon status-success" />;
      case 'failed':
        return <CircleX size={16} className="status-icon status-failed" />;
      case 'timedout':
        return <TimerOff size={16} className="status-icon status-failed" />;
      case 'running':
        return <Circle size={16} className="status-icon status-running" />;
      case 'skipped':
        return <SkipForward size={16} className="status-icon status-skipped" />;
      default:
        return <Circle size={16} className="status-icon status-pending" />;
    }
  }

  if (!run) {
    if (notFound || error) {
      return (
        <div className="page">
          <Link to={projectId ? `/project/${projectId}` : '/'} className="back-link">
            <ArrowLeft size={16} /> Back
          </Link>
          <div className="empty-state">
            <Archive size={24} strokeWidth={1} />
            <h3>This run is no longer available</h3>
            <p className="text-muted">
              Its logs were pruned by retention. The run still counts toward metrics on the Insights
              page, but there is nothing left to open.
            </p>
          </div>
        </div>
      );
    }
    return (
      <div className="page">
        <div className="loading">Loading run...</div>
      </div>
    );
  }

  const isFailed = run.status === 'failed';
  const isSuccess = run.status === 'success';
  const isRetry = run.run_kind === 'retry';
  const isRollbackRun = run.run_kind === 'rollback';
  const isAuto = isAutoRollback(run);
  // A schedule or watch failure happened with nobody watching — say so loudly.
  const unattendedFailure = isUnattendedKind(run.run_kind) && isFailureStatus(run.status);
  const linkState = { projectId, tab };

  return (
    <div className="page">
      <Link
        to={projectId ? `/project/${projectId}` : '/'}
        state={projectId ? { tab } : undefined}
        className="back-link"
      >
        <ArrowLeft size={16} /> Back
      </Link>

      {error && <div className="alert alert-error">{error}</div>}

      <header className="page-header">
        <div>
          <h2 className="page-title">Run: {run.pipeline_name}</h2>
          <div className="run-detail-meta">
            <span className={`badge badge-${statusClass(run.status)}`}>
              {capitalize(run.status)}
            </span>
            <TriggerBadge runKind={run.run_kind} size={12} />
            {run.trigger_id && (
              <span className="meta-item text-muted" title="The trigger that started this run">
                trigger: {run.trigger_id}
              </span>
            )}
            {isRetry && (
              <span
                className="badge badge-info"
                title={`Retry #${run.retry_number ?? 1} of run ${run.parent_run_id}`}
              >
                <RotateCcw size={12} /> Retry #{run.retry_number ?? 1}
              </span>
            )}
            {isRollbackRun && (
              <span
                className="badge badge-warning"
                title={
                  isAuto
                    ? `Automatic rollback of run ${run.auto_rollback_of}`
                    : `Rollback to run ${run.rollback_target_id}`
                }
              >
                <Undo2 size={12} /> {isAuto ? 'Auto-rollback' : 'Rollback'}
              </span>
            )}
            {run.retry_from_stage && (
              <span className="meta-item text-muted">from stage: {run.retry_from_stage}</span>
            )}
            {run.environment && (
              <span className="meta-item">
                <Server size={14} /> {run.environment}
              </span>
            )}
            {run.branch && (
              <span className="meta-item">
                <GitBranch size={14} /> {run.branch}
              </span>
            )}
            {run.commit && (
              <span className="meta-item" title={run.commit}>
                <GitCommit size={14} /> {run.commit.slice(0, 8)}
              </span>
            )}
            <span className="meta-item">
              <Clock size={14} /> {formatDuration(run.duration_ms)}
            </span>
            <span className="meta-item">{formatDate(run.started_at)}</span>
          </div>
          {unattendedFailure && (
            <div className="unattended-failure-notice" role="alert">
              <TriangleAlert size={14} />
              <span>
                This {runKindLabel(run.run_kind).toLowerCase()} run failed with nobody watching
                {run.trigger_id ? ` — trigger "${run.trigger_id}"` : ''}. Nothing prompted it, so
                nothing surfaced the failure at the time.
              </span>
            </div>
          )}
          {/* Parent run link */}
          {isRetry && run.parent_run_id && (
            <div className="run-parent-link">
              <Link
                to={`/run/${run.parent_run_id}`}
                state={{ projectId, tab }}
                className="text-link"
              >
                View original run
              </Link>
            </div>
          )}
          {isRollbackRun && run.rollback_target_id && (
            <div className="run-parent-link">
              <Link
                to={`/run/${run.rollback_target_id}`}
                state={{ projectId, tab }}
                className="text-link"
              >
                View rollback target run
              </Link>
            </div>
          )}
          {run.auto_rollback_of && (
            <div className="run-parent-link">
              Automatic rollback of a failed deploy —{' '}
              <Link to={`/run/${run.auto_rollback_of}`} state={linkState} className="text-link">
                view the run that triggered it
              </Link>
            </div>
          )}
        </div>
        <div className="header-actions">
          {/* Retry buttons */}
          {isFailed && !isRunning && (
            <>
              {firstFailedStage && (
                <button
                  className="btn btn-primary"
                  onClick={() => handleRetry(firstFailedStage.stage_name)}
                  disabled={retrying}
                  title={`Retry from failed stage: ${firstFailedStage.stage_name}`}
                >
                  <RefreshCw size={16} />
                  Retry from &quot;{firstFailedStage.stage_name}&quot;
                </button>
              )}
              <button
                className="btn btn-secondary"
                onClick={() => handleRetry()}
                disabled={retrying}
                title="Retry the entire pipeline from the beginning"
              >
                <RotateCcw size={16} />
                Retry All
              </button>
            </>
          )}
          {/* Cancel button during retry/rollback */}
          {isRunning && (
            <button className="btn btn-danger" onClick={handleCancel} title="Stop pipeline">
              <Square size={16} />
              Stop
            </button>
          )}
          {/* Rollback button — only for successful runs */}
          {isSuccess && run.environment && !isRunning && (
            <button
              className="btn btn-warning"
              onClick={handleRollback}
              disabled={rollingBack}
              title="Re-run this pipeline to roll back to this deployment"
            >
              <Undo2 size={16} />
              Rollback to This
            </button>
          )}
          <button
            className="btn btn-ghost btn-sm"
            onClick={async () => {
              const dir = await getAppDataDir();
              openPath(`${dir}/runs/${run.id}`);
            }}
            title="Reveal run folder in Finder"
          >
            <FolderOpen size={14} />
            Reveal
          </button>
        </div>
      </header>

      {/* Health-check failure + what auto-rollback did about it */}
      {!isRunning && <RollbackNotice run={run} linkState={linkState} />}

      {/* Live progress overlay when retrying/rolling back */}
      {isRunning && (
        <div className="live-run-progress">
          <div className="live-run-header">
            <Loader2 size={16} className="spin" />
            <span>
              {retrying ? 'Retrying' : 'Rolling back'}
              {liveStage ? ` — ${liveStage}` : '...'}
            </span>
          </div>
          <pre className="live-run-output">
            {liveLines.length === 0 && (
              <span className="log-line log-line-info">Starting pipeline...</span>
            )}
            {liveLines.map((line, i) => (
              <span
                key={i}
                className={`log-line ${line.startsWith('$') ? 'log-line-info' : line.startsWith('ERROR') ? 'log-line-stderr' : 'log-line-stdout'}`}
              >
                {line}
                {'\n'}
              </span>
            ))}
            <div ref={liveLogEndRef} />
          </pre>
        </div>
      )}

      {/* Normal run detail view (hidden during live progress) */}
      {!isRunning && (
        <div className="run-detail-layout">
          {/* Stage list sidebar */}
          <div className="stage-sidebar">
            <h4 className="sidebar-section-title">Stages</h4>
            {run.stage_results.map((stage) => (
              <button
                key={stage.stage_name}
                className={`stage-sidebar-item ${selectedStage?.stage_name === stage.stage_name ? 'active' : ''}`}
                onClick={() => setSelectedStage(stage)}
              >
                {stageIcon(stage.status)}
                <span className="stage-sidebar-name">{stage.stage_name}</span>
                {stage.attempts !== undefined && stage.attempts > 1 && (
                  <span
                    className="stage-sidebar-attempts"
                    title={`Retried — ${stage.attempts} attempts`}
                  >
                    <RotateCw size={11} /> {stage.attempts} attempts
                  </span>
                )}
                <span className="stage-sidebar-duration">{formatDuration(stage.duration_ms)}</span>
                {/* Retry from this stage button */}
                {isFailed && isFailureStatus(stage.status) && (
                  <button
                    className="btn btn-icon btn-xs"
                    onClick={(e) => {
                      e.stopPropagation();
                      handleRetry(stage.stage_name);
                    }}
                    disabled={retrying}
                    title={`Retry from "${stage.stage_name}"`}
                  >
                    <RefreshCw size={12} />
                  </button>
                )}
              </button>
            ))}
          </div>

          {/* Log panel */}
          <div className="log-panel">
            {selectedStage ? (
              <>
                {selectedStage.status === 'skipped' && selectedStage.skip_reason && (
                  <div className="stage-skip-reason">
                    <SkipForward size={14} /> Skipped: {selectedStage.skip_reason}
                  </div>
                )}
                <LogViewer stage={selectedStage} />
              </>
            ) : (
              <div className="empty-state-small">
                <p>Select a stage to view logs.</p>
              </div>
            )}
          </div>
        </div>
      )}

      {run && <AgentPanel runId={run.id} projectId={projectId} isFailed={isFailed} />}
    </div>
  );
}

export default RunDetail;
