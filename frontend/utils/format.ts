import type { PipelineRun, RunKind, RunStatus, StageStatus } from '../types';

/** Format a duration in milliseconds to a human-readable string. */
export function formatDuration(ms?: number): string {
  if (ms === undefined || ms === null) return '--';
  if (ms < 1000) return `${ms}ms`;
  const seconds = Math.floor(ms / 1000);
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  return `${minutes}m ${remainingSeconds}s`;
}

/** Format an ISO date string to a localized short form. */
export function formatDate(iso?: string): string {
  if (!iso) return '--';
  const date = new Date(iso);
  return date.toLocaleString(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

/**
 * Whether a stage/run status counts as a failure. `timedout` is a failure
 * variant — mirrors `StageStatus::is_failure` on the backend.
 */
export function isFailureStatus(status?: RunStatus | StageStatus): boolean {
  return status === 'failed' || status === 'timedout';
}

/** Get the CSS class suffix for a run status. */
export function statusClass(status: RunStatus | StageStatus): string {
  switch (status) {
    case 'success':
      return 'success';
    case 'failed':
    case 'timedout':
      return 'failed';
    case 'running':
      return 'running';
    case 'pending':
      return 'pending';
    case 'skipped':
      return 'skipped';
    case 'cancelled':
      return 'cancelled';
    default:
      return 'pending';
  }
}

/** Human-readable label for a run or stage status. */
export function statusLabel(status: RunStatus | StageStatus): string {
  return status === 'timedout' ? 'Timed out' : capitalize(status);
}

/**
 * Whether a rollback run was started by the engine rather than a person.
 * Mirrors the CLI's `[auto-rollback]` vs `[rollback]` history marker.
 */
export function isAutoRollback(run: Pick<PipelineRun, 'run_kind' | 'auto_rollback_of'>): boolean {
  return run.run_kind === 'rollback' && run.auto_rollback_of != null;
}

/** History label for a run's kind. `auto` separates engine rollbacks from manual ones. */
export function runKindLabel(kind: RunKind | undefined, auto = false): string {
  switch (kind) {
    case 'rollback':
      return auto ? 'Auto-rollback' : 'Rollback';
    case 'retry':
      return 'Retry';
    case 'scheduled':
      return 'Scheduled';
    case 'watch':
      return 'Watch';
    case 'hook':
      return 'Hook';
    default:
      return 'Normal';
  }
}

/**
 * Whether a run happened with nobody watching. Mirrors `RunKind::is_unattended`
 * on the backend — a hook run is excluded because someone typed `git push`.
 */
export function isUnattendedKind(kind: RunKind | undefined): boolean {
  return kind === 'scheduled' || kind === 'watch';
}

/** Capitalize the first character of a string. */
export function capitalize(s: string): string {
  return s.charAt(0).toUpperCase() + s.slice(1);
}

/** Extract the repo name from a full path. */
export function repoNameFromPath(path: string): string {
  const parts = path.replace(/\\/g, '/').split('/');
  return parts[parts.length - 1] || path;
}
