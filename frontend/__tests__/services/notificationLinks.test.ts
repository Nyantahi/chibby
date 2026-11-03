import { describe, it, expect } from 'vitest';
import { notificationLink } from '../../services/notificationLinks';
import type { NotificationEntry, ProjectInfo } from '../../types';

const projects = [{ project: { id: 'p1', path: '/repos/shop' } }] as ProjectInfo[];

function entry(overrides: Partial<NotificationEntry>): NotificationEntry {
  return {
    id: 'a',
    created_at: '2026-11-01T00:00:00Z',
    kind: 'run_finished',
    severity: 'success',
    title: 't',
    body: 'b',
    alert: 'none',
    alert_delivered: false,
    repo_path: '/repos/shop',
    ...overrides,
  };
}

describe('notificationLink', () => {
  it('links a run with its project for the back link', () => {
    expect(notificationLink(entry({ run_id: 'r1' }), projects)).toEqual({
      to: '/run/r1',
      state: { projectId: 'p1', tab: 'history' },
    });
  });

  it('still links a run whose project was removed', () => {
    expect(notificationLink(entry({ run_id: 'r1' }), [])).toEqual({
      to: '/run/r1',
      state: { projectId: undefined, tab: 'history' },
    });
  });

  it('links a release action to the release tab', () => {
    expect(notificationLink(entry({ kind: 'release_action' }), projects)).toEqual({
      to: '/project/p1',
      state: { tab: 'release' },
    });
  });

  it('links a start failure to its project', () => {
    expect(notificationLink(entry({ kind: 'pre_run_failure' }), projects)).toEqual({
      to: '/project/p1',
    });
  });

  it('has nowhere to go without a run or project', () => {
    expect(notificationLink(entry({ kind: 'pre_run_failure' }), [])).toBeNull();
  });
});
