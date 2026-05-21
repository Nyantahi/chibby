import { useEffect, useState } from 'react';
import { CheckCircle2, Info, X, XCircle } from 'lucide-react';
import { dismissNotify, subscribeNotify, type NotifyEntry } from '../services/notify';

function Icon({ kind }: { kind: NotifyEntry['kind'] }) {
  if (kind === 'success') return <CheckCircle2 size={16} />;
  if (kind === 'error') return <XCircle size={16} />;
  return <Info size={16} />;
}

function Toaster() {
  const [entries, setEntries] = useState<NotifyEntry[]>([]);
  useEffect(() => subscribeNotify(setEntries), []);

  if (entries.length === 0) return null;

  return (
    <div className="toaster">
      {entries.map((e) => (
        <div key={e.id} className={`toast toast-${e.kind}`}>
          <span className="toast-icon">
            <Icon kind={e.kind} />
          </span>
          <div className="toast-body">
            <div className="toast-message">{e.message}</div>
            {e.detail && <div className="toast-detail">{e.detail}</div>}
          </div>
          <button
            type="button"
            className="toast-close"
            aria-label="Dismiss"
            onClick={() => dismissNotify(e.id)}
          >
            <X size={14} />
          </button>
        </div>
      ))}
    </div>
  );
}

export default Toaster;
