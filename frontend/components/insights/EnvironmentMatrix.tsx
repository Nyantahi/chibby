import { Server, TriangleAlert } from 'lucide-react';
import InsightsSection from './InsightsSection';
import RunLink from './RunLink';
import { formatDate, capitalize } from '../../utils/format';
import { NO_VALUE, rollbackClass, shortCommit } from '../../utils/insights';
import type { EnvironmentStatus } from '../../types';

interface EnvironmentMatrixProps {
  environments: EnvironmentStatus[];
}

/** What is live on every project × environment, and whether it is behind. */
function EnvironmentMatrix({ environments }: EnvironmentMatrixProps) {
  return (
    <InsightsSection
      title="Deploy health"
      icon={<Server size={16} />}
      description="What is live right now. Ignores the window — what shipped is live however long ago it shipped."
      isEmpty={environments.length === 0}
      emptyText="No environment-scoped runs yet. Deploy to an environment to populate this matrix."
    >
      <div className="insights-table-wrap">
        <table className="insights-table">
          <thead>
            <tr>
              <th>Project</th>
              <th>Environment</th>
              <th>Live commit</th>
              <th>Branch</th>
              <th>Deployed</th>
              <th>State</th>
              <th>Run</th>
              <th>Last rollback</th>
            </tr>
          </thead>
          <tbody>
            {environments.map((env) => (
              <EnvironmentRow key={`${env.repo_path}:${env.environment}`} env={env} />
            ))}
          </tbody>
        </table>
      </div>
    </InsightsSection>
  );
}

/** One project × environment row. A stale one carries the visual weight. */
function EnvironmentRow({ env }: { env: EnvironmentStatus }) {
  const stale = env.is_stale && env.failed_since > 0;

  return (
    <tr className={stale ? 'insights-row-stale' : undefined}>
      <td>{env.project_name}</td>
      <td>{env.environment}</td>
      <td className="insights-mono">{shortCommit(env.commit)}</td>
      <td className="insights-mono">{env.branch ?? NO_VALUE}</td>
      <td>{env.deployed_at ? formatDate(env.deployed_at) : NO_VALUE}</td>
      <td>
        {stale ? (
          <span className="badge badge-failed insights-badge-icon">
            <TriangleAlert size={12} aria-hidden="true" />
            Stale · {env.failed_since} failed since
          </span>
        ) : env.current_run_id ? (
          <span className="badge badge-success">Live</span>
        ) : (
          <span className="badge badge-neutral">Never deployed</span>
        )}
      </td>
      <td>
        <RunLink runId={env.current_run_id} />
      </td>
      <td>
        {env.last_rollback_outcome ? (
          <span className={`badge badge-${rollbackClass(env.last_rollback_outcome)}`}>
            {capitalize(env.last_rollback_outcome)}
            {env.last_rollback_at ? ` · ${formatDate(env.last_rollback_at)}` : ''}
          </span>
        ) : (
          <span className="text-muted">{NO_VALUE}</span>
        )}
      </td>
    </tr>
  );
}

export default EnvironmentMatrix;
