import { useSyncExternalStore } from 'react';
import { listen } from '@tauri-apps/api/event';
import type { NotificationEntry } from '../types';
import {
  deleteNotification,
  listNotifications,
  markAllNotificationsRead,
  markNotificationRead,
  markRunRead,
} from './api';
import { notifyError, notifyInfo, notifySuccess } from './notify';

/**
 * Mirror of the on-disk inbox. Rust owns the data, so this store re-pulls on
 * every signal instead of tracking changes itself: a hidden webview may miss
 * events, and the next focus catches it up. Same module-store shape as `runStore`.
 */

/** Payload emitted by the backend on the `notification:new` event. */
export interface NewNotificationPayload {
  entry: NotificationEntry;
  foreground: boolean;
}

export interface NotificationSummary {
  unread: number;
  unreadFailures: number;
}

// --- store internals ---------------------------------------------------------

let entries: NotificationEntry[] = [];
let summary: NotificationSummary = { unread: 0, unreadFailures: 0 };
let viewedRunId: string | null = null;
const listeners = new Set<() => void>();

function subscribe(cb: () => void): () => void {
  listeners.add(cb);
  return () => {
    listeners.delete(cb);
  };
}

function setEntries(next: NotificationEntry[]): void {
  entries = next;
  const unread = next.filter((e) => !e.read_at);
  summary = {
    unread: unread.length,
    unreadFailures: unread.filter((e) => e.severity === 'failure').length,
  };
  for (const fn of listeners) fn();
}

/** Re-pull the inbox. `Promise.resolve` tolerates automocked APIs in tests. */
export function reloadNotifications(): Promise<void> {
  return Promise.resolve(listNotifications())
    .then((list) => setEntries(list ?? []))
    .catch(() => {});
}

/** Run an inbox change, then show the result. */
function applyChange(change: Promise<void>, failure: string): Promise<void> {
  return Promise.resolve(change)
    .catch((err) => {
      notifyError(failure, err);
    })
    .then(reloadNotifications);
}

// --- public read hooks -------------------------------------------------------

const MAX_SHOWN_COUNT = 99;

export function formatCount(count: number): string {
  return count > MAX_SHOWN_COUNT ? `${MAX_SHOWN_COUNT}+` : String(count);
}

export function useNotifications(): NotificationEntry[] {
  return useSyncExternalStore(subscribe, () => entries);
}

export function useNotificationSummary(): NotificationSummary {
  return useSyncExternalStore(subscribe, () => summary);
}

// --- actions -----------------------------------------------------------------

export function readNotification(id: string): Promise<void> {
  return applyChange(markNotificationRead(id), 'Could not mark notification read');
}

export function readAllNotifications(): Promise<void> {
  return applyChange(markAllNotificationsRead(), 'Could not mark notifications read');
}

export function removeNotification(id: string): Promise<void> {
  return applyChange(deleteNotification(id), 'Could not delete notification');
}

/** Note the run on screen: its entries are read now, and on arrival later. */
export function setViewedRun(runId: string | null): void {
  viewedRunId = runId;
  if (runId) void applyChange(markRunRead(runId), 'Could not mark run read');
}

// --- toasts ------------------------------------------------------------------

/**
 * In-app toasts are for someone looking at the app. Away, the OS alert speaks.
 * Release cards and attended start failures already toast from their own UI.
 */
export function shouldToast({ entry, foreground }: NewNotificationPayload): boolean {
  if (!foreground || entry.kind === 'release_action') return false;
  const attended = entry.run_kind !== 'scheduled' && entry.run_kind !== 'watch';
  return !(entry.kind === 'pre_run_failure' && attended);
}

function showToast(entry: NotificationEntry): void {
  if (entry.severity === 'success') notifySuccess(entry.title, entry.body);
  else if (entry.severity === 'failure') notifyError(entry.title, entry.body);
  else notifyInfo(entry.title, entry.body);
}

export function handleNewNotification(payload: NewNotificationPayload): void {
  const viewing = payload.entry.run_id != null && payload.entry.run_id === viewedRunId;
  if (viewing) void applyChange(markRunRead(viewedRunId as string), 'Could not mark run read');
  if (shouldToast(payload)) showToast(payload.entry);
}

// --- global event listeners --------------------------------------------------

let initialized = false;

/** Register the global inbox listeners and load it. Call once at app start. */
export function initNotificationStore(): void {
  if (initialized) return;
  initialized = true;
  void reloadNotifications();
  window.addEventListener('focus', () => void reloadNotifications());
  try {
    listen<NewNotificationPayload>('notification:new', (event) =>
      handleNewNotification(event.payload)
    ).catch(() => {});
    listen('notification:changed', () => void reloadNotifications()).catch(() => {});
  } catch {
    // No Tauri runtime available (e.g. unit tests) — nothing to listen to.
  }
}
