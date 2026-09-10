import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import RunDetail from '../../components/RunDetail';
import * as api from '../../services/api';
import type { PipelineRun, StageResult } from '../../types';

vi.mock('../../services/api');
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));

function stage(overrides: Partial<StageResult> = {}): StageResult {
  return {
    stage_name: 'build',
    status: 'success',
    stdout: '',
    stderr: '',
    ...overrides,
  };
}

function createRun(stages: StageResult[], overrides: Partial<PipelineRun> = {}): PipelineRun {
  return {
    id: 'run-1',
    pipeline_name: 'ci',
    repo_path: '/tmp/proj',
    status: 'success',
    stage_results: stages,
    started_at: '2026-03-01T10:00:00Z',
    ...overrides,
  };
}

function renderRun(run: PipelineRun) {
  vi.mocked(api.getRun).mockResolvedValue(run);
  return render(
    <MemoryRouter initialEntries={['/run/run-1']}>
      <Routes>
        <Route path="/run/:runId" element={<RunDetail />} />
      </Routes>
    </MemoryRouter>
  );
}

describe('RunDetail stage metadata', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows the attempt count when a stage was retried', async () => {
    renderRun(createRun([stage({ attempts: 3 })]));

    expect(await screen.findByText('3 attempts')).toBeInTheDocument();
  });

  it('does not show an attempt count for a single-attempt stage', async () => {
    renderRun(createRun([stage({ attempts: 1 })]));

    await screen.findAllByText('build');
    expect(screen.queryByText('1 attempts')).not.toBeInTheDocument();
  });

  it('shows the skip reason for a skipped stage', async () => {
    renderRun(
      createRun([stage({ status: 'skipped', skip_reason: "branch 'dev' does not match [main]" })])
    );

    expect(
      await screen.findByText(/Skipped: branch 'dev' does not match \[main\]/)
    ).toBeInTheDocument();
  });

  it('renders a timed-out stage as a failure', async () => {
    const run = createRun([stage({ status: 'timedout' })], { status: 'failed' });
    const { container } = renderRun(run);

    await screen.findAllByText('build');
    expect(container.querySelector('.stage-sidebar-item .status-failed')).toBeInTheDocument();
    // A timed-out stage is retry-eligible, like a failed one.
    expect(screen.getByTitle('Retry from failed stage: build')).toBeInTheDocument();
  });

  it('renders the git provenance line', async () => {
    renderRun(createRun([stage()], { branch: 'main', commit: 'abcdef1234567890' }));

    expect(await screen.findByText('main')).toBeInTheDocument();
    expect(screen.getByText('abcdef12')).toBeInTheDocument();
  });
});

describe('RunDetail auto-rollback', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  const HEALTH_FAILURE = /Health check failed on stage "deploy"/;

  function healthFailureRun(overrides: Partial<PipelineRun> = {}): PipelineRun {
    return createRun([stage({ stage_name: 'deploy', status: 'failed' })], {
      status: 'failed',
      health_failure_stage: 'deploy',
      ...overrides,
    });
  }

  it('links to the restored run when the rollback succeeded', async () => {
    renderRun(healthFailureRun({ rollback_outcome: 'succeeded', rollback_run_id: 'rollback-9' }));

    expect(await screen.findByText(HEALTH_FAILURE)).toBeInTheDocument();
    expect(screen.getByText('Automatically rolled back')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'View the restored run' })).toHaveAttribute(
      'href',
      '/run/rollback-9'
    );
  });

  it('demands manual intervention when the rollback also failed', async () => {
    const { container } = renderRun(
      healthFailureRun({ rollback_outcome: 'failed', rollback_run_id: 'rollback-9' })
    );

    expect(
      await screen.findByText(/Automatic rollback FAILED — manual intervention required/)
    ).toBeInTheDocument();
    expect(container.querySelector('.rollback-notice-failed')).toBeInTheDocument();
  });

  it('shows the skip reason verbatim when a guard refused the rollback', async () => {
    const reason = '1 auto-rollback(s) already ran in the last 60 minutes';
    renderRun(healthFailureRun({ rollback_outcome: 'skipped', rollback_skip_reason: reason }));

    expect(await screen.findByText(`Reason: ${reason}`)).toBeInTheDocument();
    expect(
      screen.getByText(/Automatic rollback skipped — the bad release is still live/)
    ).toBeInTheDocument();
  });

  it('shows only the health-check failure when no rollback policy was configured', async () => {
    const { container } = renderRun(healthFailureRun());

    expect(await screen.findByText(HEALTH_FAILURE)).toBeInTheDocument();
    expect(container.querySelector('.rollback-notice-outcome')).not.toBeInTheDocument();
  });

  it('renders no rollback UI for a plain command failure', async () => {
    const { container } = renderRun(createRun([stage({ status: 'failed' })], { status: 'failed' }));

    await screen.findAllByText('build');
    expect(container.querySelector('.rollback-notice')).not.toBeInTheDocument();
  });

  it('marks a rollback run as automatic and links back to the run that caused it', async () => {
    renderRun(createRun([stage()], { run_kind: 'rollback', auto_rollback_of: 'bad-run-1' }));

    expect(await screen.findByText('Auto-rollback')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'view the run that triggered it' })).toHaveAttribute(
      'href',
      '/run/bad-run-1'
    );
  });

  it('leaves a manual rollback run labelled as a plain rollback', async () => {
    renderRun(createRun([stage()], { run_kind: 'rollback', rollback_target_id: 'good-run-1' }));

    expect(await screen.findByText('Rollback')).toBeInTheDocument();
    expect(screen.queryByText('Auto-rollback')).not.toBeInTheDocument();
  });
});

describe('RunDetail without provenance', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('omits the commit when the backend did not record one', async () => {
    const { container } = renderRun(createRun([stage()]));

    await waitFor(() => expect(container.querySelector('.run-detail-meta')).toBeInTheDocument());
    expect(screen.queryByText('abcdef12')).not.toBeInTheDocument();
  });
});

// Run summaries outlive their logs, so an Insights link can point at a run
// whose record is gone. That is expected history, not an error.
describe('RunDetail for a pruned run', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('degrades to a "no longer available" state instead of loading forever', async () => {
    vi.mocked(api.getRun).mockResolvedValue(null);
    render(
      <MemoryRouter initialEntries={['/run/gone']}>
        <Routes>
          <Route path="/run/:runId" element={<RunDetail />} />
        </Routes>
      </MemoryRouter>
    );

    expect(await screen.findByText(/no longer available/i)).toBeInTheDocument();
    expect(screen.queryByText('Loading run...')).not.toBeInTheDocument();
  });
});
