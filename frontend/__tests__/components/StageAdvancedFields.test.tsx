import { useState } from 'react';
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import StageAdvancedFields from '../../components/pipeline-editor/StageAdvancedFields';
import type { Stage } from '../../types';

function createStage(overrides: Partial<Stage> = {}): Stage {
  return {
    name: 'deploy',
    commands: ['./deploy.sh'],
    backend: 'local',
    fail_fast: true,
    ...overrides,
  };
}

const HEALTH_CHECK = { command: 'curl -sf http://localhost/health', retries: 3, delay_secs: 5 };
const DEAD_CONFIG_HINT = /no health check, so the rollback will never fire/;

/** The block is collapsed by default; open it so the fields are in the DOM. */
function renderFields(stage: Stage) {
  const onChange = vi.fn();
  const view = render(<StageAdvancedFields stage={stage} onChange={onChange} />);
  fireEvent.click(screen.getByText('Advanced'));
  return { ...view, onChange };
}

describe('StageAdvancedFields rollback policy', () => {
  it('defaults the mode to off and hides the policy fields', () => {
    renderFields(createStage());

    expect(screen.getByLabelText('On health check failure')).toHaveValue('off');
    expect(screen.queryByLabelText('Max attempts')).not.toBeInTheDocument();
    expect(screen.queryByText(DEAD_CONFIG_HINT)).not.toBeInTheDocument();
  });

  it('writes the Rust serde defaults when a mode is selected', () => {
    const { onChange } = renderFields(createStage({ health_check: HEALTH_CHECK }));

    fireEvent.change(screen.getByLabelText('On health check failure'), {
      target: { value: 'last_good' },
    });

    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({
        on_health_failure: {
          mode: 'last_good',
          verify_health: true,
          notify: true,
          max_attempts: 1,
          window_mins: 60,
        },
      })
    );
  });

  it('clears the policy when the mode goes back to off', () => {
    const stage = createStage({
      health_check: HEALTH_CHECK,
      on_health_failure: {
        mode: 'last_good',
        verify_health: true,
        notify: true,
        max_attempts: 1,
        window_mins: 60,
      },
    });
    const { onChange } = renderFields(stage);

    fireEvent.change(screen.getByLabelText('On health check failure'), {
      target: { value: 'off' },
    });

    expect(onChange).toHaveBeenCalledWith({ on_health_failure: undefined });
  });

  it('warns that a rollback on a stage without a health check will never fire', () => {
    renderFields(
      createStage({
        on_health_failure: {
          mode: 'last_good',
          verify_health: true,
          notify: true,
          max_attempts: 1,
          window_mins: 60,
        },
      })
    );

    expect(screen.getByText(DEAD_CONFIG_HINT)).toBeInTheDocument();
  });

  it('drops the warning once the stage has a health check', () => {
    renderFields(
      createStage({
        health_check: HEALTH_CHECK,
        on_health_failure: {
          mode: 'last_good',
          verify_health: true,
          notify: true,
          max_attempts: 1,
          window_mins: 60,
        },
      })
    );

    expect(screen.queryByText(DEAD_CONFIG_HINT)).not.toBeInTheDocument();
    expect(screen.getByLabelText('Max attempts')).toHaveValue(1);
  });

  it('shows the rollback command editor only in commands mode', () => {
    renderFields(
      createStage({
        health_check: HEALTH_CHECK,
        on_health_failure: {
          mode: 'commands',
          verify_health: true,
          notify: true,
          max_attempts: 1,
          window_mins: 60,
        },
        rollback_commands: ['kubectl rollout undo deploy/api'],
      })
    );

    expect(screen.getByText('Rollback Commands')).toBeInTheDocument();
    expect(screen.getByDisplayValue('kubectl rollout undo deploy/api')).toBeInTheDocument();
  });
});

/** Stateful host: the parent feeding edits back is what remounts a keyed row. */
function EnvHost({ initial }: { initial: Stage }) {
  const [stage, setStage] = useState(initial);
  return (
    <StageAdvancedFields stage={stage} onChange={(patch) => setStage({ ...stage, ...patch })} />
  );
}

describe('StageAdvancedFields stage environment', () => {
  it('keeps focus while a variable name is typed', async () => {
    const user = userEvent.setup();
    render(<EnvHost initial={createStage({ env: { VAR_1: '' } })} />);
    await user.click(screen.getByText('Advanced'));

    const name = screen.getByLabelText('Variable 1 name');
    await user.clear(name);
    await user.type(name, 'API_URL');

    expect(screen.getByLabelText('Variable 1 name')).toHaveValue('API_URL');
  });

  it('edits the row at a position, not the row with a name', async () => {
    const user = userEvent.setup();
    render(<EnvHost initial={createStage({ env: { A: '1', B: '2' } })} />);
    await user.click(screen.getByText('Advanced'));

    await user.type(screen.getByLabelText('Variable 2 value'), '3');

    expect(screen.getByLabelText('Variable 1 name')).toHaveValue('A');
    expect(screen.getByLabelText('Variable 2 value')).toHaveValue('23');
  });
});
