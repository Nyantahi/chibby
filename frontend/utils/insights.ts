import type { RollbackOutcome } from '../types';
import { formatDuration } from './format';

/**
 * Formatting helpers for the Insights view.
 *
 * The rule that drives most of this file: a `null` delta means the previous
 * window had no runs, so there is no baseline. It must render as a dash, never
 * as `+0.0` — a brand-new project's first week would otherwise claim an
 * improvement that never happened.
 */

/** Placeholder for "no value" and, crucially, for "no baseline". */
export const NO_VALUE = '—';

/** Whether a delta reads as better, worse, or neither. */
export type DeltaTone = 'success' | 'failed' | 'neutral';

/** Explains a dashed delta wherever one is rendered. */
export const NO_BASELINE_HINT = 'No baseline — the previous window had no runs';

/** A rate in 0..1 as a percentage. Guards against `NaN` from an empty window. */
export function formatRate(rate: number | null | undefined): string {
  if (rate == null || !Number.isFinite(rate)) return NO_VALUE;
  return `${(rate * 100).toFixed(1)}%`;
}

/** A signed count of percentage *points*. `null` means no baseline. */
export function formatDeltaPoints(delta: number | null | undefined): string {
  if (delta == null || !Number.isFinite(delta)) return NO_VALUE;
  return `${signOf(delta)}${Math.abs(delta).toFixed(1)} pts`;
}

/** A signed percent change. `null` means no baseline. */
export function formatDeltaPct(delta: number | null | undefined): string {
  if (delta == null || !Number.isFinite(delta)) return NO_VALUE;
  return `${signOf(delta)}${Math.abs(delta).toFixed(1)}%`;
}

/**
 * Colour a delta by whether it is an improvement.
 *
 * `lowerIsBetter` inverts the sign convention: a duration that went *up* is a
 * regression, so a positive duration delta must read as a failure, not as the
 * green that a rising success rate earns.
 */
export function deltaTone(delta: number | null | undefined, lowerIsBetter = false): DeltaTone {
  if (delta == null || !Number.isFinite(delta) || delta === 0) return 'neutral';
  const improved = lowerIsBetter ? delta < 0 : delta > 0;
  return improved ? 'success' : 'failed';
}

/** Screen-reader/tooltip wording for a delta, including the no-baseline case. */
export function deltaHint(delta: number | null | undefined, lowerIsBetter = false): string {
  if (delta == null || !Number.isFinite(delta)) return NO_BASELINE_HINT;
  if (delta === 0) return 'Unchanged from the previous window';
  const improved = lowerIsBetter ? delta < 0 : delta > 0;
  return improved ? 'Better than the previous window' : 'Worse than the previous window';
}

/** A nullable duration. A missing one is a dash, never `0ms` and never `NaN`. */
export function formatMs(ms: number | null | undefined): string {
  return ms == null ? NO_VALUE : formatDuration(ms);
}

/** On-disk size in human-readable units. */
export function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null || !Number.isFinite(bytes) || bytes < 0) return NO_VALUE;
  if (bytes < 1024) return `${bytes} B`;
  const units = ['KB', 'MB', 'GB'];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(1)} ${units[unit]}`;
}

/**
 * A `YYYY-MM-DD` index date as a short local label. Parsed part-by-part so a
 * UTC-midnight string does not slide to the previous day in western zones.
 */
export function formatDay(date: string): string {
  const [year, month, day] = date.split('-').map(Number);
  if (!year || !month || !day) return date;
  return new Date(year, month - 1, day).toLocaleDateString(undefined, {
    weekday: 'short',
    month: 'short',
    day: 'numeric',
  });
}

/** First segment of a run UUID — enough to recognise, short enough for a cell. */
export function shortRunId(id: string): string {
  return id.slice(0, 8);
}

/** Commits are shown at git's usual short length. */
export function shortCommit(commit: string | null | undefined): string {
  return commit ? commit.slice(0, 7) : NO_VALUE;
}

/** Badge class suffix for how a rollback turned out. */
export function rollbackClass(outcome: RollbackOutcome): string {
  if (outcome === 'succeeded') return 'success';
  if (outcome === 'failed') return 'failed';
  return 'neutral';
}

function signOf(delta: number): string {
  return delta > 0 ? '+' : delta < 0 ? '-' : '';
}
