import { Link } from 'react-router-dom';
import { History, Trash2, Trophy, GitBranch, Clock, RotateCcw, Undo2 } from 'lucide-react';
import type { PipelineRun, EnvironmentsConfig } from '../../types';
import { formatDate, formatDuration, statusClass, capitalize } from '../../utils/format';
import { runScopeLabel } from './helpers';

interface HistoryTabProps {
  runs: PipelineRun[];
  filteredRuns: PipelineRun[];
  lastGoodRun: PipelineRun | null;
  envsConfig: EnvironmentsConfig;
  historyEnvFilter: string;
  onHistoryEnvFilterChange: (value: string) => void;
  onClearHistory: () => void;
  projectId: string | undefined;
}

function HistoryTab({
  runs,
  filteredRuns,
  lastGoodRun,
  envsConfig,
  historyEnvFilter,
  onHistoryEnvFilterChange,
  onClearHistory,
  projectId,
}: HistoryTabProps) {
  return (
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
              onChange={(e) => onHistoryEnvFilterChange(e.target.value)}
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
              onClick={onClearHistory}
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
                  <span className="badge badge-neutral run-scope-badge" title="Full pipeline run">
                    Full
                  </span>
                )}
                {run.environment && <span className="badge badge-neutral">{run.environment}</span>}
                {run.run_kind === 'retry' && (
                  <span className="badge badge-info" title={`Retry #${run.retry_number ?? 1}`}>
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
  );
}

export default HistoryTab;
