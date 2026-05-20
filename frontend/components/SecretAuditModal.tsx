import { useEffect, useState } from 'react';
import { X, Clock, Loader2 } from 'lucide-react';
import { getSecretAudit } from '../services/api';
import type { SecretAuditEntry } from '../types';

interface Props {
  repoPath: string;
  envName: string;
  secretName: string;
  onClose: () => void;
}

function SecretAuditModal({ repoPath, envName, secretName, onClose }: Props) {
  const [audit, setAudit] = useState<SecretAuditEntry | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    getSecretAudit(repoPath, envName, secretName)
      .then((a) => {
        if (!cancelled) setAudit(a);
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
  }, [repoPath, envName, secretName]);

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()} style={{ maxWidth: 480 }}>
        <div className="modal-header">
          <h3>
            <Clock size={16} style={{ marginRight: 6 }} />
            Audit: <code>{secretName}</code>
          </h3>
          <button className="btn-icon" onClick={onClose} aria-label="Close">
            <X size={16} />
          </button>
        </div>
        <div className="modal-body">
          <p className="text-muted" style={{ fontSize: 'var(--font-size-xs)' }}>
            Environment: <code>{envName}</code>
          </p>
          {loading ? (
            <div className="bw-loading">
              <Loader2 size={14} className="spin" /> Loading…
            </div>
          ) : error ? (
            <div className="bw-error">{error}</div>
          ) : !audit ? (
            <div className="feature-card-empty">No audit entries yet for this secret.</div>
          ) : (
            <table className="kv-table">
              <tbody>
                <tr>
                  <th>Last set</th>
                  <td>{audit.last_set ? new Date(audit.last_set).toLocaleString() : '—'}</td>
                </tr>
                <tr>
                  <th>Last deleted</th>
                  <td>
                    {audit.last_deleted ? new Date(audit.last_deleted).toLocaleString() : '—'}
                  </td>
                </tr>
                <tr>
                  <th>Set count</th>
                  <td>{audit.set_count}</td>
                </tr>
                <tr>
                  <th>Delete count</th>
                  <td>{audit.delete_count}</td>
                </tr>
                <tr>
                  <th>Last provenance</th>
                  <td>{audit.last_provenance ?? '—'}</td>
                </tr>
              </tbody>
            </table>
          )}
        </div>
        <div className="modal-footer">
          <button className="btn btn-ghost" onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}

export default SecretAuditModal;
