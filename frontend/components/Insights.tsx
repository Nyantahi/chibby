import { useCallback, useEffect, useState } from 'react';
import { BarChart3, RefreshCw } from 'lucide-react';
import { getInsights, listProjects } from '../services/api';
import { usePref, PREF_INSIGHTS_WINDOW } from '../services/prefs';
import { formatDate } from '../utils/format';
import TotalsSection from './insights/TotalsSection';
import EnvironmentMatrix from './insights/EnvironmentMatrix';
import StageReliabilityTable from './insights/StageReliabilityTable';
import HotspotsTable from './insights/HotspotsTable';
import SlowestStagesTable from './insights/SlowestStagesTable';
import DailyTable from './insights/DailyTable';
import RunIndexCard from './insights/RunIndexCard';
import type { InsightsReport, ProjectInfo } from '../types';

/** Windows offered by the selector, in days. */
const WINDOWS = [7, 30];

const ALL_PROJECTS = '';

/**
 * Cross-project metrics: what is live, which stage keeps failing, and whether
 * the pipeline is getting slower. Numbers and tables only — no charts.
 */
function Insights() {
  const [report, setReport] = useState<InsightsReport | null>(null);
  const [projects, setProjects] = useState<ProjectInfo[]>([]);
  const [repoPath, setRepoPath] = useState<string>(ALL_PROJECTS);
  const [windowDays, setWindowDays] = usePref<number>(PREF_INSIGHTS_WINDOW, WINDOWS[0]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadReport = useCallback(async () => {
    try {
      setError(null);
      setLoading(true);
      setReport(await getInsights(repoPath === ALL_PROJECTS ? null : repoPath, windowDays));
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, [repoPath, windowDays]);

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    loadReport();
  }, [loadReport]);

  useEffect(() => {
    listProjects()
      .then(setProjects)
      .catch(() => setProjects([]));
  }, []);

  return (
    <div className="page">
      <header className="page-header">
        <div>
          <h2 className="page-title">Insights</h2>
          {report && <p className="page-subtitle">Generated {formatDate(report.generated_at)}</p>}
        </div>

        <div className="header-actions">
          <select
            className="input insights-project-select"
            aria-label="Project filter"
            value={repoPath}
            onChange={(e) => setRepoPath(e.target.value)}
          >
            <option value={ALL_PROJECTS}>All projects</option>
            {projects.map(({ project }) => (
              <option key={project.id} value={project.path}>
                {project.name}
              </option>
            ))}
          </select>

          <div className="tabs" role="tablist" aria-label="Window">
            {WINDOWS.map((days) => (
              <button
                key={days}
                type="button"
                className={`tab ${windowDays === days ? 'tab-active' : ''}`}
                onClick={() => setWindowDays(days)}
                aria-pressed={windowDays === days}
              >
                {days} days
              </button>
            ))}
          </div>

          <button
            type="button"
            className="btn btn-secondary btn-sm"
            onClick={loadReport}
            disabled={loading}
          >
            <RefreshCw size={14} />
            Refresh
          </button>
        </div>
      </header>

      {error && <div className="alert alert-error">{error}</div>}

      {loading && !report ? (
        <div className="loading">Loading insights...</div>
      ) : report ? (
        <div className="insights-sections">
          <TotalsSection totals={report.totals} windowDays={report.window_days} />
          <EnvironmentMatrix environments={report.environments} />
          <StageReliabilityTable stages={report.stages} />
          <HotspotsTable hotspots={report.hotspots} />
          <SlowestStagesTable stages={report.slowest_stages} />
          <DailyTable daily={report.daily} />
          <RunIndexCard
            repoPath={repoPath === ALL_PROJECTS ? null : repoPath}
            onChanged={loadReport}
          />
        </div>
      ) : (
        <div className="empty-state">
          <BarChart3 size={24} strokeWidth={1} />
          <h3>No metrics yet</h3>
          <p className="text-muted">Run a pipeline and its numbers will show up here.</p>
        </div>
      )}
    </div>
  );
}

export default Insights;
