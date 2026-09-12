import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import TriggersTab from '../../components/project-detail/TriggersTab';
import * as api from '../../services/api';
import * as notify from '../../services/notify';
import type { InstallReport, TriggersConfig } from '../../types';

vi.mock('../../services/api');
vi.mock('../../services/notify');

const REPO = '/tmp/proj';

const CONFIG: TriggersConfig = {
  enabled: true,
  schedules: [
    {
      id: 'nightly',
      enabled: true,
      cron: '0 3 * * *',
      missed: 'run_once',
      environment: 'staging',
      stages: ['build', 'test'],
    },
  ],
  watches: [
    {
      id: 'on-src-change',
      enabled: true,
      include: ['src/**'],
      exclude: ['**/*.snap'],
      debounce_ms: 750,
      min_interval_secs: 10,
      stages: [],
    },
  ],
  hooks: { pre_push: { stages: ['test'], blocking: true } },
};

const NEXT_TIMES = ['2026-09-11T03:00:00Z', '2026-09-12T03:00:00Z', '2026-09-13T03:00:00Z'];

/** A foreign hook left untouched — the expected outcome, not a failure. */
const FOREIGN_REPORT: InstallReport = {
  path: '/tmp/proj/.git/hooks/pre-push',
  state_before: 'foreign',
  installed: false,
  snippet: '# >>> chibby pre-push >>>\nchibby run --stages test\n# <<< chibby pre-push <<<',
  message: '/tmp/proj/.git/hooks/pre-push already exists and was not written by Chibby.',
};

function renderTab(config: TriggersConfig = CONFIG) {
  vi.mocked(api.loadTriggers).mockResolvedValue(config);
  vi.mocked(api.loadTriggersLocal).mockResolvedValue(config);
  vi.mocked(api.getTriggerState).mockResolvedValue({
    nightly: { last_fired_at: '2026-09-09T03:00:00Z', last_run_id: 'run-9' },
  });
  vi.mocked(api.gitHookStatus).mockResolvedValue('not_installed');
  return render(
    <MemoryRouter>
      <TriggersTab repoPath={REPO} environments={['staging', 'production']} />
    </MemoryRouter>
  );
}

describe('TriggersTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(api.nextRunTimes).mockResolvedValue(NEXT_TIMES);
  });

  it('renders schedules, watches and hooks from the loaded config', async () => {
    renderTab();

    expect(await screen.findByText('nightly')).toBeInTheDocument();
    expect(screen.getByText('0 3 * * *')).toBeInTheDocument();
    expect(screen.getByText('Missed: Run once')).toBeInTheDocument();

    expect(screen.getByText('on-src-change')).toBeInTheDocument();
    expect(screen.getByText('src/**')).toBeInTheDocument();

    expect(screen.getByText('pre-push')).toBeInTheDocument();
    expect(screen.getByText('pre-commit')).toBeInTheDocument();

    // The committed file alone — never the merged view, which saving back
    // would publish a developer's local triggers into.
    expect(api.loadTriggers).toHaveBeenCalledWith(REPO, false);
  });

  it('says plainly that nothing fires in the background', async () => {
    renderTab();

    await screen.findByText('nightly');
    expect(screen.getByText(/installs no background daemon/i)).toBeInTheDocument();
    expect(screen.getByText(/chibby schedule --once/i)).toBeInTheDocument();
  });

  it('shows the last fire time and a link to the last run', async () => {
    renderTab();

    expect((await screen.findAllByText(/Last fired:/)).length).toBeGreaterThan(0);
    expect(screen.getByRole('link', { name: 'View last run' })).toHaveAttribute(
      'href',
      '/run/run-9'
    );
  });

  it('previews upcoming fire times for a cron while editing', async () => {
    const user = userEvent.setup();
    renderTab();

    await user.click(await screen.findByRole('button', { name: /add schedule/i }));

    expect(await screen.findByText('Next 3 fire times')).toBeInTheDocument();
    expect(api.nextRunTimes).toHaveBeenCalledWith('0 3 * * *', 5);
    // One rendered entry per returned time.
    expect(
      screen.getByText('Next 3 fire times').parentElement?.querySelectorAll('li')
    ).toHaveLength(NEXT_TIMES.length);
  });

  it('surfaces an invalid cron inline rather than as a toast', async () => {
    vi.mocked(api.nextRunTimes).mockRejectedValue(new Error('invalid cron expression: "nope"'));
    renderTab();

    const errors = await screen.findAllByText(/invalid cron expression/i);
    expect(errors.length).toBeGreaterThan(0);
    expect(notify.notifyError).not.toHaveBeenCalled();
  });

  it('blocks saving a schedule whose cron does not parse', async () => {
    const user = userEvent.setup();
    vi.mocked(api.nextRunTimes).mockRejectedValue(new Error('invalid cron expression: "nope"'));
    renderTab();

    await user.click(await screen.findByRole('button', { name: /add schedule/i }));

    await waitFor(() => expect(screen.getByRole('button', { name: /done/i })).toBeDisabled());
  });

  it('saves the committed file by default', async () => {
    const user = userEvent.setup();
    vi.mocked(api.saveTriggers).mockResolvedValue(undefined);
    renderTab();

    await user.click(await screen.findByRole('button', { name: /save triggers\.toml/i }));

    expect(api.saveTriggers).toHaveBeenCalledWith(REPO, CONFIG, false);
  });

  it('reads and writes the per-machine file when scoped to this machine', async () => {
    const user = userEvent.setup();
    vi.mocked(api.saveTriggers).mockResolvedValue(undefined);
    renderTab();

    await screen.findByText('nightly');
    await user.click(screen.getByRole('button', { name: /this machine/i }));
    await user.click(await screen.findByRole('button', { name: /save triggers\.local\.toml/i }));

    expect(api.loadTriggersLocal).toHaveBeenCalledWith(REPO);
    expect(api.saveTriggers).toHaveBeenCalledWith(REPO, CONFIG, true);
  });
});

describe('TriggersTab git hooks', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(api.nextRunTimes).mockResolvedValue(NEXT_TIMES);
  });

  it('offers the snippet plus Append and Force when a foreign hook is left alone', async () => {
    const user = userEvent.setup();
    vi.mocked(api.installGitHooks).mockResolvedValue(FOREIGN_REPORT);
    renderTab();

    await screen.findByText('pre-push');
    await user.click(screen.getAllByRole('button', { name: /^install$/i })[0]);

    expect(await screen.findByText(/You already have your own pre-push hook/)).toBeInTheDocument();
    expect(screen.getByText(FOREIGN_REPORT.message)).toBeInTheDocument();
    expect(screen.getByText(/chibby run --stages test/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /append to my hook/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /back up & replace/i })).toBeInTheDocument();
  });

  it('does not read a left-alone hook as a failure', async () => {
    const user = userEvent.setup();
    vi.mocked(api.installGitHooks).mockResolvedValue(FOREIGN_REPORT);
    renderTab();

    await screen.findByText('pre-push');
    await user.click(screen.getAllByRole('button', { name: /^install$/i })[0]);

    await screen.findByText(/You already have your own pre-push hook/);
    expect(notify.notifyError).not.toHaveBeenCalled();
    expect(notify.notifySuccess).not.toHaveBeenCalled();
    expect(document.querySelector('.alert-error')).toBeNull();
  });

  it('re-installs with append mode when the user picks Append', async () => {
    const user = userEvent.setup();
    vi.mocked(api.installGitHooks).mockResolvedValue(FOREIGN_REPORT);
    renderTab();

    await screen.findByText('pre-push');
    await user.click(screen.getAllByRole('button', { name: /^install$/i })[0]);
    await user.click(await screen.findByRole('button', { name: /append to my hook/i }));

    expect(api.installGitHooks).toHaveBeenLastCalledWith(
      REPO,
      'pre_push',
      { stages: ['test'], blocking: true },
      'append'
    );
  });
});
