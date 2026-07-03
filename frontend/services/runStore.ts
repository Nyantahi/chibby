import { useSyncExternalStore } from 'react';
import { listen } from '@tauri-apps/api/event';
import type { Pipeline, PipelineRun } from '../types';
import { runPipeline } from './api';

/**
 * Global store of in-flight (and just-finished) pipeline runs, keyed by repo
 * path. Lets multiple projects run concurrently and keeps live progress alive
 * across route navigation. Mirrors the module-store pattern in `notify.ts`.
 *
 * Durable run history stays on disk (`runs/{id}.json`); this store only holds
 * transient live state, demultiplexed from the global `pipeline:log` event.
 */

export type RunLiveStatus = 'running' | 'success' | 'failed' | 'cancelled';
export type StageStatus = 'pending' | 'running' | 'success' | 'failed' | 'skipped';
export type CmdStatus = 'pending' | 'running' | 'done' | 'failed';

/** Payload emitted by the backend on the `pipeline:log` event. */
export interface LogPayload {
  run_id: string;
  repo_path: string;
  stage: string;
  type: string;
  message: string;
}

export interface ActiveRun {
  repoPath: string;
  projectId?: string;
  projectName?: string;
  runId?: string;
  status: RunLiveStatus;
  stageStatuses: Record<string, StageStatus>;
  /** "stageName:cmdIndex" -> status */
  cmdStatuses: Record<string, CmdStatus>;
  /** stageName -> last N live output lines */
  liveOutput: Record<string, string[]>;
  runningStageName: string | null;
  error?: string;
  startedAt: number;
  environment?: string;
  pipelineFile?: string;
  stagesToRun: string[];
  /** The final run result once `runPipeline` resolves. */
  finishedRun?: PipelineRun;
  // --- internal reducer bookkeeping ---
  /** stageName -> next command index to mark running */
  stageCmdIdx: Record<string, number>;
  /** stageName -> command count (for finalizing on stage transitions) */
  stageCommands: Record<string, number>;
}

const MAX_LIVE_LINES = 50;

// eslint-disable-next-line no-control-regex
const ANSI_RE = /\x1b\[[0-9;]*[A-Za-z]|\x1b\].*?\x07/g;
function stripAnsi(text: string): string {
  return text.replace(ANSI_RE, '');
}

// --- store internals ---------------------------------------------------------

const runs = new Map<string, ActiveRun>();
let runsList: ActiveRun[] = [];
const listeners = new Set<() => void>();

function emit(): void {
  runsList = Array.from(runs.values());
  for (const fn of listeners) fn();
}

function subscribe(cb: () => void): () => void {
  listeners.add(cb);
  return () => {
    listeners.delete(cb);
  };
}

function update(repoPath: string, fn: (r: ActiveRun) => ActiveRun): void {
  const cur = runs.get(repoPath);
  if (!cur) return;
  runs.set(repoPath, fn(cur));
  emit();
}

// --- public read hooks -------------------------------------------------------

export function useActiveRun(repoPath: string | undefined): ActiveRun | undefined {
  return useSyncExternalStore(subscribe, () => (repoPath ? runs.get(repoPath) : undefined));
}

export function useActiveRuns(): ActiveRun[] {
  return useSyncExternalStore(subscribe, () => runsList);
}

/** Non-hook read (e.g. to guard a second run of the same repo). */
export function getActiveRun(repoPath: string): ActiveRun | undefined {
  return runs.get(repoPath);
}

export function isRepoRunning(repoPath: string): boolean {
  return runs.get(repoPath)?.status === 'running';
}

// --- reducer -----------------------------------------------------------------

/** Fold a single `pipeline:log` line into the run's live state. */
export function applyLogLine(repoPath: string, payload: LogPayload): void {
  update(repoPath, (cur) => {
    const { stage, type: logType, message } = payload;
    const stageStatuses = { ...cur.stageStatuses };
    const cmdStatuses = { ...cur.cmdStatuses };
    const liveOutput = { ...cur.liveOutput };
    const stageCmdIdx = { ...cur.stageCmdIdx };
    let runningStageName = cur.runningStageName;

    if (logType === 'info' && message.startsWith('--- Starting stage:')) {
      runningStageName = stage;
      stageStatuses[stage] = 'running';
      // Any stage still marked running has completed — finalize it.
      for (const key of Object.keys(stageStatuses)) {
        if (key !== stage && stageStatuses[key] === 'running') {
          stageStatuses[key] = 'success';
          const cmdCount = cur.stageCommands[key] ?? 0;
          for (let ci = 0; ci < cmdCount; ci++) {
            const cmdKey = `${key}:${ci}`;
            if (cmdStatuses[cmdKey] === 'running' || cmdStatuses[cmdKey] === 'pending') {
              cmdStatuses[cmdKey] = 'done';
            }
          }
        }
      }
      stageCmdIdx[stage] = 0;
    } else if (logType === 'cmd') {
      const idx = stageCmdIdx[stage] ?? 0;
      if (idx > 0) cmdStatuses[`${stage}:${idx - 1}`] = 'done';
      cmdStatuses[`${stage}:${idx}`] = 'running';
      stageCmdIdx[stage] = idx + 1;
    } else if (logType === 'error') {
      const idx = (stageCmdIdx[stage] ?? 1) - 1;
      cmdStatuses[`${stage}:${idx}`] = 'failed';
    } else if (logType === 'stdout' || logType === 'stderr') {
      const clean = stripAnsi(message);
      const currentLines = liveOutput[stage] || [];
      liveOutput[stage] = [...currentLines, clean].slice(-MAX_LIVE_LINES);
    }

    return { ...cur, stageStatuses, cmdStatuses, liveOutput, stageCmdIdx, runningStageName };
  });
}

// --- lifecycle ---------------------------------------------------------------

export interface StartRunOptions {
  repoPath: string;
  pipeline: Pipeline;
  projectId?: string;
  projectName?: string;
  environment?: string;
  stages?: string[];
  pipelineFile?: string;
}

/**
 * Seed a run's live state and kick off execution fire-and-forget. Returns the
 * `runPipeline` promise so callers can await completion if they want, but the
 * store updates regardless of whether anyone is listening.
 */
export function startRun(opts: StartRunOptions): Promise<PipelineRun> {
  const { repoPath, pipeline, projectId, projectName, environment, stages, pipelineFile } = opts;
  const stagesToRun = stages ?? pipeline.stages.map((s) => s.name);

  const stageStatuses: Record<string, StageStatus> = {};
  const cmdStatuses: Record<string, CmdStatus> = {};
  const stageCommands: Record<string, number> = {};
  for (const s of pipeline.stages) {
    stageCommands[s.name] = s.commands.length;
    if (stagesToRun.includes(s.name)) {
      stageStatuses[s.name] = 'pending';
      s.commands.forEach((_, ci) => {
        cmdStatuses[`${s.name}:${ci}`] = 'pending';
      });
    }
  }

  runs.set(repoPath, {
    repoPath,
    projectId,
    projectName,
    status: 'running',
    stageStatuses,
    cmdStatuses,
    liveOutput: {},
    runningStageName: null,
    startedAt: Date.now(),
    environment,
    pipelineFile,
    stagesToRun,
    stageCmdIdx: {},
    stageCommands,
  });
  emit();

  const promise = runPipeline(repoPath, environment, stages, pipelineFile);
  promise.then((run) => finalizeRun(repoPath, run)).catch((err) => failRun(repoPath, err));
  return promise;
}

/** Reconcile final stage/command statuses once the run resolves. */
function finalizeRun(repoPath: string, run: PipelineRun): void {
  update(repoPath, (cur) => {
    const stageStatuses = { ...cur.stageStatuses };
    const cmdStatuses = { ...cur.cmdStatuses };
    run.stage_results?.forEach((result) => {
      stageStatuses[result.stage_name] = result.status as StageStatus;
      const cmdCount = cur.stageCommands[result.stage_name] ?? 0;
      for (let ci = 0; ci < cmdCount; ci++) {
        const key = `${result.stage_name}:${ci}`;
        if (result.status === 'success') cmdStatuses[key] = 'done';
        else if (result.status === 'skipped') cmdStatuses[key] = 'pending';
        // 'failed' keeps its status from the event stream
      }
    });
    const status: RunLiveStatus =
      run.status === 'success' ? 'success' : run.status === 'cancelled' ? 'cancelled' : 'failed';
    return {
      ...cur,
      runId: run.id,
      status,
      runningStageName: null,
      stageStatuses,
      cmdStatuses,
      finishedRun: run,
    };
  });
}

function failRun(repoPath: string, err: unknown): void {
  update(repoPath, (cur) => {
    const stageStatuses = { ...cur.stageStatuses };
    if (cur.runningStageName) stageStatuses[cur.runningStageName] = 'failed';
    return { ...cur, status: 'failed', error: String(err), runningStageName: null, stageStatuses };
  });
}

/** Drop a finished run from the store (e.g. dismiss from the badge). */
export function clearRun(repoPath: string): void {
  if (runs.delete(repoPath)) emit();
}

// --- global event listener ---------------------------------------------------

let initialized = false;

/** Register the single global `pipeline:log` listener. Call once at app start. */
export function initRunStore(): void {
  if (initialized) return;
  initialized = true;
  try {
    listen<LogPayload>('pipeline:log', (event) => {
      const { repo_path } = event.payload;
      // Only feed runs we started (status 'running'). Retries/rollbacks driven
      // from RunDetail manage their own overlay and must not reanimate a finished
      // run left in the store.
      if (repo_path && runs.get(repo_path)?.status === 'running') {
        applyLogLine(repo_path, event.payload);
      }
    }).catch(() => {});
  } catch {
    // No Tauri runtime available (e.g. unit tests) — nothing to listen to.
    initialized = false;
  }
}
