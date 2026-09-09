import { useSyncExternalStore } from 'react';
import { listen } from '@tauri-apps/api/event';
import type { ChatTurn } from '../types';
import { agentChat, agentRunToolSession, approveAgentAction } from './api';

/**
 * Per-project agent sessions, keyed by a session key (e.g. `project:<id>` or
 * `general`). Concurrent projects keep independent context — mirrors the
 * module-store pattern in `runStore.ts`. Only transient state lives here.
 *
 * Two interaction modes share one item stream:
 *  - advisory chat (`sendAgentMessage`) — no project/tools;
 *  - action-taking tool sessions (`runToolSession`) — streams command/edit
 *    events and pauses for approval.
 */

export type AgentItem =
  | { kind: 'user'; text: string }
  | { kind: 'assistant'; text: string; skill?: string }
  | {
      kind: 'tool';
      id: string;
      name: string;
      summary: string;
      status: 'running' | 'ok' | 'error';
      output?: string;
    }
  | {
      kind: 'approval';
      id: string;
      tool: string;
      summary: string;
      reason: string;
      resolved?: 'approved' | 'rejected';
    }
  | { kind: 'error'; text: string };

export interface AgentSession {
  key: string;
  items: AgentItem[];
  running: boolean;
  /** Backend session id for the in-flight tool session, if any. */
  sessionId?: string;
}

interface AgentUI {
  open: boolean;
  width: number;
}

// --- store internals ---------------------------------------------------------

const sessions = new Map<string, AgentSession>();
/** Maps a backend tool-session id back to its session key. */
const bySessionId = new Map<string, string>();
let ui: AgentUI = { open: false, width: 420 };
const listeners = new Set<() => void>();

const EMPTY_SESSION: AgentSession = { key: '', items: [], running: false };

function emit(): void {
  for (const fn of listeners) fn();
}

function subscribe(cb: () => void): () => void {
  listeners.add(cb);
  return () => {
    listeners.delete(cb);
  };
}

function setSession(key: string, updater: (s: AgentSession) => AgentSession): void {
  const cur = sessions.get(key) ?? { key, items: [], running: false };
  sessions.set(key, updater(cur));
  emit();
}

// --- read hooks --------------------------------------------------------------

export function useAgentSession(key: string): AgentSession {
  return useSyncExternalStore(subscribe, () => sessions.get(key) ?? EMPTY_SESSION);
}

export function useAgentUI(): AgentUI {
  return useSyncExternalStore(subscribe, () => ui);
}

// --- UI actions --------------------------------------------------------------

export function openDrawer(): void {
  if (!ui.open) {
    ui = { ...ui, open: true };
    emit();
  }
}

export function closeDrawer(): void {
  if (ui.open) {
    ui = { ...ui, open: false };
    emit();
  }
}

const MIN_WIDTH = 320;
const MAX_WIDTH = 720;

export function setDrawerWidth(width: number): void {
  const clamped = Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, width));
  if (clamped !== ui.width) {
    ui = { ...ui, width: clamped };
    emit();
  }
}

// --- advisory chat (no project / no tools) -----------------------------------

export async function sendAgentMessage(
  key: string,
  text: string,
  projectId?: string
): Promise<void> {
  const history: ChatTurn[] = (sessions.get(key)?.items ?? [])
    .filter(
      (i): i is Extract<AgentItem, { kind: 'user' | 'assistant' }> =>
        i.kind === 'user' || i.kind === 'assistant'
    )
    .map((i) => ({ role: i.kind === 'user' ? 'user' : 'assistant', content: i.text }));

  setSession(key, (s) => ({ ...s, items: [...s.items, { kind: 'user', text }], running: true }));

  try {
    const res = await agentChat(text, history, projectId);
    setSession(key, (s) => ({
      ...s,
      items: [...s.items, { kind: 'assistant', text: res.message, skill: res.skill_used }],
      running: false,
    }));
  } catch (err) {
    setSession(key, (s) => ({
      ...s,
      items: [...s.items, { kind: 'error', text: String(err) }],
      running: false,
    }));
  }
}

// --- action-taking tool session ----------------------------------------------

export async function runToolSession(
  key: string,
  text: string,
  projectPath: string,
  readOnly: boolean
): Promise<void> {
  const sessionId = crypto.randomUUID();
  bySessionId.set(sessionId, key);

  setSession(key, (s) => ({
    ...s,
    items: [...s.items, { kind: 'user', text }],
    running: true,
    sessionId,
  }));

  try {
    await agentRunToolSession(sessionId, text, projectPath, readOnly);
  } catch (err) {
    setSession(key, (s) => ({ ...s, items: [...s.items, { kind: 'error', text: String(err) }] }));
  } finally {
    setSession(key, (s) => ({ ...s, running: false, sessionId: undefined }));
    bySessionId.delete(sessionId);
  }
}

export async function resolveApproval(
  key: string,
  actionId: string,
  approved: boolean
): Promise<void> {
  const sessionId = sessions.get(key)?.sessionId;
  if (!sessionId) return;
  // Mark the approval item resolved optimistically.
  setSession(key, (s) => ({
    ...s,
    items: s.items.map((i) =>
      i.kind === 'approval' && i.id === actionId
        ? { ...i, resolved: approved ? 'approved' : 'rejected' }
        : i
    ),
  }));
  try {
    await approveAgentAction(sessionId, actionId, approved);
  } catch {
    // Session may have already resolved; ignore.
  }
}

export function clearAgentSession(key: string): void {
  if (sessions.delete(key)) emit();
}

// --- event stream ------------------------------------------------------------

interface SessionEventPayload {
  session_id: string;
  kind: string;
  text?: string;
  id?: string;
  name?: string;
  summary?: string;
  ok?: boolean;
  output?: string;
  message?: string;
  pending?: { id: string; tool: string; summary: string; reason: string };
}

function handleEvent(payload: SessionEventPayload): void {
  const key = bySessionId.get(payload.session_id);
  if (!key) return;

  switch (payload.kind) {
    case 'assistant':
      setSession(key, (s) => ({
        ...s,
        items: [...s.items, { kind: 'assistant', text: payload.text ?? '' }],
      }));
      break;
    case 'tool_start':
      setSession(key, (s) => ({
        ...s,
        items: [
          ...s.items,
          {
            kind: 'tool',
            id: payload.id ?? '',
            name: payload.name ?? '',
            summary: payload.summary ?? '',
            status: 'running',
          },
        ],
      }));
      break;
    case 'tool_result':
      setSession(key, (s) => ({
        ...s,
        items: s.items.map((i) =>
          i.kind === 'tool' && i.id === payload.id
            ? { ...i, status: payload.ok ? 'ok' : 'error', output: payload.output }
            : i
        ),
      }));
      break;
    case 'awaiting_approval':
      if (payload.pending) {
        const p = payload.pending;
        setSession(key, (s) => ({
          ...s,
          items: [
            ...s.items,
            { kind: 'approval', id: p.id, tool: p.tool, summary: p.summary, reason: p.reason },
          ],
        }));
      }
      break;
    case 'error':
      setSession(key, (s) => ({
        ...s,
        items: [...s.items, { kind: 'error', text: payload.message ?? '' }],
      }));
      break;
    // 'done' is handled by the run promise resolving.
    default:
      break;
  }
}

let initialized = false;

/** Register the single global `agent:session` listener. Call once at app start. */
export function initAgentStore(): void {
  if (initialized) return;
  initialized = true;
  try {
    listen<SessionEventPayload>('agent:session', (event) => handleEvent(event.payload)).catch(
      () => {}
    );
  } catch {
    initialized = false;
  }
}
