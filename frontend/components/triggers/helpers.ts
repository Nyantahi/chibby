import type {
  HookKind,
  HookSpec,
  HookState,
  MissedPolicy,
  ScheduleTrigger,
  WatchTrigger,
} from '../../types';

/** What to do about fire times that elapsed while nothing was running. */
export const MISSED_OPTIONS: { id: MissedPolicy; label: string; desc: string }[] = [
  {
    id: 'skip',
    label: 'Skip',
    desc: 'Forget fire times that elapsed while nothing was running. A laptop closed over the weekend simply misses those runs.',
  },
  {
    id: 'run_once',
    label: 'Run once',
    desc: 'Catch up with a single run when Chibby starts again — never one run per missed occurrence.',
  },
];

export const HOOK_KINDS: HookKind[] = ['pre_push', 'pre_commit'];

export const HOOK_LABELS: Record<HookKind, string> = {
  pre_push: 'pre-push',
  pre_commit: 'pre-commit',
};

export const HOOK_DESCRIPTIONS: Record<HookKind, string> = {
  pre_push: 'Runs before `git push` — the usual place for the slower checks.',
  pre_commit: 'Runs before every `git commit` — keep this one fast.',
};

export const HOOK_STATE_LABELS: Record<HookState, string> = {
  not_installed: 'Not installed',
  chibby_managed: 'Managed by Chibby',
  foreign: 'Your own hook',
  foreign_with_chibby_block: 'Your hook + Chibby block',
};

/** Badge class suffix per hook state. Nothing here is an error state. */
export const HOOK_STATE_BADGE: Record<HookState, string> = {
  not_installed: 'neutral',
  chibby_managed: 'success',
  foreign: 'warning',
  foreign_with_chibby_block: 'info',
};

/** Globs Chibby always ignores, whatever a watch's include patterns say. */
export const ALWAYS_EXCLUDED = ['.git/', '.chibby/'];

export function newSchedule(id: string): ScheduleTrigger {
  return { id, enabled: true, cron: '0 3 * * *', missed: 'skip', stages: [] };
}

export function newWatch(id: string): WatchTrigger {
  return {
    id,
    enabled: true,
    include: ['src/**'],
    exclude: [],
    debounce_ms: 750,
    min_interval_secs: 10,
    stages: [],
  };
}

export function newHookSpec(): HookSpec {
  return { stages: [], blocking: true };
}

/** Split a comma / newline separated field into trimmed, non-empty entries. */
export function parseList(value: string): string[] {
  return value
    .split(/[\n,]/)
    .map((s) => s.trim())
    .filter(Boolean);
}

/** Render a string list back into a comma-separated field value. */
export function formatList(items: string[]): string {
  return items.join(', ');
}

/** A trigger id not already taken by an existing trigger. */
export function uniqueId(base: string, taken: string[]): string {
  if (!taken.includes(base)) return base;
  let n = 2;
  while (taken.includes(`${base}-${n}`)) n += 1;
  return `${base}-${n}`;
}
