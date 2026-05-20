import { useEffect, useMemo, useState } from 'react';
import { X, Loader2, KeyRound, FileCode2, AlertTriangle } from 'lucide-react';
import { applyBootstrap, scanBootstrap } from '../services/api';
import { notifyError, notifySuccess } from '../services/notify';
import type { BootstrapReport, Classification, DetectedName, SourceKind } from '../types';

const SOURCE_LABEL: Record<SourceKind, string> = {
  dot_env: '.env',
  docker_compose: 'docker-compose',
  gha_workflow: 'GitHub Actions',
  js_code: 'JS/TS source',
  py_code: 'Python source',
  rs_code: 'Rust source',
};

interface Props {
  repoPath: string;
  /** Optional pre-fetched report (e.g. from auto-bootstrap). */
  initialReport?: BootstrapReport | null;
  onClose: () => void;
  onApplied?: () => void;
}

function ClassificationBadge({ kind }: { kind: Classification }) {
  if (kind === 'secret') {
    return (
      <span className="bw-badge bw-badge-secret">
        <KeyRound size={12} /> secret
      </span>
    );
  }
  return (
    <span className="bw-badge bw-badge-var">
      <FileCode2 size={12} /> variable
    </span>
  );
}

function DetectedRow({ row }: { row: DetectedName }) {
  return (
    <div className="bw-row">
      <div className="bw-row-head">
        <ClassificationBadge kind={row.classification} />
        <code>{row.name}</code>
      </div>
      <div className="bw-row-sources">
        {row.sources.map((s, i) => (
          <span key={`${s.path}-${i}`} className="bw-source-chip" title={s.path}>
            {SOURCE_LABEL[s.kind] ?? s.kind} · {s.path}
          </span>
        ))}
      </div>
    </div>
  );
}

function BootstrapWizardModal({ repoPath, initialReport, onClose, onApplied }: Props) {
  const [report, setReport] = useState<BootstrapReport | null>(initialReport ?? null);
  const [loading, setLoading] = useState(!initialReport);
  const [applying, setApplying] = useState(false);
  const [merge, setMerge] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (initialReport) return;
    let cancelled = false;
    scanBootstrap(repoPath)
      .then((r) => {
        if (!cancelled) setReport(r);
      })
      .catch((err) => {
        if (!cancelled) setError(String(err));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [repoPath, initialReport]);

  const counts = useMemo(() => {
    if (!report) return { secrets: 0, variables: 0 };
    let s = 0;
    let v = 0;
    for (const d of report.detected) {
      if (d.classification === 'secret') s++;
      else v++;
    }
    return { secrets: s, variables: v };
  }, [report]);

  async function handleApply() {
    if (!report || report.detected.length === 0) return;
    setApplying(true);
    setError(null);
    try {
      const wrote = await applyBootstrap(repoPath, report, merge);
      if (wrote) {
        notifySuccess('Bootstrap applied', `${report.detected.length} names written`);
        onApplied?.();
        onClose();
      } else {
        setError(
          'Nothing was written. Configs already exist — switch to Merge mode to append missing names.'
        );
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg);
      notifyError('Bootstrap failed', err);
    } finally {
      setApplying(false);
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="modal"
        onClick={(e) => e.stopPropagation()}
        style={{ maxWidth: 720, width: '90%' }}
      >
        <div className="modal-header">
          <h3>Bootstrap project</h3>
          <button className="btn-icon" onClick={onClose} aria-label="Close">
            <X size={16} />
          </button>
        </div>

        <div className="modal-body" style={{ maxHeight: '60vh', overflow: 'auto' }}>
          <p className="text-muted" style={{ fontSize: 'var(--font-size-xs)', marginBottom: 12 }}>
            Chibby scans <code>.env</code> files, Docker Compose, GitHub workflows, and source code
            for environment variable and secret references, then writes them into{' '}
            <code>.chibby/environments.toml</code> and <code>.chibby/secrets.toml</code>.
          </p>

          {loading && (
            <div className="bw-loading">
              <Loader2 size={16} className="spin" /> Scanning project…
            </div>
          )}

          {!loading && report && (
            <>
              <div className="bw-summary">
                <span>
                  <strong>{report.scanned_files}</strong> files scanned
                </span>
                <span>
                  <strong>{counts.secrets}</strong> secrets
                </span>
                <span>
                  <strong>{counts.variables}</strong> variables
                </span>
                {report.suggested_environments.length > 0 && (
                  <span>
                    envs:{' '}
                    {report.suggested_environments.map((e) => (
                      <code key={e} style={{ marginRight: 4 }}>
                        {e}
                      </code>
                    ))}
                  </span>
                )}
              </div>

              {report.detected.length === 0 ? (
                <div className="bw-empty">No env/secret references detected. Nothing to do.</div>
              ) : (
                <div className="bw-list">
                  {report.detected.map((d) => (
                    <DetectedRow key={d.name} row={d} />
                  ))}
                </div>
              )}
            </>
          )}

          {error && (
            <div className="bw-error">
              <AlertTriangle size={14} /> {error}
            </div>
          )}
        </div>

        <div className="modal-footer" style={{ justifyContent: 'space-between' }}>
          <label
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 6,
              fontSize: 'var(--font-size-xs)',
              color: 'var(--color-text-muted)',
            }}
          >
            <input
              type="checkbox"
              checked={merge}
              onChange={(e) => setMerge(e.target.checked)}
              disabled={applying}
            />
            Merge with existing files (append missing names only)
          </label>
          <div style={{ display: 'flex', gap: 8 }}>
            <button className="btn btn-ghost" onClick={onClose} disabled={applying}>
              Cancel
            </button>
            <button
              className="btn btn-primary"
              onClick={handleApply}
              disabled={applying || loading || !report || report.detected.length === 0}
            >
              {applying ? 'Applying…' : 'Apply'}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

export default BootstrapWizardModal;
