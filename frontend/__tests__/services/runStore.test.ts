import { describe, it, expect, beforeEach } from 'vitest';
import {
  startRun,
  applyLogLine,
  getActiveRun,
  isRepoRunning,
  clearRun,
} from '../../services/runStore';
import type { Pipeline } from '../../types';

// A pipeline with two stages; build has 2 commands, deploy has 1.
const PIPELINE: Pipeline = {
  name: 'ci',
  stages: [
    { name: 'build', commands: ['npm ci', 'npm run build'], backend: 'local', fail_fast: true },
    { name: 'deploy', commands: ['./deploy.sh'], backend: 'local', fail_fast: true },
  ],
};

const REPO = '/tmp/proj-a';

function log(stage: string, type: string, message: string) {
  applyLogLine(REPO, { run_id: 'r1', repo_path: REPO, stage, type, message });
}

describe('runStore reducer', () => {
  beforeEach(() => {
    clearRun(REPO);
  });

  it('seeds pending stage and command statuses on startRun', () => {
    // startRun kicks off runPipeline (which rejects without a Tauri backend);
    // swallow it — we only assert the synchronous seeding.
    startRun({ repoPath: REPO, pipeline: PIPELINE }).catch(() => {});
    const run = getActiveRun(REPO)!;
    expect(run.status).toBe('running');
    expect(isRepoRunning(REPO)).toBe(true);
    expect(run.stageStatuses).toEqual({ build: 'pending', deploy: 'pending' });
    expect(run.cmdStatuses).toEqual({
      'build:0': 'pending',
      'build:1': 'pending',
      'deploy:0': 'pending',
    });
  });

  it('marks a stage running and progresses commands from log lines', () => {
    startRun({ repoPath: REPO, pipeline: PIPELINE }).catch(() => {});
    log('build', 'info', '--- Starting stage: build ---');
    log('build', 'cmd', 'npm ci');
    log('build', 'cmd', 'npm run build');

    const run = getActiveRun(REPO)!;
    expect(run.runningStageName).toBe('build');
    expect(run.stageStatuses.build).toBe('running');
    expect(run.cmdStatuses['build:0']).toBe('done'); // previous cmd marked done
    expect(run.cmdStatuses['build:1']).toBe('running');
  });

  it('finalizes a prior stage when the next stage starts', () => {
    startRun({ repoPath: REPO, pipeline: PIPELINE }).catch(() => {});
    log('build', 'info', '--- Starting stage: build ---');
    log('build', 'cmd', 'npm ci');
    log('deploy', 'info', '--- Starting stage: deploy ---');

    const run = getActiveRun(REPO)!;
    expect(run.stageStatuses.build).toBe('success');
    expect(run.cmdStatuses['build:0']).toBe('done');
    expect(run.cmdStatuses['build:1']).toBe('done'); // pending cmd finalized to done
    expect(run.stageStatuses.deploy).toBe('running');
  });

  it('marks the current command failed on an error line', () => {
    startRun({ repoPath: REPO, pipeline: PIPELINE }).catch(() => {});
    log('build', 'info', '--- Starting stage: build ---');
    log('build', 'cmd', 'npm ci');
    log('build', 'error', 'exit code 1');

    expect(getActiveRun(REPO)!.cmdStatuses['build:0']).toBe('failed');
  });

  it('captures and caps live output lines per stage', () => {
    startRun({ repoPath: REPO, pipeline: PIPELINE }).catch(() => {});
    log('build', 'info', '--- Starting stage: build ---');
    for (let i = 0; i < 60; i++) log('build', 'stdout', `line ${i}`);

    const lines = getActiveRun(REPO)!.liveOutput.build;
    expect(lines).toHaveLength(50); // MAX_LIVE_LINES
    expect(lines[lines.length - 1]).toBe('line 59');
  });

  it('ignores log lines for a repo with no active run', () => {
    applyLogLine('/tmp/unknown', {
      run_id: 'x',
      repo_path: '/tmp/unknown',
      stage: 'build',
      type: 'stdout',
      message: 'noise',
    });
    expect(getActiveRun('/tmp/unknown')).toBeUndefined();
  });
});
