import { Link } from 'react-router-dom';
import { XCircle, RotateCcw, RefreshCw, Settings } from 'lucide-react';
import type { Pipeline, PipelineRun, StageResult } from '../../types';
import type { StageStatus, CmdStatus } from '../../services/runStore';
import PipelineEditor from '../PipelineEditor';
import StageCard from './StageCard';

interface PipelineTabProps {
  repoPath: string;
  pipeline: Pipeline | null;
  runs: PipelineRun[];
  running: boolean;
  stageStatuses: Record<string, StageStatus>;
  cmdStatuses: Record<string, CmdStatus>;
  liveOutput: Record<string, string[]>;
  editingPipeline: boolean;
  selectedStageResult: StageResult | null;
  onSelectStageResult: (result: StageResult | null) => void;
  onToggleEditing: () => void;
  onRun: (stages?: string[]) => void;
  onBannerRetry: (fromStage?: string) => void;
  onRegenerate: () => void;
  onPipelineSaved: () => void;
}

function PipelineTab({
  repoPath,
  pipeline,
  runs,
  running,
  stageStatuses,
  cmdStatuses,
  liveOutput,
  editingPipeline,
  selectedStageResult,
  onSelectStageResult,
  onToggleEditing,
  onRun,
  onBannerRetry,
  onRegenerate,
  onPipelineSaved,
}: PipelineTabProps) {
  return (
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
                    onClick={() => onBannerRetry(failedStage.stage_name)}
                    disabled={running}
                  >
                    <RotateCcw size={12} />
                    Retry from {failedStage.stage_name}
                  </button>
                )}
                <button
                  className="btn btn-sm btn-primary"
                  onClick={() => onBannerRetry()}
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
                onClick={onRegenerate}
                title="Re-detect files and regenerate pipeline from scratch"
              >
                <RefreshCw size={14} />
                Regenerate
              </button>
              <button className="btn btn-secondary btn-sm" onClick={onToggleEditing}>
                <Settings size={14} />
                {editingPipeline ? 'Done Editing' : 'Edit Pipeline'}
              </button>
            </div>
          </div>

          {editingPipeline ? (
            <PipelineEditor repoPath={repoPath} pipeline={pipeline} onSaved={onPipelineSaved} />
          ) : (
            <>
              <div className="pipeline-stages">
                {pipeline.stages.map((stage, idx) => (
                  <StageCard
                    key={idx}
                    stage={stage}
                    index={idx}
                    status={stageStatuses[stage.name]}
                    cmdStatuses={cmdStatuses}
                    liveOutput={liveOutput}
                    running={running}
                    runs={runs}
                    selectedStageResult={selectedStageResult}
                    onSelectStageResult={onSelectStageResult}
                    onRunStage={onRun}
                  />
                ))}
              </div>

              {/* Stage result viewer removed from here — now rendered inline below each stage card */}
            </>
          )}
        </section>
      ) : (
        <section className="section">
          <div className="empty-state-small">
            <p>No pipeline configured yet.</p>
            <button className="btn btn-primary" onClick={onRegenerate}>
              <RefreshCw size={14} />
              Auto-detect & Generate Pipeline
            </button>
          </div>
        </section>
      )}
    </>
  );
}

export default PipelineTab;
