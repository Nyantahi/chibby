import { Flame } from 'lucide-react';
import InsightsSection from './InsightsSection';
import { NO_VALUE } from '../../utils/insights';
import type { FailureHotspot } from '../../types';

interface HotspotsTableProps {
  hotspots: FailureHotspot[];
}

/**
 * Where each project's failures land. Health-check failures stay in their own
 * column: a command that exited non-zero and a deploy whose health check never
 * came up are different problems with different fixes.
 */
function HotspotsTable({ hotspots }: HotspotsTableProps) {
  return (
    <InsightsSection
      title="Failure hotspots"
      icon={<Flame size={16} />}
      description="The worst failing stage per project. Health-check failures are counted separately from failing commands."
      isEmpty={hotspots.length === 0}
      emptyText="No failures in this window."
    >
      <div className="insights-table-wrap">
        <table className="insights-table">
          <thead>
            <tr>
              <th>Project</th>
              <th>Top failing stage</th>
              <th>Stage failures</th>
              <th>Total failures</th>
              <th>Health-check failures</th>
              <th>Top health-check stage</th>
            </tr>
          </thead>
          <tbody>
            {hotspots.map((hotspot) => (
              <tr key={hotspot.repo_path}>
                <td>{hotspot.project_name}</td>
                <td>{hotspot.top_failing_stage ?? NO_VALUE}</td>
                <td>{hotspot.stage_failures}</td>
                <td className={hotspot.total_failures > 0 ? 'insights-cell-failed' : undefined}>
                  {hotspot.total_failures}
                </td>
                <td>
                  {hotspot.health_check_failures > 0 ? (
                    <span
                      className="badge badge-warning"
                      title="Commands passed but the post-deploy health check did not"
                    >
                      {hotspot.health_check_failures} health
                    </span>
                  ) : (
                    0
                  )}
                </td>
                <td>{hotspot.top_health_failure_stage ?? NO_VALUE}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </InsightsSection>
  );
}

export default HotspotsTable;
