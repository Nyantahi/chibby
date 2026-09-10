import { CircleAlert } from 'lucide-react';
import { formatDate } from '../../utils/format';

interface CronPreviewProps {
  cron: string;
  /** Upcoming fire times from `useNextRuns`, owned by the parent editor. */
  times: string[];
  /** Backend cron parse error, shown inline instead of as a toast. */
  error: string | null;
}

/**
 * Live preview of the next few fire times for a cron expression — the only
 * practical way to tell whether the expression you typed means what you think.
 */
function CronPreview({ cron, times, error }: CronPreviewProps) {
  if (error) {
    return (
      <p className="cron-preview-error" role="alert">
        <CircleAlert size={12} /> {error}
      </p>
    );
  }

  if (!cron.trim()) {
    return <p className="cron-preview-hint">Type a cron expression to preview its fire times.</p>;
  }

  if (times.length === 0) {
    return <p className="cron-preview-hint">Checking…</p>;
  }

  return (
    <div className="cron-preview">
      <span className="cron-preview-label">Next {times.length} fire times</span>
      <ol className="cron-preview-list">
        {times.map((t) => (
          <li key={t}>{formatDate(t)}</li>
        ))}
      </ol>
    </div>
  );
}

export default CronPreview;
