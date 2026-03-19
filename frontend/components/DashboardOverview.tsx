import { useEffect, useState, useMemo } from 'react';
import { Link } from 'react-router-dom';
import { CircleCheck, CircleX, Circle, RotateCcw, Activity } from 'lucide-react';
import { listProjects, getAllRuns, retryRun } from '../services/api';
import { formatDuration, capitalize } from '../utils/format';
import type { ProjectInfo, PipelineRun } from '../types';

function timeAgo(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return 'just now';
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

function isToday(iso: string): boolean {
  const d = new Date(iso);
  const now = new Date();
  return (
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate()
  );
}

function statusIcon(status?: string) {
  switch (status) {
    case 'success':
      return <CircleCheck size={14} className="status-icon status-success" />;
    case 'failed':
      return <CircleX size={14} className="status-icon status-failed" />;
    case 'running':
      return <Circle size={14} className="status-icon status-running" />;
    default:
      return <Circle size={14} className="status-icon status-pending" />;
  }
}

function DashboardOverview() {
  const [projects, setProjects] = useState<ProjectInfo[]>([]);
  const [runs, setRuns] = useState<PipelineRun[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [retrying, setRetrying] = useState<string | null>(null);

  useEffect(() => {
    loadData();
  }, []);

  async function loadData() {
    try {
      setLoading(true);
      const [p, r] = await Promise.all([listProjects(), getAllRuns()]);
      setProjects(p);
      setRuns(r);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  // Compute stats
  const stats = useMemo(() => {
    const todayRuns = runs.filter((r) => isToday(r.started_at));
    const todaySuccess = todayRuns.filter((r) => r.status === 'success').length;
    const todayTotal = todayRuns.length;
    const successRate = todayTotal > 0 ? Math.round((todaySuccess / todayTotal) * 100) : 0;

    // Failed runs needing attention: projects whose most recent run is failed
    const latestByProject = new Map<string, PipelineRun>();
    for (const run of runs) {
      const existing = latestByProject.get(run.repo_path);
      if (!existing || new Date(run.started_at) > new Date(existing.started_at)) {
        latestByProject.set(run.repo_path, run);
      }
    }
    const needsAttention = [...latestByProject.values()].filter(
      (r) => r.status === 'failed'
    ).length;

    return { totalProjects: projects.length, runsToday: todayTotal, successRate, needsAttention };
  }, [projects, runs]);

  const recentRuns = useMemo(() => runs.slice(0, 5), [runs]);

  // Map repo_path to project name for display
  const projectNameMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const { project } of projects) {
      map.set(project.path, project.name);
    }
    return map;
  }, [projects]);

  async function handleRetry(runId: string) {
    try {
      setRetrying(runId);
      await retryRun(runId);
      await loadData();
    } catch (err) {
      setError(String(err));
    } finally {
      setRetrying(null);
    }
  }

  if (loading) {
    return (
      <div className="page">
        <div className="loading">Loading dashboard...</div>
      </div>
    );
  }

  return (
    <div className="page">
      <header className="page-header">
        <h2 className="page-title">Dashboard</h2>
      </header>

      {error && <div className="alert alert-error">{error}</div>}

      {/* Stats bar */}
      <div className="stats-bar">
        <div className="stat-card">
          <div className="stat-value">{stats.totalProjects}</div>
          <div className="stat-label">Projects</div>
        </div>
        <div className="stat-card">
          <div className="stat-value">{stats.runsToday}</div>
          <div className="stat-label">Runs Today</div>
        </div>
        <div className="stat-card">
          <div
            className={`stat-value ${stats.successRate >= 80 ? 'stat-success' : stats.successRate > 0 ? 'stat-failed' : ''}`}
          >
            {stats.runsToday > 0 ? `${stats.successRate}%` : '--'}
          </div>
          <div className="stat-label">Success Rate</div>
        </div>
        <div className="stat-card">
          <div className={`stat-value ${stats.needsAttention > 0 ? 'stat-failed' : ''}`}>
            {stats.needsAttention}
          </div>
          <div className="stat-label">Needs Attention</div>
        </div>
      </div>

      {projects.length === 0 ? (
        <div className="empty-dashboard">
          <Activity size={48} strokeWidth={1} />
          <h3>No projects yet</h3>
          <p>Add a project to start seeing activity here.</p>
          <Link to="/add-project" className="btn btn-primary">
            Add Your First Project
          </Link>
        </div>
      ) : (
        <div className="dashboard-content">
          {/* Recent runs */}
          <div className="recent-runs-section">
            <div className="section-header">
              <h3 className="section-title">Recent Runs</h3>
              {recentRuns.length > 0 && (
                <Link to="/projects" className="btn btn-sm">
                  View All
                </Link>
              )}
            </div>
            {recentRuns.length === 0 ? (
              <p className="text-muted">No runs yet. Run a pipeline to see activity here.</p>
            ) : (
              <div className="recent-runs-list">
                {recentRuns.map((run) => (
                  <div key={run.id} className="recent-run-row">
                    {statusIcon(run.status)}
                    <Link to={`/run/${run.id}`} className="recent-run-project">
                      {projectNameMap.get(run.repo_path) || run.pipeline_name}
                    </Link>
                    <span className={`badge badge-${run.status}`}>{capitalize(run.status)}</span>
                    {run.branch && <span className="recent-run-branch">{run.branch}</span>}
                    <span className="recent-run-duration">{formatDuration(run.duration_ms)}</span>
                    <span className="recent-run-time">{timeAgo(run.started_at)}</span>
                    {run.status === 'failed' && (
                      <div className="recent-run-actions">
                        <button
                          className="btn btn-sm"
                          onClick={() => handleRetry(run.id)}
                          disabled={retrying === run.id}
                          title="Retry this run"
                        >
                          <RotateCcw size={12} />
                          {retrying === run.id ? ' ...' : ' Retry'}
                        </button>
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Project health */}
          <div className="health-section">
            <h3 className="section-title">Project Health</h3>
            <div className="health-list">
              {projects.map(({ project }) => {
                const status = project.last_run_status;
                const dotClass = status
                  ? `dot-${status === 'cancelled' ? 'none' : status}`
                  : 'dot-none';
                return (
                  <Link key={project.id} to={`/project/${project.id}`} className="health-item">
                    <span className={`health-dot ${dotClass}`} />
                    <span className="health-name">{project.name}</span>
                    {project.last_run_at && (
                      <span className="recent-run-time">{timeAgo(project.last_run_at)}</span>
                    )}
                  </Link>
                );
              })}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default DashboardOverview;
