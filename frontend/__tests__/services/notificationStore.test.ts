import { describe, it, expect, vi, beforeEach } from 'vitest';
import * as api from '../../services/api';
import * as notify from '../../services/notify';
import {
  handleNewNotification,
  reloadNotifications,
  setViewedRun,
  shouldToast,
  useNotificationSummary,
} from '../../services/notificationStore';
import { renderHook, waitFor } from '@testing-library/react';
import type { NotificationEntry } from '../../types';

vi.mock('../../services/api');
vi.mock('../../services/notify');

function entry(overrides: Partial<NotificationEntry> = {}): NotificationEntry {
  return {
    id: 'a',
    created_at: '2026-11-01T00:00:00Z',
    kind: 'run_finished',
    severity: 'success',
    title: 'shop: deploy succeeded',
    body: 'done',
    alert: 'when_away',
    alert_delivered: true,
    ...overrides,
  };
}

describe('notificationStore', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setViewedRun(null);
    vi.mocked(api.markRunRead).mockResolvedValue();
    vi.mocked(api.listNotifications).mockResolvedValue([]);
  });

  it.each([
    ['run in front', {}, true, true],
    ['run while away', {}, false, false],
    ['release card already toasts', { kind: 'release_action' }, true, false],
    ['attended start failure already toasts', { kind: 'pre_run_failure' }, true, false],
    [
      'manual start failure already toasts',
      { kind: 'pre_run_failure', run_kind: 'normal' },
      true,
      false,
    ],
    [
      'schedule failing to start has no other toast',
      { kind: 'pre_run_failure', run_kind: 'scheduled' },
      true,
      true,
    ],
    ['interrupted run', { kind: 'run_interrupted' }, true, true],
  ] as const)('shouldToast: %s', (_name, overrides, foreground, expected) => {
    expect(shouldToast({ entry: entry(overrides), foreground })).toBe(expected);
  });

  it('toasts by severity', () => {
    handleNewNotification({ entry: entry(), foreground: true });
    handleNewNotification({ entry: entry({ severity: 'failure' }), foreground: true });
    handleNewNotification({ entry: entry({ severity: 'info' }), foreground: true });

    expect(notify.notifySuccess).toHaveBeenCalledWith('shop: deploy succeeded', 'done');
    expect(notify.notifyError).toHaveBeenCalledWith('shop: deploy succeeded', 'done');
    expect(notify.notifyInfo).toHaveBeenCalledWith('shop: deploy succeeded', 'done');
  });

  it('counts unread and unread failures, ignoring read entries', async () => {
    vi.mocked(api.listNotifications).mockResolvedValue([
      entry({ id: 'a', severity: 'failure' }),
      entry({ id: 'b' }),
      entry({ id: 'c', severity: 'failure', read_at: '2026-11-01T00:00:01Z' }),
    ]);
    const { result } = renderHook(() => useNotificationSummary());

    await reloadNotifications();

    await waitFor(() => expect(result.current).toEqual({ unread: 2, unreadFailures: 1 }));
  });

  it('marks a run read when viewed, and again for an entry arriving during viewing', async () => {
    setViewedRun('run-1');
    expect(api.markRunRead).toHaveBeenCalledTimes(1);

    handleNewNotification({ entry: entry({ run_id: 'run-1' }), foreground: true });
    expect(api.markRunRead).toHaveBeenCalledTimes(2);

    handleNewNotification({ entry: entry({ run_id: 'run-2' }), foreground: true });
    expect(api.markRunRead).toHaveBeenCalledTimes(2);
  });

  it('survives an automocked API returning undefined', async () => {
    vi.mocked(api.listNotifications).mockReturnValue(
      undefined as unknown as Promise<NotificationEntry[]>
    );

    await expect(reloadNotifications()).resolves.toBeUndefined();
  });
});
