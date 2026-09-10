import { useState } from 'react';
import { Link } from 'react-router-dom';
import { Eye, Pencil, Play, Plus, Timer, Trash2 } from 'lucide-react';
import type { TriggerStateEntry, WatchTrigger } from '../../types';
import { formatDate } from '../../utils/format';
import WatchEditor from './WatchEditor';
import { ALWAYS_EXCLUDED, newWatch, uniqueId } from './helpers';

/** Sentinel editing key for the not-yet-added watch. */
const NEW = ' new';

interface WatchesSectionProps {
  watches: WatchTrigger[];
  triggerState: Record<string, TriggerStateEntry>;
  environments: string[];
  firingId: string | null;
  onChange: (watches: WatchTrigger[]) => void;
  onRunNow: (triggerId: string) => void;
}

function WatchesSection({
  watches,
  triggerState,
  environments,
  firingId,
  onChange,
  onRunNow,
}: WatchesSectionProps) {
  const [editing, setEditing] = useState<string | null>(null);

  const ids = watches.map((w) => w.id);
  const draft = editing === NEW ? newWatch(uniqueId('on-change', ids)) : null;

  function handleSave(originalId: string | null, next: WatchTrigger) {
    onChange(
      originalId === null
        ? [...watches, next]
        : watches.map((w) => (w.id === originalId ? next : w))
    );
    setEditing(null);
  }

  return (
    <section className="section">
      <div className="section-header-row">
        <h3 className="section-title">
          <Eye size={16} />
          File watches
          <span className="badge badge-neutral">{watches.length}</span>
        </h3>
        <button
          className="btn btn-sm btn-secondary"
          onClick={() => setEditing(NEW)}
          disabled={editing === NEW}
        >
          <Plus size={14} /> Add watch
        </button>
      </div>

      <p className="section-hint">
        {ALWAYS_EXCLUDED.join(' and ')} are always excluded, so a run that writes into the repo
        cannot re-trigger itself.
      </p>

      {watches.length === 0 && editing !== NEW && (
        <div className="empty-state-small">
          <p>No watches yet. Add one to run stages whenever matching files change.</p>
        </div>
      )}

      <div className="trigger-list">
        {watches.map((watch) => {
          const state = triggerState[watch.id];
          if (editing === watch.id) {
            return (
              <WatchEditor
                key={watch.id}
                initial={watch}
                environments={environments}
                takenIds={ids.filter((id) => id !== watch.id)}
                onSave={(next) => handleSave(watch.id, next)}
                onCancel={() => setEditing(null)}
              />
            );
          }
          return (
            <div
              key={watch.id}
              className={`trigger-row ${watch.enabled ? '' : 'trigger-row-disabled'}`}
            >
              <div className="trigger-row-head">
                <label className="trigger-switch" title={watch.enabled ? 'Disable' : 'Enable'}>
                  <input
                    type="checkbox"
                    checked={watch.enabled}
                    aria-label={`Enable watch ${watch.id}`}
                    onChange={(e) =>
                      onChange(
                        watches.map((w) =>
                          w.id === watch.id ? { ...w, enabled: e.target.checked } : w
                        )
                      )
                    }
                  />
                </label>
                <span className="trigger-name">{watch.id}</span>
                <code className="trigger-cron">{watch.include.join(', ') || 'everything'}</code>
                {watch.exclude.length > 0 && (
                  <span className="badge badge-neutral" title={watch.exclude.join(', ')}>
                    {watch.exclude.length} excluded
                  </span>
                )}
                {watch.environment && (
                  <span className="badge badge-neutral">{watch.environment}</span>
                )}
                <div className="trigger-row-actions">
                  <button
                    className="btn btn-xs btn-ghost"
                    onClick={() => onRunNow(watch.id)}
                    disabled={firingId === watch.id}
                    title="Run this trigger's pipeline right now"
                  >
                    <Play size={12} /> {firingId === watch.id ? 'Running…' : 'Run now'}
                  </button>
                  <button
                    className="btn btn-xs btn-ghost"
                    onClick={() => setEditing(watch.id)}
                    title="Edit watch"
                  >
                    <Pencil size={12} />
                  </button>
                  <button
                    className="btn btn-xs btn-danger-icon"
                    onClick={() => onChange(watches.filter((w) => w.id !== watch.id))}
                    title="Delete watch"
                  >
                    <Trash2 size={12} />
                  </button>
                </div>
              </div>
              <div className="trigger-row-meta">
                <span className="trigger-meta-item">
                  <Timer size={12} /> Debounce {watch.debounce_ms}ms · min {watch.min_interval_secs}
                  s between runs
                </span>
                <span className="trigger-meta-item">
                  Last fired: {formatDate(state?.last_fired_at)}
                </span>
                {state?.last_run_id && (
                  <Link to={`/run/${state.last_run_id}`} className="text-link">
                    View last run
                  </Link>
                )}
                {state?.last_skip_reason && (
                  <span className="trigger-meta-item">Skipped: {state.last_skip_reason}</span>
                )}
              </div>
            </div>
          );
        })}

        {draft && (
          <WatchEditor
            initial={draft}
            environments={environments}
            takenIds={ids}
            onSave={(next) => handleSave(null, next)}
            onCancel={() => setEditing(null)}
          />
        )}
      </div>
    </section>
  );
}

export default WatchesSection;
