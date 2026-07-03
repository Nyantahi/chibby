import { Link } from 'react-router-dom';
import { Loader2 } from 'lucide-react';
import { useActiveRuns } from '../services/runStore';

/**
 * Sidebar surface listing pipelines currently running across all projects.
 * Renders nothing when no run is in progress. Each entry links to its project.
 */
function RunningRunsBadge() {
  const runs = useActiveRuns().filter((r) => r.status === 'running');
  if (runs.length === 0) return null;

  return (
    <div className="running-runs">
      <div className="running-runs-header">
        <Loader2 size={14} className="spin" />
        <span>
          {runs.length} run{runs.length > 1 ? 's' : ''} in progress
        </span>
      </div>
      <ul className="running-runs-list">
        {runs.map((run) => {
          const label = run.projectName ?? run.repoPath;
          const inner = (
            <>
              <span className="running-runs-name">{label}</span>
              {run.runningStageName && (
                <span className="running-runs-stage">{run.runningStageName}</span>
              )}
            </>
          );
          return (
            <li key={run.repoPath} className="running-runs-item">
              {run.projectId ? (
                <Link to={`/project/${run.projectId}`} className="running-runs-link">
                  {inner}
                </Link>
              ) : (
                inner
              )}
            </li>
          );
        })}
      </ul>
    </div>
  );
}

export default RunningRunsBadge;
