import { describe, it, expect, vi, beforeEach } from 'vitest';
import { startRun, clearRun, getActiveRun } from '../../services/runStore';
import * as api from '../../services/api';
import type { Pipeline, PipelineRun } from '../../types';

vi.mock('../../services/api');

const PIPELINE: Pipeline = {
  name: 'ci',
  stages: [{ name: 'build', commands: ['npm ci'], backend: 'local', fail_fast: true }],
};

const REPO = '/tmp/proj-preflight';

const FINISHED_RUN: PipelineRun = {
  id: 'run-1',
  pipeline_name: 'ci',
  repo_path: REPO,
  status: 'success',
  stage_results: [],
  started_at: '2026-03-01T10:00:00Z',
};

describe('startRun preflight wiring', () => {
  beforeEach(() => {
    clearRun(REPO);
    vi.clearAllMocks();
    vi.mocked(api.runPipeline).mockResolvedValue(FINISHED_RUN);
  });

  it('runs preflight by default (skipPreflight false)', async () => {
    await startRun({ repoPath: REPO, pipeline: PIPELINE });

    expect(api.runPipeline).toHaveBeenCalledWith(REPO, undefined, undefined, undefined, false);
  });

  it('forwards an explicit skipPreflight along with the other run options', async () => {
    await startRun({
      repoPath: REPO,
      pipeline: PIPELINE,
      environment: 'production',
      stages: ['build'],
      pipelineFile: 'release',
      skipPreflight: true,
    });

    expect(api.runPipeline).toHaveBeenCalledWith(REPO, 'production', ['build'], 'release', true);
  });

  it('surfaces a preflight failure through the run error state', async () => {
    const message = "Preflight validation failed for environment 'production': missing secret";
    vi.mocked(api.runPipeline).mockRejectedValue(message);

    await expect(startRun({ repoPath: REPO, pipeline: PIPELINE })).rejects.toBe(message);
    // The store's own rejection handler runs a microtask later.
    await Promise.resolve();
    await Promise.resolve();

    const run = getActiveRun(REPO)!;
    expect(run.status).toBe('failed');
    expect(run.error).toContain('Preflight validation failed');
  });
});
