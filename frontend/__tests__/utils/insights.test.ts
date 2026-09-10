import { describe, it, expect } from 'vitest';
import {
  deltaTone,
  formatBytes,
  formatDeltaPct,
  formatDeltaPoints,
  formatDay,
  formatMs,
  formatRate,
  NO_VALUE,
  rollbackClass,
  shortCommit,
  shortRunId,
} from '../../utils/insights';

describe('insights formatting', () => {
  it('renders a rate as a percentage and guards against a missing one', () => {
    expect(formatRate(0.8)).toBe('80.0%');
    expect(formatRate(0)).toBe('0.0%');
    expect(formatRate(null)).toBe(NO_VALUE);
    expect(formatRate(Number.NaN)).toBe(NO_VALUE);
  });

  // A null delta means the previous window had no runs. It has to read as a
  // dash: "+0.0" would claim an improvement that never happened.
  it('renders a null delta as a dash, never as zero', () => {
    expect(formatDeltaPoints(null)).toBe(NO_VALUE);
    expect(formatDeltaPct(null)).toBe(NO_VALUE);
    expect(formatDeltaPoints(null)).not.toContain('0.0');
    expect(formatDeltaPct(null)).not.toContain('0.0');
  });

  it('signs a known delta', () => {
    expect(formatDeltaPoints(12.34)).toBe('+12.3 pts');
    expect(formatDeltaPoints(-5)).toBe('-5.0 pts');
    expect(formatDeltaPct(25)).toBe('+25.0%');
    expect(formatDeltaPct(0)).toBe('0.0%');
  });

  it('treats a rising success rate as an improvement', () => {
    expect(deltaTone(10)).toBe('success');
    expect(deltaTone(-10)).toBe('failed');
  });

  // Duration inverts: slower is worse, so a positive delta must not read green.
  it('inverts the sign convention when lower is better', () => {
    expect(deltaTone(25, true)).toBe('failed');
    expect(deltaTone(-25, true)).toBe('success');
  });

  it('gives an unknown or unchanged delta no colour', () => {
    expect(deltaTone(null)).toBe('neutral');
    expect(deltaTone(null, true)).toBe('neutral');
    expect(deltaTone(0)).toBe('neutral');
  });

  it('formats a missing duration as a dash rather than zero', () => {
    expect(formatMs(null)).toBe(NO_VALUE);
    expect(formatMs(1500)).toBe('1s');
  });

  it('formats bytes in human-readable units', () => {
    expect(formatBytes(512)).toBe('512 B');
    expect(formatBytes(2048)).toBe('2.0 KB');
    expect(formatBytes(5 * 1024 * 1024)).toBe('5.0 MB');
    expect(formatBytes(null)).toBe(NO_VALUE);
  });

  it('reads an index date in local time without sliding a day', () => {
    expect(formatDay('2026-09-10')).toContain('10');
    expect(formatDay('not-a-date')).toBe('not-a-date');
  });

  it('shortens ids and commits', () => {
    expect(shortRunId('aaaaaaaa-1111-2222-3333-444444444444')).toBe('aaaaaaaa');
    expect(shortCommit('abcdef1234567')).toBe('abcdef1');
    expect(shortCommit(null)).toBe(NO_VALUE);
  });

  it('maps a rollback outcome to a badge tone', () => {
    expect(rollbackClass('succeeded')).toBe('success');
    expect(rollbackClass('failed')).toBe('failed');
    expect(rollbackClass('skipped')).toBe('neutral');
  });
});
