import { useEffect, useState, useMemo } from 'react';
import { Link } from 'react-router-dom';
import { FolderGit2, CircleCheck, CircleX, Circle } from 'lucide-react';
import { listProjects, getAllRuns } from '../services/api';
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
        <Link to="/add-project" className="btn btn-primary">
          Add Project
        </Link>
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
      ) : (
        <div className="project-grid">
          {projects.map(({ project, has_pipeline }) => (
            <Link key={project.id} to={`/project/${project.id}`} className="project-card">
              <div className="project-card-header">
                <FolderGit2 size={18} />
                <h3 className="project-name">{project.name}</h3>
              </div>
              <p className="project-path">{project.path}</p>
              <div className="project-card-footer">
                <span className={`badge badge-${has_pipeline ? 'success' : 'neutral'}`}>
                  {has_pipeline ? 'Pipeline configured' : 'No pipeline'}
                </span>
                {project.last_run_status && (
                  <div className="project-last-run">
                    <span className="project-run-status">
                      {statusIcon(project.last_run_status)}
                      <span className={`status-text status-${statusClass(project.last_run_status)}`}>
                        {capitalize(project.last_run_status)}
                      </span>
                    </span>
                    {project.last_run_at && (
                      <span className="run-date">{formatDate(project.last_run_at)}</span>
                    )}
                  </div>
                )}
              </div>
            </Link>
          ))}
        </div>
      )}
    </div>
  );
}

export default Projects;
