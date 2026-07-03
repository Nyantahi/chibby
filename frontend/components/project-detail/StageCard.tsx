import { Play, CheckCircle, Loader2, XCircle, Circle, Eye } from 'lucide-react';
import type { Stage, StageResult, PipelineRun } from '../../types';
import type { StageStatus, CmdStatus } from '../../services/runStore';
import LogViewer from '../LogViewer';

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
  const isFailed = status === 'failed';
  const isPending = status === 'pending';
  const isSkipped = status === 'skipped';
  const hasResult = isSuccess || isFailed;
  const isSelected = selectedStageResult?.stage_name === stage.name;

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
