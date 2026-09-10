import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import StageCard from '../../components/project-detail/StageCard';
import type { Stage } from '../../types';
import type { StageStatus } from '../../services/runStore';

function createStage(overrides: Partial<Stage> = {}): Stage {
  return {
    name: 'build',
    commands: ['npm run build'],
    backend: 'local',
    fail_fast: true,
    ...overrides,
  };
}

function renderCard(stage: Stage, status?: StageStatus) {
  return render(
    <StageCard
      stage={stage}
      index={0}
      status={status}
      cmdStatuses={{}}
      liveOutput={{}}
      running={false}
      runs={[]}
      selectedStageResult={null}
      onSelectStageResult={vi.fn()}
      onRunStage={vi.fn()}
    />
  );
}

const TIMEOUT_TITLE = 'Timeout: 30s';
const WHEN_TITLE = 'Runs only when branch: main';
const ENV_TITLE = 'Stage-scoped environment variables';

describe('StageCard advanced badges', () => {
  it('renders timeout, retry, when and env badges when the fields are set', () => {
    const stage = createStage({
      timeout_secs: 30,
      retry: { attempts: 3, delay_secs: 5, backoff: 'fixed' },
      when: { branch: ['main'], branch_not: [], environment: [], environment_not: [] },
      env: { API_URL: 'https://x', LOG_LEVEL: 'debug' },
    });
    renderCard(stage);

    expect(screen.getByTitle(TIMEOUT_TITLE)).toHaveTextContent('30s');
    expect(screen.getByTitle('Retry: up to 3 attempts, 5s fixed backoff')).toHaveTextContent('3');
    expect(screen.getByTitle(WHEN_TITLE)).toHaveTextContent('when');
    expect(screen.getByTitle(ENV_TITLE)).toHaveTextContent('env ×2');
  });

  it('renders no advanced badges when the fields are absent', () => {
    renderCard(createStage());

    expect(screen.queryByTitle(TIMEOUT_TITLE)).not.toBeInTheDocument();
    expect(screen.queryByTitle(ENV_TITLE)).not.toBeInTheDocument();
    expect(screen.queryByText('when')).not.toBeInTheDocument();
    // The backend badge is always present, so the header is not simply empty.
    expect(screen.getByText('local')).toBeInTheDocument();
  });

  it('omits the when badge when the condition object is empty', () => {
    const stage = createStage({
      when: { branch: [], branch_not: [], environment: [], environment_not: [] },
    });
    renderCard(stage);

    expect(screen.queryByText('when')).not.toBeInTheDocument();
  });
});

describe('StageCard rollback badge', () => {
  const POLICY = { verify_health: true, notify: true, max_attempts: 1, window_mins: 60 };

  it('renders the badge when a rollback policy is active', () => {
    renderCard(createStage({ on_health_failure: { ...POLICY, mode: 'last_good' } }));

    expect(screen.getByTitle('On health check failure: roll back — last good')).toHaveTextContent(
      'rollback'
    );
  });

  it('renders the badge for the commands mode', () => {
    renderCard(
      createStage({
        on_health_failure: { ...POLICY, mode: 'commands' },
        rollback_commands: ['kubectl rollout undo deploy/api'],
      })
    );

    expect(screen.getByTitle('On health check failure: roll back — commands')).toBeInTheDocument();
  });

  it('omits the badge when the mode is off', () => {
    renderCard(createStage({ on_health_failure: { ...POLICY, mode: 'off' } }));

    expect(screen.queryByText('rollback')).not.toBeInTheDocument();
  });

  it('omits the badge when no policy is set', () => {
    renderCard(createStage());

    expect(screen.queryByText('rollback')).not.toBeInTheDocument();
  });
});

describe('StageCard timedout status', () => {
  it('renders a timed-out stage as a failure, not a success', () => {
    const { container } = renderCard(createStage(), 'timedout');
    const card = container.querySelector('.stage-card');

    expect(card).toHaveClass('stage-failed');
    expect(card).not.toHaveClass('stage-success');
    expect(card).not.toHaveClass('stage-pending');
  });

  it('treats a timed-out stage as having a viewable result', () => {
    const { container } = renderCard(createStage(), 'timedout');

    expect(container.querySelector('.stage-card')).toHaveClass('stage-clickable');
  });
});
