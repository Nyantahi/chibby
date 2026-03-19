import { useState } from 'react';
import { Copy, Check } from 'lucide-react';
import { capitalize, formatDuration } from '../utils/format';
import type { StageResult } from '../types';

interface LogViewerProps {
  stage: StageResult;
}

/** Strip ANSI escape sequences from a string. */
// eslint-disable-next-line no-control-regex
const ANSI_RE = /\x1b\[[0-9;]*[A-Za-z]|\x1b\].*?\x07/g;
function stripAnsi(text: string): string {
  return text.replace(ANSI_RE, '');
}

/** Terminal-style log viewer for a single stage. */
function LogViewer({ stage }: LogViewerProps) {
  const lines = buildLogLines(stage);
  const [copied, setCopied] = useState(false);

  async function handleCopy() {
    const text = lines.map((l) => l.text).join('\n');
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error('Failed to copy:', err);
    }
  }

  return (
    <div className="log-viewer">
      <div className="log-header">
        <strong>{stage.stage_name}</strong>
        <span className={`badge badge-${stage.status}`}>{capitalize(stage.status)}</span>
        <span className="log-duration">{formatDuration(stage.duration_ms)}</span>
        {stage.exit_code !== undefined && stage.exit_code !== null && (
          <span className="log-exit-code">Exit: {stage.exit_code}</span>
        )}
        <button
          className={`btn btn-icon btn-copy ${copied ? 'btn-copied' : ''}`}
          onClick={handleCopy}
          title={copied ? 'Copied!' : 'Copy output'}
          disabled={lines.length === 0}
        >
          {copied ? <Check size={14} /> : <Copy size={14} />}
        </button>
      </div>
      <pre className="log-output">
        {lines.map((line, i) => (
          <span key={i} className={`log-line log-line-${line.type}`}>
            {line.text}
            {'\n'}
          </span>
        ))}
        {lines.length === 0 && <span className="log-line log-line-info">No output recorded.</span>}
      </pre>
    </div>
  );
}

interface LogLine {
  type: 'stdout' | 'stderr' | 'warning' | 'info';
  text: string;
}

/** Patterns that indicate a line is a warning rather than an error. */
const WARNING_RE = /^warning[\s:[]/i;

function classifyStderrLine(text: string): 'warning' | 'stderr' {
  return WARNING_RE.test(text) ? 'warning' : 'stderr';
}

function buildLogLines(stage: StageResult): LogLine[] {
  const lines: LogLine[] = [];

  if (stage.stdout) {
    for (const l of stage.stdout.split('\n')) {
      const clean = stripAnsi(l);
      if (clean.trim()) {
        lines.push({ type: 'stdout', text: clean });
      }
    }
  }

  if (stage.stderr) {
    for (const l of stage.stderr.split('\n')) {
      const clean = stripAnsi(l);
      if (clean.trim()) {
        lines.push({ type: classifyStderrLine(clean), text: clean });
      }
    }
  }

  return lines;
}

export default LogViewer;
