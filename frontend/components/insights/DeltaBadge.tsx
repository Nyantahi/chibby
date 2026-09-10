import { ArrowDown, ArrowUp, Minus } from 'lucide-react';
import {
  deltaHint,
  deltaTone,
  formatDeltaPct,
  formatDeltaPoints,
  NO_VALUE,
} from '../../utils/insights';

interface DeltaBadgeProps {
  /** `null` means the previous window had no runs — rendered as a dash. */
  value: number | null;
  /** Percentage points (a rate delta) or percent change (a duration delta). */
  unit: 'points' | 'percent';
  /** Duration inverts the sign: slower is a regression, so up reads red. */
  lowerIsBetter?: boolean;
}

/**
 * A single trend delta. The one thing it must never do is turn a missing
 * baseline into `+0.0` — a `null` value renders `—` and carries no colour.
 */
function DeltaBadge({ value, unit, lowerIsBetter = false }: DeltaBadgeProps) {
  const known = value != null && Number.isFinite(value);
  const tone = deltaTone(value, lowerIsBetter);
  const text = known
    ? unit === 'points'
      ? formatDeltaPoints(value)
      : formatDeltaPct(value)
    : NO_VALUE;
  const Icon = !known || value === 0 ? Minus : (value as number) > 0 ? ArrowUp : ArrowDown;

  return (
    <span
      className={`insights-delta insights-delta-${tone}`}
      title={deltaHint(value, lowerIsBetter)}
    >
      <Icon size={12} aria-hidden="true" />
      <span>{text}</span>
    </span>
  );
}

export default DeltaBadge;
