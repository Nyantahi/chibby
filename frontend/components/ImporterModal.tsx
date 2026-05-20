import { useEffect, useState } from 'react';
import { X, Loader2, AlertTriangle } from 'lucide-react';
import { open } from '@tauri-apps/plugin-dialog';
import { importerCliStatus, runImporter } from '../services/api';
import { notifyError, notifySuccess } from '../services/notify';
import type { ApplyReport, ImportReport, ImporterSource } from '../types';

interface Props {
  repoPath: string;
  environments: string[];
  onClose: () => void;
  onDone?: () => void;
}

const SOURCES: { id: ImporterSource; label: string; cli: string }[] = [
  { id: 'dotenv', label: '.env file', cli: '' },
  { id: 'vercel', label: 'Vercel', cli: 'vercel' },
  { id: 'railway', label: 'Railway', cli: 'railway' },
  { id: 'fly', label: 'Fly.io', cli: 'flyctl' },
];

function ImporterModal({ repoPath, environments, onClose, onDone }: Props) {
  const [source, setSource] = useState<ImporterSource>('dotenv');
  const [envName, setEnvName] = useState<string>(environments[0]?.toString() ?? 'production');
  const [sourcePath, setSourcePath] = useState<string>('');
  const [withValues, setWithValues] = useState(true);
  const [persistSecrets, setPersistSecrets] = useState(true);
  /** CLI status tagged with the source it was fetched for, so we can detect
   *  a stale result if the source has changed since the check started. */
  const [cliStatus, setCliStatus] = useState<{ source: ImporterSource; ok: boolean } | null>(null);
  const [running, setRunning] = useState(false);
  const [report, setReport] = useState<ImportReport | null>(null);
  const [apply, setApply] = useState<ApplyReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Refetch CLI status when the importer source changes. Tag every result with
  // its source so the renderer can fall back to "Checking…" if the source
  // changed while the previous probe was in flight.
  useEffect(() => {
    let cancelled = false;
    importerCliStatus(source)
      .then((ok) => {
        if (!cancelled) setCliStatus({ source, ok });
      })
      .catch(() => {
        if (!cancelled) setCliStatus({ source, ok: false });
      });
    return () => {
      cancelled = true;
    };
  }, [source]);

  /** ok|false if the most-recent probe matches the current source, null otherwise. */
  const cliOk = cliStatus && cliStatus.source === source ? cliStatus.ok : null;

  async function handlePickFile() {
    const picked = await open({
      title: 'Select .env file',
      multiple: false,
      directory: false,
    });
    if (typeof picked === 'string') setSourcePath(picked);
  }

  async function handleRun() {
    setRunning(true);
    setError(null);
    setReport(null);
    setApply(null);
    try {
      const [r, a] = await runImporter(
        source,
        repoPath,
        envName,
        source === 'dotenv' ? sourcePath || undefined : undefined,
        withValues,
        persistSecrets
      );
      setReport(r);
      setApply(a);
      notifySuccess(
        'Import complete',
        `${a.variables_added} variables, ${a.secrets_ref_added} secret refs, ${a.secrets_value_saved} secret values`
      );
      onDone?.();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg);
      notifyError('Importer failed', err);
    } finally {
      setRunning(false);
    }
  }

  const cliSource = SOURCES.find((s) => s.id === source);
  const cliNeeded = source !== 'dotenv';

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="modal"
        onClick={(e) => e.stopPropagation()}
        style={{ maxWidth: 640, width: '90%' }}
      >
        <div className="modal-header">
          <h3>Import environment</h3>
          <button className="btn-icon" onClick={onClose} aria-label="Close">
            <X size={16} />
          </button>
        </div>

        <div className="modal-body">
          <div className="form-row">
            <label>Source</label>
            <select
              className="input"
              value={source}
              onChange={(e) => setSource(e.target.value as ImporterSource)}
              disabled={running}
            >
              {SOURCES.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.label}
                </option>
              ))}
            </select>
          </div>

          {cliNeeded && (
            <div className="text-muted" style={{ fontSize: 'var(--font-size-xs)' }}>
              Requires <code>{cliSource?.cli}</code> CLI installed and authenticated.{' '}
              {cliOk === null && 'Checking…'}
              {cliOk === true && <span style={{ color: 'var(--color-success)' }}>found ✓</span>}
              {cliOk === false && <span style={{ color: 'var(--color-failed)' }}>not found</span>}
            </div>
          )}

          <div className="form-row">
            <label>Target environment</label>
            <input
              className="input"
              value={envName}
              onChange={(e) => setEnvName(e.target.value)}
              disabled={running}
            />
          </div>

          {source === 'dotenv' && (
            <div className="form-row">
              <label>.env file</label>
              <div style={{ display: 'flex', gap: 6 }}>
                <input
                  className="input"
                  value={sourcePath}
                  onChange={(e) => setSourcePath(e.target.value)}
                  placeholder="/path/to/.env"
                  disabled={running}
                />
                <button
                  className="btn btn-secondary btn-sm"
                  onClick={handlePickFile}
                  disabled={running}
                >
                  Browse
                </button>
              </div>
            </div>
          )}

          <div className="form-row" style={{ display: 'flex', gap: 16, alignItems: 'center' }}>
            <label
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 6,
                fontSize: 'var(--font-size-xs)',
              }}
            >
              <input
                type="checkbox"
                checked={withValues}
                onChange={(e) => setWithValues(e.target.checked)}
                disabled={running}
              />
              Pull values
            </label>
            <label
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 6,
                fontSize: 'var(--font-size-xs)',
              }}
            >
              <input
                type="checkbox"
                checked={persistSecrets}
                onChange={(e) => setPersistSecrets(e.target.checked)}
                disabled={running || !withValues}
              />
              Save secret values to keychain
            </label>
          </div>

          {report && apply && (
            <div className="bw-summary" style={{ marginTop: 12 }}>
              <span>
                <strong>{report.entries.length}</strong> entries
              </span>
              <span>
                vars added: <strong>{apply.variables_added}</strong>
              </span>
              <span>
                vars valued: <strong>{apply.variables_value_set}</strong>
              </span>
              <span>
                secret refs: <strong>{apply.secrets_ref_added}</strong>
              </span>
              <span>
                secrets saved: <strong>{apply.secrets_value_saved}</strong>
              </span>
            </div>
          )}

          {error && (
            <div className="bw-error">
              <AlertTriangle size={14} /> {error}
            </div>
          )}
        </div>

        <div className="modal-footer">
          <button className="btn btn-ghost" onClick={onClose} disabled={running}>
            Close
          </button>
          <button
            className="btn btn-primary"
            onClick={handleRun}
            disabled={
              running || (cliNeeded && cliOk === false) || (source === 'dotenv' && !sourcePath)
            }
          >
            {running ? (
              <>
                <Loader2 size={14} className="spin" /> Running…
              </>
            ) : (
              'Run import'
            )}
          </button>
        </div>
      </div>
    </div>
  );
}

export default ImporterModal;
