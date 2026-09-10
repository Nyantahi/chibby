import { useState } from 'react';
import { Check, X } from 'lucide-react';
import type { MissedPolicy, ScheduleTrigger } from '../../types';
import { useNextRuns } from '../../hooks/useNextRuns';
import CronPreview from './CronPreview';
import { MISSED_OPTIONS, formatList, parseList } from './helpers';

interface ScheduleEditorProps {
  initial: ScheduleTrigger;
  environments: string[];
  /** Ids already in use by other schedules, so we can refuse a duplicate. */
  takenIds: string[];
  onSave: (schedule: ScheduleTrigger) => void;
  onCancel: () => void;
}

function ScheduleEditor({
  initial,
  environments,
  takenIds,
  onSave,
  onCancel,
}: ScheduleEditorProps) {
  const [draft, setDraft] = useState<ScheduleTrigger>(initial);
  const [stagesText, setStagesText] = useState(formatList(initial.stages));
  const { times, error: cronError } = useNextRuns(draft.cron, 5);

  const id = draft.id.trim();
  const duplicateId = id.length > 0 && takenIds.includes(id);
  const canSave = id.length > 0 && !duplicateId && draft.cron.trim().length > 0 && !cronError;

  function handleSave() {
    if (!canSave) return;
    onSave({ ...draft, id, cron: draft.cron.trim(), stages: parseList(stagesText) });
  }

  return (
    <div className="trigger-editor">
      <div className="trigger-editor-grid">
        <div className="form-group">
          <label className="form-label" htmlFor="schedule-id">
            Name
          </label>
          <input
            id="schedule-id"
            className="input input-sm"
            value={draft.id}
            onChange={(e) => setDraft({ ...draft, id: e.target.value })}
            placeholder="nightly"
          />
          {duplicateId && (
            <p className="cron-preview-error">Another schedule already uses this name.</p>
          )}
        </div>

        <div className="form-group">
          <label className="form-label" htmlFor="schedule-cron">
            Cron expression
          </label>
          <input
            id="schedule-cron"
            className="input input-sm mono"
            value={draft.cron}
            onChange={(e) => setDraft({ ...draft, cron: e.target.value })}
            placeholder="0 3 * * *"
          />
          <p className="form-hint">
            Five fields (<code>0 3 * * *</code>) or six with leading seconds (
            <code>0 30 4 * * *</code>). Times are local to this machine.
          </p>
        </div>

        <div className="form-group">
          <label className="form-label" htmlFor="schedule-missed">
            If the machine was asleep
          </label>
          <select
            id="schedule-missed"
            className="input input-sm"
            value={draft.missed}
            onChange={(e) => setDraft({ ...draft, missed: e.target.value as MissedPolicy })}
          >
            {MISSED_OPTIONS.map((o) => (
              <option key={o.id} value={o.id}>
                {o.label}
              </option>
            ))}
          </select>
          <p className="form-hint">{MISSED_OPTIONS.find((o) => o.id === draft.missed)?.desc}</p>
        </div>

        <div className="form-group">
          <label className="form-label" htmlFor="schedule-env">
            Environment
          </label>
          <select
            id="schedule-env"
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
          <label className="form-label" htmlFor="schedule-pipeline">
            Pipeline file
          </label>
          <input
            id="schedule-pipeline"
            className="input input-sm"
            value={draft.pipeline_file ?? ''}
            onChange={(e) => setDraft({ ...draft, pipeline_file: e.target.value || undefined })}
            placeholder="pipeline (default)"
          />
        </div>

        <div className="form-group">
          <label className="form-label" htmlFor="schedule-stages">
            Stages
          </label>
          <input
            id="schedule-stages"
            className="input input-sm"
            value={stagesText}
            onChange={(e) => setStagesText(e.target.value)}
            placeholder="build, test — empty runs every stage"
          />
        </div>
      </div>

      <CronPreview cron={draft.cron} times={times} error={cronError} />

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

export default ScheduleEditor;
