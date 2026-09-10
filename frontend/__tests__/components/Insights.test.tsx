import { describe, it, expect, vi, beforeEach } from 'vitest';
import { screen, waitFor } from '@testing-library/react';
import { renderWithRouter } from '../test-utils';
import Insights from '../../components/Insights';
import * as api from '../../services/api';
import type { InsightsReport, PeriodStats } from '../../types';

vi.mock('../../services/api');

function period(overrides: Partial<PeriodStats> = {}): PeriodStats {
  return {
    runs: 10,
    succeeded: 8,
    failed: 2,
    cancelled: 0,
    success_rate: 0.8,
    avg_duration_ms: 60_000,
    p95_duration_ms: 90_000,
    unattended_runs: 3,
    unattended_failures: 1,
    ...overrides,
  };
}

function report(overrides: Partial<InsightsReport> = {}): InsightsReport {
  return {
    generated_at: '2026-09-10T12:00:00Z',
    window_days: 7,
    totals: {
      current: period(),
      previous: period({ runs: 6, succeeded: 3, failed: 3, success_rate: 0.5 }),
      success_rate_delta: 30,
      avg_duration_delta_pct: -12.5,
    },
    daily: [
      { date: '2026-09-09', runs: 4, succeeded: 3, failed: 1 },
      { date: '2026-09-10', runs: 6, succeeded: 5, failed: 1 },
    ],
    environments: [
      {
        repo_path: '/repos/api',
        project_name: 'api',
        environment: 'production',
        current_run_id: 'aaaaaaaa-1111-2222-3333-444444444444',
        commit: 'abcdef1234567',
        branch: 'main',
        deployed_at: '2026-09-01T10:00:00Z',
        is_stale: false,
        failed_since: 0,
        last_rollback_at: null,
        last_rollback_outcome: null,
      },
    ],
    stages: [
      {
        stage_name: 'build',
        runs: 10,
        failures: 1,
        timeouts: 0,
        failure_rate: 0.1,
        flaky_passes: 0,
        flaky_rate: 0,
        avg_duration_ms: 30_000,
        p95_duration_ms: 40_000,
        slowest_run_id: 'bbbbbbbb-1111-2222-3333-444444444444',
      },
    ],
    hotspots: [
      {
        repo_path: '/repos/api',
        project_name: 'api',
        top_failing_stage: 'test',
        stage_failures: 2,
        total_failures: 3,
        health_check_failures: 1,
        top_health_failure_stage: 'deploy',
      },
    ],
    slowest_stages: [
      {
        stage_name: 'test',
        current_avg_ms: 50_000,
        previous_avg_ms: 40_000,
        delta_pct: 25,
        runs: 10,
      },
    ],
    ...overrides,
  };
}

const EMPTY_REPORT: InsightsReport = {
  generated_at: '2026-09-10T12:00:00Z',
  window_days: 7,
  totals: {
    current: period({
      runs: 0,
      succeeded: 0,
      failed: 0,
      cancelled: 0,
      success_rate: 0,
      avg_duration_ms: null,
      p95_duration_ms: null,
      unattended_runs: 0,
      unattended_failures: 0,
    }),
    previous: period({
      runs: 0,
      succeeded: 0,
      failed: 0,
      cancelled: 0,
      success_rate: 0,
      avg_duration_ms: null,
      p95_duration_ms: null,
      unattended_runs: 0,
      unattended_failures: 0,
    }),
    success_rate_delta: null,
    avg_duration_delta_pct: null,
  },
  // A fresh install still gets a row per day — all zeros. That must read as
  // "no data yet", not as a table of zeros.
  daily: [
    { date: '2026-09-09', runs: 0, succeeded: 0, failed: 0 },
    { date: '2026-09-10', runs: 0, succeeded: 0, failed: 0 },
  ],
  environments: [],
  stages: [],
  hotspots: [],
  slowest_stages: [],
};

/**
 * A brand-new project's first window: runs happened, but the window before it
 * had none, so both deltas come back null. This is where a `+0.0` would lie.
 */
const NO_BASELINE_REPORT: InsightsReport = report({
  totals: {
    current: period(),
    previous: period({
      runs: 0,
      succeeded: 0,
      failed: 0,
      cancelled: 0,
      success_rate: 0,
      avg_duration_ms: null,
      p95_duration_ms: null,
      unattended_runs: 0,
      unattended_failures: 0,
    }),
    success_rate_delta: null,
    avg_duration_delta_pct: null,
  },
  slowest_stages: [
    {
      stage_name: 'test',
      current_avg_ms: 50_000,
      previous_avg_ms: null,
      delta_pct: null,
      runs: 10,
    },
  ],
});

/** The stat tile carrying the given label. */
function tile(label: string): HTMLElement {
  return screen.getByText(label).closest('.stat-card') as HTMLElement;
}

describe('Insights', () => {
  beforeEach(() => {
    vi.mocked(api.listProjects).mockResolvedValue([]);
    vi.mocked(api.getRunIndexStats).mockResolvedValue({
      entries: 42,
      payloads_pruned: 7,
      bytes: 2048,
    });
    vi.mocked(api.getInsights).mockResolvedValue(report());
  });

  it('renders every section from the report', async () => {
    renderWithRouter(<Insights />);

    expect(await screen.findByRole('heading', { name: /totals/i })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: /deploy health/i })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: /stage reliability/i })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: /failure hotspots/i })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: /slowest stages/i })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: /daily/i })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: /run index/i })).toBeInTheDocument();
  });

  it('fetches every project by default and links runs to their detail page', async () => {
    renderWithRouter(<Insights />);

    await screen.findByRole('heading', { name: /totals/i });
    expect(api.getInsights).toHaveBeenCalledWith(null, 7);
    expect(screen.getByRole('link', { name: 'aaaaaaaa' })).toHaveAttribute(
      'href',
      '/run/aaaaaaaa-1111-2222-3333-444444444444'
    );
  });

  it('renders the run index stats in human-readable units', async () => {
    renderWithRouter(<Insights />);

    expect(await screen.findByText('42')).toBeInTheDocument();
    expect(screen.getByText('2.0 KB')).toBeInTheDocument();
  });

  // The regression that matters: a first-ever window has no baseline, so the
  // delta must be a dash. Rendering +0.0 would claim a fabricated improvement.
  it('renders a dash for a null success rate delta and never a zero delta', async () => {
    vi.mocked(api.getInsights).mockResolvedValue(NO_BASELINE_REPORT);
    const { container } = renderWithRouter(<Insights />);

    await screen.findByRole('heading', { name: /totals/i });
    const successTile = tile('Success rate');
    // The rate itself is real (80.0%); only its delta has no baseline.
    expect(successTile).toHaveTextContent('80.0%');
    expect(successTile).toHaveTextContent('—');
    expect(successTile.textContent).not.toContain('+0.0');
    expect(tile('Avg duration')).toHaveTextContent('—');
    expect(container.textContent).not.toContain('+0.0');
    expect(container.textContent).not.toContain('-0.0');
  });

  it('does not colour a null delta as an improvement', async () => {
    vi.mocked(api.getInsights).mockResolvedValue(NO_BASELINE_REPORT);
    const { container } = renderWithRouter(<Insights />);

    await screen.findByRole('heading', { name: /totals/i });
    expect(container.querySelector('.insights-delta-success')).toBeNull();
    expect(container.querySelector('.insights-delta-failed')).toBeNull();
    expect(container.querySelectorAll('.insights-delta-neutral').length).toBeGreaterThan(0);
  });

  it('presents a stage that only passes on retry as a problem', async () => {
    vi.mocked(api.getInsights).mockResolvedValue(
      report({
        stages: [
          {
            stage_name: 'integration',
            runs: 10,
            failures: 0,
            timeouts: 0,
            failure_rate: 0,
            flaky_passes: 3,
            flaky_rate: 0.3,
            avg_duration_ms: 20_000,
            p95_duration_ms: 25_000,
            slowest_run_id: null,
          },
        ],
      })
    );
    const { container } = renderWithRouter(<Insights />);

    const flakyBadge = await screen.findByText(/3 flaky/);
    expect(flakyBadge).toHaveClass('badge-warning');
    expect(container.querySelector('.insights-row-flaky')).toBeInTheDocument();
    // Flakiness stays separate from the failure rate so a 0% failure rate
    // cannot make the row look healthy.
    expect(screen.getByText('30.0%')).toBeInTheDocument();
    expect(screen.getByText('0.0%')).toBeInTheDocument();
  });

  it('distinguishes a stale environment row', async () => {
    vi.mocked(api.getInsights).mockResolvedValue(
      report({
        environments: [
          {
            repo_path: '/repos/api',
            project_name: 'api',
            environment: 'production',
            current_run_id: 'cccccccc-1111-2222-3333-444444444444',
            commit: 'abcdef1234567',
            branch: 'main',
            deployed_at: '2026-09-01T10:00:00Z',
            is_stale: true,
            failed_since: 3,
            last_rollback_at: '2026-09-02T10:00:00Z',
            last_rollback_outcome: 'failed',
          },
        ],
      })
    );
    const { container } = renderWithRouter(<Insights />);

    expect(await screen.findByText(/3 failed since/)).toBeInTheDocument();
    const staleRow = container.querySelector('.insights-row-stale');
    expect(staleRow).toBeInTheDocument();
    expect(staleRow).toHaveTextContent('production');
  });

  // Lower is better for duration: a slower window is a regression, not green.
  it('presents a slower current window as a regression', async () => {
    renderWithRouter(<Insights />);

    await screen.findByRole('heading', { name: /slowest stages/i });
    const slower = screen.getByText('+25.0%').closest('.insights-delta');
    expect(slower).toHaveClass('insights-delta-failed');

    // And the mirror case: the totals row got faster, which is an improvement.
    const faster = screen.getByText('-12.5%').closest('.insights-delta');
    expect(faster).toHaveClass('insights-delta-success');
  });

  it('shows an empty state per section and no NaN when there is no data', async () => {
    vi.mocked(api.getInsights).mockResolvedValue(EMPTY_REPORT);
    const { container } = renderWithRouter(<Insights />);

    expect(await screen.findByText(/no runs recorded in either window/i)).toBeInTheDocument();
    expect(screen.getByText(/no environment-scoped runs yet/i)).toBeInTheDocument();
    expect(screen.getByText(/no stage executions in this window/i)).toBeInTheDocument();
    expect(screen.getByText(/no failures in this window/i)).toBeInTheDocument();
    expect(screen.getByText(/not enough history to compare/i)).toBeInTheDocument();
    expect(screen.getByText(/no runs on any day in this window/i)).toBeInTheDocument();
    expect(container.textContent).not.toContain('NaN');
    expect(container.querySelector('.insights-table')).toBeNull();
  });

  it('re-fetches with the selected window', async () => {
    renderWithRouter(<Insights />);

    await screen.findByRole('heading', { name: /totals/i });
    screen.getByRole('button', { name: '30 days' }).click();

    await waitFor(() => expect(api.getInsights).toHaveBeenCalledWith(null, 30));
  });
});
