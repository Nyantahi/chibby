import { useEffect, useState, useMemo } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import {
  FolderGit2,
  CircleCheck,
  CircleX,
  Circle,
  Loader2,
  LayoutGrid,
  Table as TableIcon,
} from 'lucide-react';
import { listProjects, getAllRuns } from '../services/api';
import { useActiveRuns } from '../services/runStore';
import { usePref, PREF_PROJECTS_VIEW, type ProjectsView } from '../services/prefs';
import { formatDate, statusClass, capitalize } from '../utils/format';
import type { ProjectInfo, PipelineRun } from '../types';

function isToday(iso: string): boolean {
  const d = new Date(iso);
  const now = new Date();
  return (
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate()
  );
}

function Projects() {
  const [projects, setProjects] = useState<ProjectInfo[]>([]);
  const [runs, setRuns] = useState<PipelineRun[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [view, setView] = usePref<ProjectsView>(PREF_PROJECTS_VIEW, 'cards');

  const navigate = useNavigate();
  const activeRuns = useActiveRuns();
  const runningPaths = useMemo(
    () => new Set(activeRuns.filter((r) => r.status === 'running').map((r) => r.repoPath)),
    [activeRuns]
  );

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

  // One-shot data fetch on mount. setState inside the .finally is intentional
  // — the rule is overly strict here.
  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    loadData();
  }, []);

  const stats = useMemo(() => {
    const todayRuns = runs.filter((r) => isToday(r.started_at));
    const todaySuccess = todayRuns.filter((r) => r.status === 'success').length;
    const todayTotal = todayRuns.length;
    const successRate = todayTotal > 0 ? Math.round((todaySuccess / todayTotal) * 100) : 0;

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

  if (loading) {
    return (
      <div className="page">
        <div className="loading">Loading projects...</div>
      </div>
    );
  }

  return (
    <div className="page">
      <header className="page-header">
        <h2 className="page-title">Projects</h2>
        <div className="header-actions">
          {projects.length > 0 && (
            <div className="tabs" role="tablist" aria-label="Project view">
              <button
                type="button"
                className={`tab ${view === 'cards' ? 'tab-active' : ''}`}
                onClick={() => setView('cards')}
                aria-pressed={view === 'cards'}
              >
                <LayoutGrid size={14} />
                Cards
              </button>
              <button
                type="button"
                className={`tab ${view === 'table' ? 'tab-active' : ''}`}
                onClick={() => setView('table')}
                aria-pressed={view === 'table'}
              >
                <TableIcon size={14} />
                Table
              </button>
            </div>
          )}
          <Link to="/add-project" className="btn btn-primary">
            Add Project
          </Link>
        </div>
      </header>

      {error && <div className="alert alert-error">{error}</div>}

      {/* Stats bar */}
      {projects.length > 0 && (
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
      )}

      {projects.length === 0 ? (
        <div className="empty-state">
          <FolderGit2 size={48} strokeWidth={1} />
          <h3>No projects yet</h3>
          <p>Add a repository to get started with Chibby.</p>
          <Link to="/add-project" className="btn btn-primary">
            Add Your First Project
          </Link>
        </div>
      ) : view === 'cards' ? (
        <div className="project-grid">
          {projects.map(({ project, has_pipeline }) => {
            const isRunning = runningPaths.has(project.path);
            // While running, show live "Running" instead of the stale last-run summary.
            const displayStatus = isRunning ? 'running' : project.last_run_status;
            return (
              <Link key={project.id} to={`/project/${project.id}`} className="project-card">
                <div className="project-card-header">
                  <FolderGit2 size={18} />
                  <h3 className="project-name">{project.name}</h3>
                  {isRunning && <Loader2 size={14} className="spin status-running" />}
                </div>
                <p className="project-path">{project.path}</p>
                <div className="project-card-footer">
                  <span className={`badge badge-${has_pipeline ? 'success' : 'neutral'}`}>
                    {has_pipeline ? 'Pipeline configured' : 'No pipeline'}
                  </span>
                  {displayStatus && (
                    <div className="project-last-run">
                      <span className="project-run-status">
                        {statusIcon(displayStatus)}
                        <span className={`status-text status-${statusClass(displayStatus)}`}>
                          {isRunning ? 'Running' : capitalize(displayStatus)}
                        </span>
                      </span>
                      {!isRunning && project.last_run_at && (
                        <span className="run-date">{formatDate(project.last_run_at)}</span>
                      )}
                    </div>
                  )}
                </div>
              </Link>
            );
          })}
        </div>
      ) : (
        <table className="project-table">
          <thead>
            <tr>
              <th>Project</th>
              <th>Path</th>
              <th>Pipeline</th>
              <th>Status</th>
              <th>Last Run</th>
            </tr>
          </thead>
          <tbody>
            {projects.map(({ project, has_pipeline }) => {
              const isRunning = runningPaths.has(project.path);
              const displayStatus = isRunning ? 'running' : project.last_run_status;
              return (
                <tr
                  key={project.id}
                  className="project-row"
                  onClick={() => navigate(`/project/${project.id}`)}
                >
                  <td>
                    <div className="project-row-name">
                      <FolderGit2 size={16} />
                      <span>{project.name}</span>
                      {isRunning && <Loader2 size={14} className="spin status-running" />}
                    </div>
                  </td>
                  <td className="project-row-path">{project.path}</td>
                  <td>
                    <span className={`badge badge-${has_pipeline ? 'success' : 'neutral'}`}>
                      {has_pipeline ? 'Pipeline configured' : 'No pipeline'}
                    </span>
                  </td>
                  <td>
                    {displayStatus ? (
                      <span className="project-run-status">
                        {statusIcon(displayStatus)}
                        <span className={`status-text status-${statusClass(displayStatus)}`}>
                          {isRunning ? 'Running' : capitalize(displayStatus)}
                        </span>
                      </span>
                    ) : (
                      <span className="project-row-dim">—</span>
                    )}
                  </td>
                  <td className="project-row-dim">
                    {!isRunning && project.last_run_at ? formatDate(project.last_run_at) : '—'}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </div>
  );
}

export default Projects;
