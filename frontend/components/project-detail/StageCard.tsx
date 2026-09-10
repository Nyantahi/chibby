import {
  Play,
  CheckCircle,
  Loader2,
  XCircle,
  Circle,
  Eye,
  TimerOff,
  Timer,
  RotateCw,
  GitBranch,
  Undo2,
} from 'lucide-react';
import type { RollbackMode, Stage, StageResult, PipelineRun } from '../../types';
import type { StageStatus, CmdStatus } from '../../services/runStore';
import { isFailureStatus } from '../../utils/format';
import LogViewer from '../LogViewer';

/** Human-readable summary of a stage's `when` conditions, or null if unset. */
function whenSummary(stage: Stage): string | null {
  const w = stage.when;
  if (!w) return null;
  const parts = [
    ...w.branch.map((b) => `branch: ${b}`),
    ...w.branch_not.map((b) => `branch not: ${b}`),
    ...w.environment.map((e) => `env: ${e}`),
    ...w.environment_not.map((e) => `env not: ${e}`),
  ];
  return parts.length > 0 ? parts.join(', ') : null;
}

const ROLLBACK_MODE_LABEL: Record<RollbackMode, string> = {
  off: 'off',
  last_good: 'last good',
  commands: 'commands',
};

/** The stage's active rollback policy, or null when it is unset or `off`. */
function rollbackSummary(stage: Stage): string | null {
  const mode = stage.on_health_failure?.mode;
  if (!mode || mode === 'off') return null;
  return ROLLBACK_MODE_LABEL[mode];
}

interface StageCardProps {
  stage: Stage;
  index: number;
  status: StageStatus | undefined;
  cmdStatuses: Record<string, CmdStatus>;
  liveOutput: Record<string, string[]>;
  running: boolean;
  runs: PipelineRun[];
  selectedStageResult: StageResult | null;
  onSelectStageResult: (result: StageResult | null) => void;
  onRunStage: (stages: string[]) => void;
}

function StageCard({
  stage,
  index,
  status,
  cmdStatuses,
  liveOutput,
  running,
  runs,
  selectedStageResult,
  onSelectStageResult,
  onRunStage,
}: StageCardProps) {
  const idx = index;
  const isRunning = status === 'running';
  const isSuccess = status === 'success';
  const isFailed = isFailureStatus(status);
  const isTimedOut = status === 'timedout';
  const isPending = status === 'pending';
  const isSkipped = status === 'skipped';
  const hasResult = isSuccess || isFailed;
  const isSelected = selectedStageResult?.stage_name === stage.name;
  const envCount = stage.env ? Object.keys(stage.env).length : 0;
  const when = whenSummary(stage);
  const rollback = rollbackSummary(stage);

  // Click handler to show stage result
  const handleStageClick = () => {
    if (!hasResult || runs.length === 0) return;
    const latestRun = runs[0];
    const result = latestRun.stage_results?.find((r) => r.stage_name === stage.name);
    if (result) {
      // Toggle selection - click again to close
      if (isSelected) {
        onSelectStageResult(null);
      } else {
        onSelectStageResult(result);
      }
    }
  };

  return (
    <div className="stage-card-wrapper">
      <div
        className={`stage-card ${
          isRunning ? 'stage-running' : ''
        } ${isSuccess ? 'stage-success' : ''} ${
          isFailed ? 'stage-failed' : ''
        } ${isPending ? 'stage-pending' : ''} ${
          isSkipped ? 'stage-skipped' : ''
        } ${hasResult ? 'stage-clickable' : ''} ${isSelected ? 'stage-selected' : ''}`}
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
            } ${isSuccess ? 'stage-number-success' : ''} ${isFailed ? 'stage-number-failed' : ''}`}
          >
            {isRunning ? (
              <Loader2 size={14} className="spin" />
            ) : isSuccess ? (
              <CheckCircle size={14} />
            ) : isTimedOut ? (
              <TimerOff size={14} />
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
          {stage.timeout_secs !== undefined && (
            <span className="badge badge-neutral" title={`Timeout: ${stage.timeout_secs}s`}>
              <Timer size={11} /> {stage.timeout_secs}s
            </span>
          )}
          {stage.retry && (
            <span
              className="badge badge-neutral"
              title={`Retry: up to ${stage.retry.attempts} attempts, ${stage.retry.delay_secs}s ${stage.retry.backoff} backoff`}
            >
              <RotateCw size={11} /> {stage.retry.attempts}
            </span>
          )}
          {when && (
            <span className="badge badge-neutral" title={`Runs only when ${when}`}>
              <GitBranch size={11} /> when
            </span>
          )}
          {rollback && (
            <span
              className="badge badge-neutral"
              title={`On health check failure: roll back — ${rollback}`}
            >
              <Undo2 size={11} /> rollback
            </span>
          )}
          {envCount > 0 && (
            <span className="badge badge-neutral" title="Stage-scoped environment variables">
              env ×{envCount}
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
            className={`btn btn-icon btn-stage-run ${isRunning ? 'btn-stage-running' : ''}`}
            title={`Run "${stage.name}" only`}
            disabled={running}
            onClick={(e) => {
              e.stopPropagation();
              onRunStage([stage.name]);
            }}
          >
            {isRunning ? <Loader2 size={14} className="spin" /> : <Play size={14} />}
          </button>
        </div>
        <div className="stage-commands">
          {stage.commands.map((cmd, ci) => {
            const cmdKey = `${stage.name}:${ci}`;
            const cmdStatus = cmdStatuses[cmdKey];
            return (
              <div key={ci} className={`command-line ${cmdStatus ? `cmd-${cmdStatus}` : ''}`}>
                {cmdStatus === 'done' && (
                  <CheckCircle size={12} className="cmd-icon cmd-icon-done" />
                )}
                {cmdStatus === 'running' && (
                  <Loader2 size={12} className="cmd-icon cmd-icon-running spin" />
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
        {/* Live output preview — visible while running AND after completion */}
        {(isRunning || isFailed || isSuccess) && liveOutput[stage.name]?.length > 0 && (
          <div className="live-output-preview">
            <div className="live-output-header">
              {isRunning && <span className="live-output-dot" />}
              <span>{isRunning ? 'Live Output' : 'Output'}</span>
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
            <span className="stage-result-title">Output: {selectedStageResult.stage_name}</span>
            <button
              className="btn btn-icon btn-sm"
              onClick={(e) => {
                e.stopPropagation();
                onSelectStageResult(null);
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
}

export default StageCard;
