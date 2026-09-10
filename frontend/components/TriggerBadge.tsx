import { CalendarClock, Eye, GitFork } from 'lucide-react';
import type { RunKind } from '../types';
import { runKindLabel } from '../utils/format';

/** The three trigger-fired run kinds get an icon; everything else renders nothing. */
const TRIGGER_ICONS = {
  scheduled: CalendarClock,
  watch: Eye,
  hook: GitFork,
} as const;

interface TriggerBadgeProps {
  runKind: RunKind | undefined;
  /** The trigger id that fired the run, shown next to the badge when there is room. */
  triggerId?: string;
  size?: number;
}

/**
 * Provenance badge for a run that a trigger started rather than a person.
 * Renders nothing for normal / retry / rollback runs, which have their own badges.
 */
function TriggerBadge({ runKind, triggerId, size = 10 }: TriggerBadgeProps) {
  if (runKind !== 'scheduled' && runKind !== 'watch' && runKind !== 'hook') return null;

  const Icon = TRIGGER_ICONS[runKind];
  const label = runKindLabel(runKind);
  const title = triggerId ? `${label} run — trigger "${triggerId}"` : `${label} run`;

  return (
    <span className={`badge badge-trigger badge-trigger-${runKind}`} title={title}>
      <Icon size={size} />
      {label}
      {triggerId && <span className="badge-trigger-id">{triggerId}</span>}
    </span>
  );
}

export default TriggerBadge;
