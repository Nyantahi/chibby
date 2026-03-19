import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import LogViewer from '../../components/LogViewer';
import type { StageResult } from '../../types';

function createStageResult(overrides: Partial<StageResult> = {}): StageResult {
  return {
    stage_name: 'test-stage',
    status: 'success',
    stdout: '',
    stderr: '',
    ...overrides,
  };
}

describe('LogViewer', () => {
  it('renders stage name', () => {
    const stage = createStageResult({ stage_name: 'Build' });
    render(<LogViewer stage={stage} />);

    expect(screen.getByText('Build')).toBeInTheDocument();
  });

  it('renders status badge', () => {
    const stage = createStageResult({ status: 'success' });
    render(<LogViewer stage={stage} />);

    expect(screen.getByText('Success')).toBeInTheDocument();
  });

  it('renders failed status', () => {
    const stage = createStageResult({ status: 'failed' });
    render(<LogViewer stage={stage} />);

    expect(screen.getByText('Failed')).toBeInTheDocument();
  });

  it('renders running status', () => {
    const stage = createStageResult({ status: 'running' });
    render(<LogViewer stage={stage} />);

    expect(screen.getByText('Running')).toBeInTheDocument();
  });

  it('renders exit code when present', () => {
    const stage = createStageResult({ exit_code: 0 });
    render(<LogViewer stage={stage} />);

    expect(screen.getByText('Exit: 0')).toBeInTheDocument();
  });

  it('renders non-zero exit code', () => {
    const stage = createStageResult({ exit_code: 1 });
    render(<LogViewer stage={stage} />);

    expect(screen.getByText('Exit: 1')).toBeInTheDocument();
  });

  it('renders duration when present', () => {
    const stage = createStageResult({ duration_ms: 5000 });
    render(<LogViewer stage={stage} />);

    expect(screen.getByText('5s')).toBeInTheDocument();
  });

  it('renders stdout content', () => {
    const stage = createStageResult({ stdout: 'Build successful\nAll tests passed' });
    render(<LogViewer stage={stage} />);

    expect(screen.getByText('Build successful')).toBeInTheDocument();
    expect(screen.getByText('All tests passed')).toBeInTheDocument();
  });

  it('renders stderr content', () => {
    const stage = createStageResult({ stderr: 'Warning: deprecated function' });
    render(<LogViewer stage={stage} />);

    expect(screen.getByText('Warning: deprecated function')).toBeInTheDocument();
  });

  it('strips ANSI escape codes from output', () => {
    const stage = createStageResult({
      stdout: '\x1b[32mSUCCESS\x1b[0m Build completed',
    });
    render(<LogViewer stage={stage} />);

    expect(screen.getByText('SUCCESS Build completed')).toBeInTheDocument();
  });

  it('shows empty message when no output', () => {
    const stage = createStageResult({ stdout: '', stderr: '' });
    render(<LogViewer stage={stage} />);

    expect(screen.getByText('No output recorded.')).toBeInTheDocument();
  });

  it('filters empty lines from output', () => {
    const stage = createStageResult({
      stdout: 'Line 1\n\n\nLine 2',
    });
    const { container } = render(<LogViewer stage={stage} />);

    const lines = container.querySelectorAll('.log-line-stdout');
    expect(lines).toHaveLength(2);
  });
});
