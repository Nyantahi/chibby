import { useEffect, useState } from 'react';
import { Database, RefreshCw, Scissors } from 'lucide-react';
import { getRunIndexStats, pruneRunIndex, rebuildRunIndex } from '../../services/api';
import { notifyError, notifySuccess } from '../../services/notify';
import { formatBytes } from '../../utils/insights';
import type { IndexStats } from '../../types';

interface RunIndexCardProps {
  /** Project the page is scoped to; null means every project. */
  repoPath: string | null;
  /** Re-fetch the report after maintenance changed what the index holds. */
  onChanged: () => void;
}

/**
 * The run index is what makes every number on this page cheap to compute, so
 * it has to be inspectable and fixable by hand when the numbers look wrong.
 */
function RunIndexCard({ repoPath, onChanged }: RunIndexCardProps) {
  const [stats, setStats] = useState<IndexStats | null>(null);
  const [busy, setBusy] = useState<'rebuild' | 'prune' | null>(null);

  async function loadStats() {
    try {
      setStats(await getRunIndexStats());
    } catch (err) {
      notifyError('Could not read the run index', err);
    }
  }

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    loadStats();
  }, []);

  async function handleRebuild() {
    if (!window.confirm('Rebuild the run index from the runs on disk? Metrics reload afterwards.'))
      return;
    try {
      setBusy('rebuild');
      const entries = await rebuildRunIndex();
      notifySuccess(`Run index rebuilt — ${entries} entries`);
      await loadStats();
      onChanged();
    } catch (err) {
      notifyError('Rebuilding the run index failed', err);
    } finally {
      setBusy(null);
    }
  }

  async function handlePrune() {
    // Dropping entries is irreversible, so the prompt has to say whose history
    // is at stake: the page's project, or every project.
    const scope = repoPath ? 'this project' : 'every project';
    if (
      !window.confirm(
        `Apply retention to the run index for ${scope}? Dropped entries leave metrics.`
      )
    )
      return;
    try {
      setBusy('prune');
      const dropped = await pruneRunIndex(repoPath, null, null);
      notifySuccess(`Run index pruned — ${dropped} entries dropped`);
      await loadStats();
      onChanged();
    } catch (err) {
      notifyError('Pruning the run index failed', err);
    } finally {
      setBusy(null);
    }
  }

  return (
    <section className="insights-section">
      <h3 className="section-title insights-section-title">
        <Database size={16} />
        Run index
      </h3>
      <p className="insights-section-desc">
        Every number above is read from this index. Run summaries outlive their logs, so a run
        counted here may no longer have log output to open.
      </p>

      <div className="insights-index-stats">
        <div className="insights-index-stat">
          <span className="insights-index-value">{stats ? stats.entries : '—'}</span>
          <span className="stat-label">Entries</span>
        </div>
        <div className="insights-index-stat">
          <span className="insights-index-value">{stats ? stats.payloads_pruned : '—'}</span>
          <span className="stat-label">With pruned logs</span>
        </div>
        <div className="insights-index-stat">
          <span className="insights-index-value">{stats ? formatBytes(stats.bytes) : '—'}</span>
          <span className="stat-label">On disk</span>
        </div>
      </div>

      <div className="insights-index-actions">
        <button
          type="button"
          className="btn btn-secondary btn-sm"
          onClick={handleRebuild}
          disabled={busy !== null}
        >
          <RefreshCw size={14} />
          {busy === 'rebuild' ? 'Rebuilding...' : 'Rebuild index'}
        </button>
        <button
          type="button"
          className="btn btn-secondary btn-sm"
          onClick={handlePrune}
          disabled={busy !== null}
        >
          <Scissors size={14} />
          {busy === 'prune' ? 'Pruning...' : 'Prune index'}
        </button>
      </div>
    </section>
  );
}

export default RunIndexCard;
