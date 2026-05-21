import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { AlertTriangle, ArrowLeft, FolderOpen, Trash2 } from 'lucide-react';
import { clearCrashLog, getAppDataDir, getCrashLog } from '../services/api';
import { openPath } from '../services/openExternal';
import { notifyError, notifySuccess } from '../services/notify';

function CrashLog() {
  const navigate = useNavigate();
  const [content, setContent] = useState<string | null>(null);
  const [dataDir, setDataDir] = useState<string>('');
  const [loading, setLoading] = useState(true);
  const [clearing, setClearing] = useState(false);

  useEffect(() => {
    Promise.all([getCrashLog().catch(() => null), getAppDataDir().catch(() => '')])
      .then(([c, d]) => {
        setContent(c);
        setDataDir(d);
      })
      .finally(() => setLoading(false));
  }, []);

  async function handleClear() {
    if (!window.confirm('Clear the crash log?')) return;
    setClearing(true);
    try {
      await clearCrashLog();
      setContent(null);
      notifySuccess('Crash log cleared');
    } catch (err) {
      notifyError('Clear failed', err);
    } finally {
      setClearing(false);
    }
  }

  return (
    <div className="page-content">
      <button className="btn btn-ghost btn-sm" onClick={() => navigate(-1)}>
        <ArrowLeft size={14} /> Back
      </button>
      <h2 style={{ display: 'flex', gap: 8, alignItems: 'center', margin: '12px 0' }}>
        <AlertTriangle size={20} /> Crash log
      </h2>
      <p className="text-muted" style={{ fontSize: 'var(--font-size-xs)' }}>
        Path: <code>{dataDir ? `${dataDir}/crash.log` : 'crash.log'}</code>
      </p>

      <div style={{ display: 'flex', gap: 8, marginBottom: 12 }}>
        <button
          className="btn btn-sm btn-ghost"
          onClick={() => openPath(`${dataDir}/crash.log`)}
          disabled={!dataDir || !content}
        >
          <FolderOpen size={12} /> Reveal in Finder
        </button>
        <button
          className="btn btn-sm btn-danger-outline"
          onClick={handleClear}
          disabled={clearing || !content}
        >
          <Trash2 size={12} /> {clearing ? 'Clearing…' : 'Clear'}
        </button>
      </div>

      {loading ? (
        <p className="text-muted">Loading…</p>
      ) : !content ? (
        <p className="text-muted">No crash log present.</p>
      ) : (
        <pre
          style={{
            padding: 'var(--space-md)',
            border: '1px solid var(--color-border)',
            borderRadius: 'var(--radius-sm)',
            background: 'var(--color-surface)',
            fontSize: 'var(--font-size-xs)',
            whiteSpace: 'pre-wrap',
            maxHeight: '70vh',
            overflow: 'auto',
          }}
        >
          {content}
        </pre>
      )}
    </div>
  );
}

export default CrashLog;
