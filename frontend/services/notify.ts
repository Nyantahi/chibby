export type NotifyKind = 'info' | 'success' | 'error';

export interface NotifyEntry {
  id: number;
  kind: NotifyKind;
  message: string;
  detail?: string;
  createdAt: number;
}

type Listener = (entries: NotifyEntry[]) => void;

let entries: NotifyEntry[] = [];
let nextId = 1;
const listeners = new Set<Listener>();

function emit() {
  for (const fn of listeners) fn(entries);
}

export function subscribeNotify(listener: Listener): () => void {
  listeners.add(listener);
  listener(entries);
  return () => {
    listeners.delete(listener);
  };
}

export function pushNotify(kind: NotifyKind, message: string, detail?: string): number {
  const id = nextId++;
  entries = [...entries, { id, kind, message, detail, createdAt: Date.now() }];
  emit();
  const ttl = kind === 'error' ? 8000 : 4000;
  window.setTimeout(() => dismissNotify(id), ttl);
  return id;
}

export function dismissNotify(id: number): void {
  const next = entries.filter((e) => e.id !== id);
  if (next.length === entries.length) return;
  entries = next;
  emit();
}

export function notifyInfo(message: string, detail?: string): number {
  return pushNotify('info', message, detail);
}

export function notifySuccess(message: string, detail?: string): number {
  return pushNotify('success', message, detail);
}

export function notifyError(message: string, err?: unknown): number {
  const detail = err == null ? undefined : err instanceof Error ? err.message : String(err);
  return pushNotify('error', message, detail);
}
