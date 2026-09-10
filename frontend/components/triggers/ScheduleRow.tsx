import { Link } from 'react-router-dom';
import { CalendarClock, Pencil, Play, SkipForward, Trash2 } from 'lucide-react';
import type { ScheduleTrigger, TriggerStateEntry } from '../../types';
import { useNextRuns } from '../../hooks/useNextRuns';
import { formatDate } from '../../utils/format';
import { MISSED_OPTIONS } from './helpers';

interface ScheduleRowProps {
  schedule: ScheduleTrigger;
  state?: TriggerStateEntry;
  firing: boolean;
  onToggle: (enabled: boolean) => void;
  onEdit: () => void;
  onDelete: () => void;
  onRunNow: () => void;
}

function ScheduleRow({
  schedule,
  state,
  firing,
  onToggle,
  onEdit,
  onDelete,
  onRunNow,
}: ScheduleRowProps) {
  const { times, error } = useNextRuns(schedule.cron, 1);
  const missed = MISSED_OPTIONS.find((o) => o.id === schedule.missed);

  return (
    <div className={`trigger-row ${schedule.enabled ? '' : 'trigger-row-disabled'}`}>
      <div className="trigger-row-head">
        <label className="trigger-switch" title={schedule.enabled ? 'Disable' : 'Enable'}>
          <input
            type="checkbox"
            checked={schedule.enabled}
            onChange={(e) => onToggle(e.target.checked)}
            aria-label={`Enable schedule ${schedule.id}`}
          />
        </label>
        <span className="trigger-name">{schedule.id}</span>
        <code className="trigger-cron">{schedule.cron}</code>
        {schedule.environment && (
          <span className="badge badge-neutral">{schedule.environment}</span>
        )}
        {schedule.stages.length > 0 && (
          <span className="badge badge-neutral" title={schedule.stages.join(', ')}>
            {schedule.stages.length} stage{schedule.stages.length === 1 ? '' : 's'}
          </span>
        )}
        {missed && (
          <span className="badge badge-neutral" title={missed.desc}>
            Missed: {missed.label}
          </span>
        )}
        <div className="trigger-row-actions">
          <button
            className="btn btn-xs btn-ghost"
            onClick={onRunNow}
            disabled={firing}
            title="Run this trigger's pipeline right now"
          >
            <Play size={12} /> {firing ? 'Running…' : 'Run now'}
          </button>
          <button className="btn btn-xs btn-ghost" onClick={onEdit} title="Edit schedule">
            <Pencil size={12} />
          </button>
          <button className="btn btn-xs btn-danger-icon" onClick={onDelete} title="Delete schedule">
            <Trash2 size={12} />
          </button>
        </div>
      </div>

      <div className="trigger-row-meta">
        {error ? (
          <span className="cron-preview-error">
            Invalid cron — this schedule will never fire. {error}
          </span>
        ) : (
          <span className="trigger-meta-item">
            <CalendarClock size={12} /> Next: {times[0] ? formatDate(times[0]) : '…'}
          </span>
        )}
        <span className="trigger-meta-item">Last fired: {formatDate(state?.last_fired_at)}</span>
        {state?.last_run_id && (
          <Link to={`/run/${state.last_run_id}`} className="text-link">
            View last run
          </Link>
        )}
        {state?.last_skip_reason && (
          <span className="trigger-meta-item" title={state.last_skip_reason}>
            <SkipForward size={12} /> Skipped: {state.last_skip_reason}
          </span>
        )}
      </div>
    </div>
  );
}

export default ScheduleRow;
