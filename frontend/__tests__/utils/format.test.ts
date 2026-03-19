import { describe, it, expect } from 'vitest';
import {
  formatDuration,
  formatDate,
  statusClass,
  capitalize,
  repoNameFromPath,
} from '../../utils/format';

describe('formatDuration', () => {
  it('returns -- for undefined', () => {
    expect(formatDuration(undefined)).toBe('--');
  });

  it('returns -- for null', () => {
    expect(formatDuration(null as unknown as undefined)).toBe('--');
  });

  it('formats milliseconds under 1 second', () => {
    expect(formatDuration(0)).toBe('0ms');
    expect(formatDuration(500)).toBe('500ms');
    expect(formatDuration(999)).toBe('999ms');
  });

  it('formats seconds under 1 minute', () => {
    expect(formatDuration(1000)).toBe('1s');
    expect(formatDuration(1500)).toBe('1s');
    expect(formatDuration(30000)).toBe('30s');
    expect(formatDuration(59999)).toBe('59s');
  });

  it('formats minutes and seconds', () => {
    expect(formatDuration(60000)).toBe('1m 0s');
    expect(formatDuration(65000)).toBe('1m 5s');
    expect(formatDuration(125000)).toBe('2m 5s');
    expect(formatDuration(3600000)).toBe('60m 0s');
  });
});

describe('formatDate', () => {
  it('returns -- for undefined', () => {
    expect(formatDate(undefined)).toBe('--');
  });

  it('returns -- for empty string', () => {
    expect(formatDate('')).toBe('--');
  });

  it('formats ISO date string', () => {
    const result = formatDate('2026-03-17T10:30:00Z');
    // Result varies by timezone, just check it's not the fallback
    expect(result).not.toBe('--');
    expect(result.length).toBeGreaterThan(5);
  });
});

describe('statusClass', () => {
  it('returns correct class for each status', () => {
    expect(statusClass('success')).toBe('success');
    expect(statusClass('failed')).toBe('failed');
    expect(statusClass('running')).toBe('running');
    expect(statusClass('pending')).toBe('pending');
    expect(statusClass('skipped')).toBe('skipped');
    expect(statusClass('cancelled')).toBe('cancelled');
  });

  it('returns pending for unknown status', () => {
    expect(statusClass('unknown' as never)).toBe('pending');
  });
});

describe('capitalize', () => {
  it('capitalizes first character', () => {
    expect(capitalize('hello')).toBe('Hello');
    expect(capitalize('world')).toBe('World');
  });

  it('handles single character', () => {
    expect(capitalize('a')).toBe('A');
  });

  it('handles empty string', () => {
    expect(capitalize('')).toBe('');
  });

  it('handles already capitalized', () => {
    expect(capitalize('Hello')).toBe('Hello');
  });
});

describe('repoNameFromPath', () => {
  it('extracts name from Unix path', () => {
    expect(repoNameFromPath('/Users/dev/projects/my-app')).toBe('my-app');
    expect(repoNameFromPath('/home/user/code/project')).toBe('project');
  });

  it('extracts name from Windows path', () => {
    expect(repoNameFromPath('C:\\Users\\dev\\projects\\my-app')).toBe('my-app');
  });

  it('returns path for trailing slash (edge case)', () => {
    // Current behavior: falls back to original path when last segment is empty
    expect(repoNameFromPath('/Users/dev/my-app/')).toBe('/Users/dev/my-app/');
  });

  it('handles single segment path', () => {
    expect(repoNameFromPath('my-app')).toBe('my-app');
  });
});
