import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import NotificationBell from '../../components/NotificationBell';
import * as api from '../../services/api';
import { formatCount, reloadNotifications } from '../../services/notificationStore';
import type { NotificationEntry } from '../../types';

vi.mock('../../services/api');

function entries(count: number, severity: NotificationEntry['severity']): NotificationEntry[] {
  return Array.from({ length: count }, (_, i) => ({
    id: `${severity}-${i}`,
    created_at: '2026-11-01T00:00:00Z',
    kind: 'run_finished',
    severity,
    title: 't',
    body: 'b',
    alert: 'none',
    alert_delivered: false,
  }));
}

async function renderWith(list: NotificationEntry[]) {
  vi.mocked(api.listNotifications).mockResolvedValue(list);
  await reloadNotifications();
  render(
    <MemoryRouter>
      <NotificationBell />
    </MemoryRouter>
  );
}

describe('NotificationBell', () => {
  beforeEach(() => vi.clearAllMocks());

  it('caps the shown count', () => {
    expect(formatCount(99)).toBe('99');
    expect(formatCount(100)).toBe('99+');
  });

  it('shows no count when nothing is unread', async () => {
    await renderWith([]);

    expect(screen.getByRole('link', { name: 'Notifications' })).toBeInTheDocument();
  });

  it('shows the count in the accent style when nothing failed', async () => {
    await renderWith(entries(2, 'success'));

    await waitFor(() => expect(screen.getByText('2')).toHaveClass('nav-count'));
    expect(screen.getByText('2')).not.toHaveClass('nav-count-failed');
    expect(screen.getByRole('link', { name: 'Notifications, 2 unread' })).toBeInTheDocument();
  });

  it('turns red when a failure is unread', async () => {
    await renderWith([...entries(2, 'success'), ...entries(1, 'failure')]);

    await waitFor(() => expect(screen.getByText('3')).toHaveClass('nav-count-failed'));
  });
});
