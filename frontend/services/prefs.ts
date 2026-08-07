import { useState, useCallback } from 'react';

// Lightweight, typed localStorage wrappers for UI preferences.
// The app's only persistence for ephemeral view/theme choices.

export type Theme = 'dark' | 'light';
export type ProjectsView = 'cards' | 'table';

export const PREF_THEME = 'chibby.theme';
export const PREF_PROJECTS_VIEW = 'chibby.projectsView';

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
