import { useState } from 'react';
import { Check, X } from 'lucide-react';
import type { WatchTrigger } from '../../types';
import { ALWAYS_EXCLUDED, formatList, parseList } from './helpers';

interface WatchEditorProps {
  initial: WatchTrigger;
  environments: string[];
  takenIds: string[];
  onSave: (watch: WatchTrigger) => void;
  onCancel: () => void;
}

/** Guard rails matching the backend defaults — a zero debounce would hot-loop. */
const MIN_DEBOUNCE_MS = 0;
const MIN_INTERVAL_SECS = 0;

function WatchEditor({ initial, environments, takenIds, onSave, onCancel }: WatchEditorProps) {
  const [draft, setDraft] = useState<WatchTrigger>(initial);
  const [includeText, setIncludeText] = useState(formatList(initial.include));
  const [excludeText, setExcludeText] = useState(formatList(initial.exclude));
  const [stagesText, setStagesText] = useState(formatList(initial.stages));

  const id = draft.id.trim();
  const duplicateId = id.length > 0 && takenIds.includes(id);
  const canSave = id.length > 0 && !duplicateId;

  function handleSave() {
    if (!canSave) return;
    onSave({
      ...draft,
      id,
      include: parseList(includeText),
      exclude: parseList(excludeText),
      stages: parseList(stagesText),
    });
  }

  return (
    <div className="trigger-editor">
      <div className="trigger-editor-grid">
        <div className="form-group">
          <label className="form-label" htmlFor="watch-id">
            Name
          </label>
          <input
            id="watch-id"
            className="input input-sm"
            value={draft.id}
            onChange={(e) => setDraft({ ...draft, id: e.target.value })}
            placeholder="on-src-change"
          />
          {duplicateId && (
            <p className="cron-preview-error">Another watch already uses this name.</p>
          )}
        </div>

        <div className="form-group">
          <label className="form-label" htmlFor="watch-include">
            Include globs
          </label>
          <input
            id="watch-include"
            className="input input-sm mono"
            value={includeText}
            onChange={(e) => setIncludeText(e.target.value)}
            placeholder="src/**, package.json"
          />
          <p className="form-hint">Relative to the repo root. Empty watches everything.</p>
        </div>

        <div className="form-group">
          <label className="form-label" htmlFor="watch-exclude">
            Exclude globs
          </label>
          <input
            id="watch-exclude"
            className="input input-sm mono"
            value={excludeText}
            onChange={(e) => setExcludeText(e.target.value)}
            placeholder="**/*.snap"
          />
          <p className="form-hint">
            {ALWAYS_EXCLUDED.join(' and ')} are always excluded, whatever you put here.
          </p>
        </div>

        <div className="form-group">
          <label className="form-label" htmlFor="watch-debounce">
            Debounce (ms)
          </label>
          <input
            id="watch-debounce"
            type="number"
            className="input input-sm"
            min={MIN_DEBOUNCE_MS}
            value={draft.debounce_ms}
            onChange={(e) =>
              setDraft({ ...draft, debounce_ms: Math.max(MIN_DEBOUNCE_MS, +e.target.value || 0) })
            }
          />
          <p className="form-hint">Quiet period before a burst of saves fires one run.</p>
        </div>

        <div className="form-group">
          <label className="form-label" htmlFor="watch-interval">
            Minimum interval (seconds)
          </label>
          <input
            id="watch-interval"
            type="number"
            className="input input-sm"
            min={MIN_INTERVAL_SECS}
            value={draft.min_interval_secs}
            onChange={(e) =>
              setDraft({
                ...draft,
                min_interval_secs: Math.max(MIN_INTERVAL_SECS, +e.target.value || 0),
              })
            }
          />
          <p className="form-hint">
            Floor between two runs, so a build that writes into the repo cannot hot-loop.
          </p>
        </div>

        <div className="form-group">
          <label className="form-label" htmlFor="watch-env">
            Environment
          </label>
          <select
            id="watch-env"
            className="input input-sm"
            value={draft.environment ?? ''}
            onChange={(e) => setDraft({ ...draft, environment: e.target.value || undefined })}
          >
            <option value="">None</option>
            {environments.map((env) => (
              <option key={env} value={env}>
                {env}
              </option>
            ))}
          </select>
        </div>

        <div className="form-group">
          <label className="form-label" htmlFor="watch-pipeline">
            Pipeline file
          </label>
          <input
            id="watch-pipeline"
            className="input input-sm"
            value={draft.pipeline_file ?? ''}
            onChange={(e) => setDraft({ ...draft, pipeline_file: e.target.value || undefined })}
            placeholder="pipeline (default)"
          />
        </div>

        <div className="form-group">
          <label className="form-label" htmlFor="watch-stages">
            Stages
          </label>
          <input
            id="watch-stages"
            className="input input-sm"
            value={stagesText}
            onChange={(e) => setStagesText(e.target.value)}
            placeholder="test — empty runs every stage"
          />
        </div>
      </div>

      <div className="trigger-editor-actions">
        <button className="btn btn-sm btn-primary" onClick={handleSave} disabled={!canSave}>
          <Check size={14} /> Done
        </button>
        <button className="btn btn-sm btn-ghost" onClick={onCancel}>
          <X size={14} /> Cancel
        </button>
      </div>
    </div>
  );
}

export default WatchEditor;
