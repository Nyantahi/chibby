import { Timer } from 'lucide-react';
import DeltaBadge from './DeltaBadge';
import InsightsSection from './InsightsSection';
import { formatMs } from '../../utils/insights';
import type { StageDelta } from '../../types';

interface SlowestStagesTableProps {
  stages: StageDelta[];
}

/**
 * "Is this pipeline getting slower?" — current average against the previous
 * window's. A stage with no previous-window baseline shows a dash, not zero.
 */
function SlowestStagesTable({ stages }: SlowestStagesTableProps) {
  return (
    <InsightsSection
      title="Slowest stages"
      icon={<Timer size={16} />}
      description="Average duration this window against the window before it, worst first."
      isEmpty={stages.length === 0}
      emptyText="Not enough history to compare stage durations yet."
    >
      <div className="insights-table-wrap">
        <table className="insights-table">
          <thead>
            <tr>
              <th>Stage</th>
              <th>Runs</th>
              <th>Current avg</th>
              <th>Previous avg</th>
              <th>Change</th>
            </tr>
          </thead>
          <tbody>
            {stages.map((stage) => (
              <tr key={stage.stage_name}>
                <td>{stage.stage_name}</td>
                <td>{stage.runs}</td>
                <td>{formatMs(stage.current_avg_ms)}</td>
                <td>{formatMs(stage.previous_avg_ms)}</td>
                <td>
                  <DeltaBadge value={stage.delta_pct} unit="percent" lowerIsBetter />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </InsightsSection>
  );
}

export default SlowestStagesTable;
