import { useState, useCallback } from 'react';

// Lightweight, typed localStorage wrappers for UI preferences.
// The app's only persistence for ephemeral view/theme choices.

export type Theme = 'dark' | 'light';
export type ProjectsView = 'cards' | 'table';
export type CardTableView = 'cards' | 'table';
/** Agent posture: 'advise' = read-only investigate & recommend; 'act' = full tool loop. */
export type AgentInteractionMode = 'advise' | 'act';

export const PREF_THEME = 'chibby.theme';
export const PREF_PROJECTS_VIEW = 'chibby.projectsView';
export const PREF_TEMPLATES_VIEW = 'chibby.templatesView';
export const PREF_AGENT_MODE = 'chibby.agentMode';
export const PREF_INSIGHTS_WINDOW = 'chibby.insightsWindow';

export function getPref<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw === null ? fallback : (JSON.parse(raw) as T);
  } catch {
    return fallback;
  }
}

export function setPref<T>(key: string, value: T): void {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Storage unavailable (private mode / quota) — pref is best-effort.
  }
}

// useState-backed hook that writes through to localStorage on change.
export function usePref<T>(key: string, fallback: T): [T, (value: T) => void] {
  const [value, setValue] = useState<T>(() => getPref(key, fallback));
  const set = useCallback(
    (next: T) => {
      setValue(next);
      setPref(key, next);
    },
    [key]
  );
  return [value, set];
}
