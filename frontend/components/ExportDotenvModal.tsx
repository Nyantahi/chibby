import { useState } from 'react';
import { X, AlertTriangle } from 'lucide-react';
import { save } from '@tauri-apps/plugin-dialog';
import { exportDotenv } from '../services/api';
import { notifyError, notifySuccess } from '../services/notify';
import type { Environment } from '../types';

interface Props {
  repoPath: string;
  environments: Environment[];
  onClose: () => void;
}

function ExportDotenvModal({ repoPath, environments, onClose }: Props) {
  const [envName, setEnvName] = useState<string>(environments[0]?.name ?? '');
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleExport() {
    setError(null);
    setRunning(true);
    try {
      const dest = await save({
        title: `Export ${envName} to .env`,
        defaultPath: `.env.${envName}`,
        filters: [{ name: 'env file', extensions: ['env', ''] }],
      });
      if (!dest) {
        setRunning(false);
        return;
      }
      const count = await exportDotenv(repoPath, envName, dest);
      notifySuccess('Exported .env', `${count} variables → ${dest}`);
      onClose();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg);
      notifyError('Export failed', err);
    } finally {
      setRunning(false);
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()} style={{ maxWidth: 480 }}>
        <div className="modal-header">
          <h3>Export to .env</h3>
          <button className="btn-icon" onClick={onClose} aria-label="Close">
            <X size={16} />
          </button>
        </div>
        <div className="modal-body">
          <p className="text-muted" style={{ fontSize: 'var(--font-size-xs)' }}>
            Resolves variables and secret values from the keychain and writes a flat .env file.
          </p>
          <div className="form-row">
            <label>Environment</label>
            <select
              className="input"
              value={envName}
              onChange={(e) => setEnvName(e.target.value)}
              disabled={running}
            >
              {environments.map((e) => (
                <option key={e.name} value={e.name}>
                  {e.name}
                </option>
              ))}
            </select>
          </div>
          {error && (
            <div className="bw-error">
              <AlertTriangle size={14} /> {error}
            </div>
          )}
        </div>
        <div className="modal-footer">
          <button className="btn btn-ghost" onClick={onClose} disabled={running}>
            Cancel
          </button>
          <button
            className="btn btn-primary"
            onClick={handleExport}
            disabled={running || !envName}
          >
            {running ? 'Exporting…' : 'Export'}
          </button>
        </div>
      </div>
    </div>
  );
}

export default ExportDotenvModal;
