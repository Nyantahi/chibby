import { useEffect, useState } from 'react';
import { Trash2, Loader2 } from 'lucide-react';
import { loadCleanupConfig, runCleanup, saveCleanupConfig } from '../services/api';
import { notifyError, notifySuccess } from '../services/notify';
import type { CleanupConfig, CleanupResult } from '../types';

interface Props {
  repoPath: string;
}

function CleanupCard({ repoPath }: Props) {
  const [cfg, setCfg] = useState<CleanupConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<CleanupResult | null>(null);
  const [dryRun, setDryRun] = useState(true);

  useEffect(() => {
    loadCleanupConfig(repoPath)
      .then(setCfg)
      .catch((err) => notifyError('Load cleanup failed', err))
      .finally(() => setLoading(false));
  }, [repoPath]);

  function update<K extends keyof CleanupConfig>(key: K, value: CleanupConfig[K]) {
    setCfg((p) => (p ? { ...p, [key]: value } : p));
  }

  async function handleSave() {
    if (!cfg) return;
    setSaving(true);
    try {
      await saveCleanupConfig(repoPath, cfg);
      notifySuccess('Cleanup config saved');
    } catch (err) {
      notifyError('Save failed', err);
    } finally {
      setSaving(false);
    }
  }

  async function handleRun() {
    setRunning(true);
    setResult(null);
    try {
      const r = await runCleanup(repoPath, dryRun);
      setResult(r);
      notifySuccess(
        dryRun ? 'Dry-run cleanup' : 'Cleanup complete',
        `${r.artifacts_removed} artifacts, ${r.runs_removed} runs, ${(r.bytes_freed / 1024).toFixed(1)} KB`
      );
    } catch (err) {
      notifyError('Cleanup failed', err);
    } finally {
      setRunning(false);
    }
  }

  return (
    <div className="feature-card">
      <div className="feature-card-header">
        <div className="feature-card-title">
          <Trash2 size={16} /> Cleanup
        </div>
        <div className="feature-card-actions">
          <button className="btn btn-sm btn-secondary" onClick={handleSave} disabled={saving}>
            {saving ? 'Saving…' : 'Save'}
          </button>
          <button className="btn btn-sm btn-primary" onClick={handleRun} disabled={running}>
            {running ? 'Running…' : dryRun ? 'Dry run' : 'Run'}
          </button>
        </div>
      </div>
      <div className="feature-card-body">
        {loading ? (
          <div className="feature-card-empty">
            <Loader2 size={14} className="spin" /> Loading…
          </div>
        ) : cfg ? (
          <>
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8 }}>
              <div className="form-row">
                <label>Artifact retention</label>
                <input
                  className="input"
                  type="number"
                  value={cfg.artifact_retention}
                  onChange={(e) => update('artifact_retention', parseInt(e.target.value, 10) || 0)}
                />
              </div>
              <div className="form-row">
                <label>Run retention</label>
                <input
                  className="input"
                  type="number"
                  value={cfg.run_retention}
                  onChange={(e) => update('run_retention', parseInt(e.target.value, 10) || 0)}
                />
              </div>
            </div>
            <label style={{ display: 'flex', gap: 6, alignItems: 'center', fontSize: 'var(--font-size-xs)' }}>
              <input
                type="checkbox"
                checked={cfg.prune_remote_docker}
                onChange={(e) => update('prune_remote_docker', e.target.checked)}
              />
              Prune remote Docker images on SSH hosts
            </label>
            <label style={{ display: 'flex', gap: 6, alignItems: 'center', fontSize: 'var(--font-size-xs)' }}>
              <input
                type="checkbox"
                checked={dryRun}
                onChange={(e) => setDryRun(e.target.checked)}
              />
              Dry run (don't actually delete)
            </label>

            {result && (
              <div style={{ marginTop: 8, fontSize: 'var(--font-size-xs)' }}>
                <strong>
                  {result.artifacts_removed} artifacts · {result.runs_removed} runs ·{' '}
                  {(result.bytes_freed / 1024).toFixed(1)} KB
                </strong>
                {result.details.length > 0 && (
                  <ul style={{ paddingLeft: 16, marginTop: 4 }}>
                    {result.details.map((d, i) => (
                      <li key={i}>{d}</li>
                    ))}
                  </ul>
                )}
              </div>
            )}
          </>
        ) : null}
      </div>
    </div>
  );
}

export default CleanupCard;
