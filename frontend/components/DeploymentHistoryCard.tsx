import { useEffect, useState } from 'react';
import { History, Loader2 } from 'lucide-react';
import { Link } from 'react-router-dom';
import { getDeploymentHistory } from '../services/api';
import { notifyError } from '../services/notify';
import { formatDate, formatDuration, statusClass, capitalize } from '../utils/format';
import type { DeploymentRecord, Environment } from '../types';

interface Props {
  repoPath: string;
  environments: Environment[];
}

function DeploymentHistoryCard({ repoPath, environments }: Props) {
  const [byEnv, setByEnv] = useState<Record<string, DeploymentRecord[]>>({});
  const [loading, setLoading] = useState(environments.length > 0);

  useEffect(() => {
    if (environments.length === 0) return;
    Promise.all(
      environments.map((e) =>
        getDeploymentHistory(repoPath, e.name)
          .then((r) => [e.name, r] as const)
          .catch((err) => {
            notifyError(`History for ${e.name} failed`, err);
            return [e.name, [] as DeploymentRecord[]] as const;
          })
      )
    )
      .then((pairs) => {
        const m: Record<string, DeploymentRecord[]> = {};
        for (const [k, v] of pairs) m[k] = v;
        setByEnv(m);
      })
      .finally(() => setLoading(false));
  }, [repoPath, environments]);

  return (
    <div className="feature-card">
      <div className="feature-card-header">
        <div className="feature-card-title">
          <History size={16} /> Deployment history
        </div>
      </div>
      <div className="feature-card-body">
        {loading ? (
          <div className="feature-card-empty">
            <Loader2 size={14} className="spin" /> Loading…
          </div>
        ) : environments.length === 0 ? (
          <div className="feature-card-empty">Define environments first.</div>
        ) : (
          environments.map((e) => (
            <div key={e.name}>
              <strong style={{ fontSize: 'var(--font-size-sm)' }}>{e.name}</strong>
              {(byEnv[e.name]?.length ?? 0) === 0 ? (
                <p className="text-muted" style={{ fontSize: 'var(--font-size-xs)' }}>
                  No deployments yet.
                </p>
              ) : (
                <table className="kv-table">
                  <thead>
                    <tr>
                      <th>When</th>
                      <th>Status</th>
                      <th>Kind</th>
                      <th>Branch</th>
                      <th>Duration</th>
                      <th></th>
                    </tr>
                  </thead>
                  <tbody>
                    {byEnv[e.name].map((d) => (
                      <tr key={d.run_id}>
                        <td>{formatDate(d.started_at)}</td>
                        <td>
                          <span className={`badge badge-${statusClass(d.status)}`}>
                            {capitalize(d.status)}
                          </span>
                        </td>
                        <td>{d.run_kind}</td>
                        <td>{d.branch ?? '—'}</td>
                        <td>{d.duration_ms ? formatDuration(d.duration_ms) : '—'}</td>
                        <td>
                          <Link to={`/run/${d.run_id}`} className="btn btn-xs btn-ghost">
                            View
                          </Link>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>
          ))
        )}
      </div>
    </div>
  );
}

export default DeploymentHistoryCard;
