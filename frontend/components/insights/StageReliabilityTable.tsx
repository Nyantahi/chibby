import { Repeat, ShieldAlert } from 'lucide-react';
import InsightsSection from './InsightsSection';
import RunLink from './RunLink';
import { formatMs, formatRate } from '../../utils/insights';
import type { StageReliability } from '../../types';

interface StageReliabilityTableProps {
  stages: StageReliability[];
}

/**
 * A row per stage. Flakiness is reported apart from failure rate on purpose:
 * a stage that never fails but only passes on retry is not a healthy stage,
 * and merging the two numbers would hide exactly that.
 */
function StageReliabilityTable({ stages }: StageReliabilityTableProps) {
  return (
    <InsightsSection
      title="Stage reliability"
      icon={<ShieldAlert size={16} />}
      description="Failure rate and flakiness per stage. A flaky pass is an execution that only succeeded after a retry."
      isEmpty={stages.length === 0}
      emptyText="No stage executions in this window yet."
    >
      <div className="insights-table-wrap">
        <table className="insights-table">
          <thead>
            <tr>
              <th>Stage</th>
              <th>Runs</th>
              <th>Failures</th>
              <th>Timeouts</th>
              <th>Failure rate</th>
              <th>Flaky passes</th>
              <th>Flaky rate</th>
              <th>Avg</th>
              <th>p95</th>
              <th>Slowest run</th>
            </tr>
          </thead>
          <tbody>
            {stages.map((stage) => (
              <StageRow key={stage.stage_name} stage={stage} />
            ))}
          </tbody>
        </table>
      </div>
    </InsightsSection>
  );
}

function StageRow({ stage }: { stage: StageReliability }) {
  const flaky = stage.flaky_passes > 0;

  return (
    <tr className={flaky ? 'insights-row-flaky' : undefined}>
      <td>{stage.stage_name}</td>
      <td>{stage.runs}</td>
      <td className={stage.failures > 0 ? 'insights-cell-failed' : undefined}>{stage.failures}</td>
      <td>{stage.timeouts}</td>
      <td className={stage.failures > 0 ? 'insights-cell-failed' : undefined}>
        {formatRate(stage.failure_rate)}
      </td>
      <td>
        {flaky ? (
          <span
            className="badge badge-warning insights-badge-icon"
            title="Passes only after a retry — unreliable even at a 0% failure rate"
          >
            <Repeat size={12} aria-hidden="true" />
            {stage.flaky_passes} flaky
          </span>
        ) : (
          0
        )}
      </td>
      <td className={flaky ? 'insights-cell-flaky' : undefined}>{formatRate(stage.flaky_rate)}</td>
      <td>{formatMs(stage.avg_duration_ms)}</td>
      <td>{formatMs(stage.p95_duration_ms)}</td>
      <td>
        <RunLink runId={stage.slowest_run_id} />
      </td>
    </tr>
  );
}

export default StageReliabilityTable;
