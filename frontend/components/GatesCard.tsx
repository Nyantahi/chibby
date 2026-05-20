import { useEffect, useState } from 'react';
import {
  Shield,
  Loader2,
  ShieldCheck,
  AlertTriangle,
  ChevronDown,
  ChevronRight,
  Copy,
  X,
} from 'lucide-react';
import {
  createSecretScanBaseline,
  loadGatesConfig,
  runCommitLint,
  runContainerScan,
  runDependencyAudit,
  runGates,
  runIacScan,
  runLicenseCheck,
  runSast,
  runSecretScan,
  saveGatesConfig,
} from '../services/api';
import { notifyError, notifySuccess } from '../services/notify';
import type { GateMode, GatesConfig, GatesResult } from '../types';

interface Props {
  repoPath: string;
}

const MODES: GateMode[] = ['block', 'warn', 'off'];

type ScanStatus = 'pass' | 'fail' | 'info';

interface ScanEntry {
  /** Stable id per gate so reruns replace the prior entry. */
  id: string;
  label: string;
  status: ScanStatus;
  /** Single-line summary shown on the collapsed header. */
  summary: string;
  /** Full body shown when expanded (and what Copy puts on the clipboard). */
  body: string;
  /** Whether this entry is currently expanded. */
  expanded: boolean;
  /** Wall-clock time the entry was produced. */
  at: number;
}

function GatesCard({ repoPath }: Props) {
  const [cfg, setCfg] = useState<GatesConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [running, setRunning] = useState<string | null>(null);
  /** Ordered most-recent-first; reruns replace by id. */
  const [entries, setEntries] = useState<ScanEntry[]>([]);

  function upsertEntry(entry: Omit<ScanEntry, 'at' | 'expanded'> & { expanded?: boolean }) {
    setEntries((prev) => {
      const at = Date.now();
      const next = prev.filter((e) => e.id !== entry.id);
      next.unshift({ expanded: entry.expanded ?? true, at, ...entry });
      return next;
    });
  }

  function toggleEntry(id: string) {
    setEntries((prev) => prev.map((e) => (e.id === id ? { ...e, expanded: !e.expanded } : e)));
  }

  function removeEntry(id: string) {
    setEntries((prev) => prev.filter((e) => e.id !== id));
  }

  function copyEntry(entry: ScanEntry) {
    const text = `# ${entry.label}\n# ${entry.summary}\n\n${entry.body}`;
    navigator.clipboard
      .writeText(text)
      .then(() => notifySuccess('Copied to clipboard'))
      .catch((err) => notifyError('Copy failed', err));
  }

  useEffect(() => {
    loadGatesConfig(repoPath)
      .then(setCfg)
      .catch((err) => notifyError('Load gates failed', err))
      .finally(() => setLoading(false));
  }, [repoPath]);

  function updateMode<K extends keyof GatesConfig>(key: K, value: GatesConfig[K]) {
    setCfg((p) => (p ? { ...p, [key]: value } : p));
  }

  async function handleSave() {
    if (!cfg) return;
    setSaving(true);
    try {
      await saveGatesConfig(repoPath, cfg);
      notifySuccess('Gates config saved');
    } catch (err) {
      notifyError('Save failed', err);
    } finally {
      setSaving(false);
    }
  }

  function statusOf(passed: boolean): ScanStatus {
    return passed ? 'pass' : 'fail';
  }

  async function handleRunAll() {
    setRunning('all');
    try {
      const r: GatesResult = await runGates(repoPath);
      upsertEntry({
        id: 'all',
        label: 'Run all gates',
        status: statusOf(r.passed),
        summary: r.passed
          ? 'All enabled gates passed'
          : 'One or more gates failed — expand for details',
        body: JSON.stringify(r, null, 2),
      });
      if (r.passed) notifySuccess('Gates passed');
    } catch (err) {
      notifyError('Run gates failed', err);
    } finally {
      setRunning(null);
    }
  }

  async function handleRunSecretScan() {
    setRunning('secret');
    try {
      const r = await runSecretScan(repoPath);
      upsertEntry({
        id: 'secret',
        label: 'Secret scan',
        status: statusOf(r.passed),
        summary: `${r.findings.length} finding(s) via ${r.scanner}`,
        body: `${r.message}\n\n${JSON.stringify(r, null, 2)}`,
      });
    } catch (err) {
      notifyError('Secret scan failed', err);
    } finally {
      setRunning(null);
    }
  }

  async function handleRunAudit() {
    setRunning('audit');
    try {
      const r = await runDependencyAudit(repoPath);
      upsertEntry({
        id: 'audit',
        label: 'Dependency audit',
        status: statusOf(r.passed),
        summary: `${r.findings.length} finding(s) via ${r.scanner}`,
        body: `${r.message}\n\n${JSON.stringify(r, null, 2)}`,
      });
    } catch (err) {
      notifyError('Dependency audit failed', err);
    } finally {
      setRunning(null);
    }
  }

  async function handleRunCommitLint() {
    setRunning('commit');
    try {
      const r = await runCommitLint(repoPath);
      upsertEntry({
        id: 'commit',
        label: 'Commit lint',
        status: statusOf(r.passed),
        summary: `${r.violations.length} violation(s) across ${r.commits_checked} commits`,
        body: `${r.message}\n\n${JSON.stringify(r, null, 2)}`,
      });
    } catch (err) {
      notifyError('Commit lint failed', err);
    } finally {
      setRunning(null);
    }
  }

  async function handleBaseline() {
    setRunning('baseline');
    try {
      const msg = await createSecretScanBaseline(repoPath);
      upsertEntry({
        id: 'baseline',
        label: 'Secret-scan baseline',
        status: 'info',
        summary: msg,
        body: msg,
      });
      notifySuccess('Baseline created', msg);
    } catch (err) {
      notifyError('Baseline failed', err);
    } finally {
      setRunning(null);
    }
  }

  async function handleRunSast() {
    setRunning('sast');
    try {
      const r = await runSast(repoPath);
      upsertEntry({
        id: 'sast',
        label: 'SAST (semgrep)',
        status: statusOf(r.passed),
        summary: `${r.findings.length} finding(s) via ${r.scanner}`,
        body: `${r.message}\n\n${JSON.stringify(r, null, 2)}`,
      });
    } catch (err) {
      notifyError('SAST failed', err);
    } finally {
      setRunning(null);
    }
  }

  async function handleRunContainer() {
    setRunning('container');
    try {
      const r = await runContainerScan(repoPath);
      upsertEntry({
        id: 'container',
        label: 'Container scan',
        status: statusOf(r.passed),
        summary: `${r.findings.length} finding(s) via ${r.scanner}, ${r.targets.length} target(s)`,
        body: `${r.message}\nTargets: ${r.targets.join(', ') || '(none)'}\n\n${JSON.stringify(r, null, 2)}`,
      });
    } catch (err) {
      notifyError('Container scan failed', err);
    } finally {
      setRunning(null);
    }
  }

  async function handleRunIac() {
    setRunning('iac');
    try {
      const r = await runIacScan(repoPath);
      upsertEntry({
        id: 'iac',
        label: 'IaC scan',
        status: statusOf(r.passed),
        summary: `${r.findings.length} finding(s) via ${r.scanner}`,
        body: `${r.message}\n\n${JSON.stringify(r, null, 2)}`,
      });
    } catch (err) {
      notifyError('IaC scan failed', err);
    } finally {
      setRunning(null);
    }
  }

  async function handleRunLicense() {
    setRunning('license');
    try {
      const r = await runLicenseCheck(repoPath);
      upsertEntry({
        id: 'license',
        label: 'License check',
        status: statusOf(r.passed),
        summary: `${r.findings.length} finding(s) via ${r.scanner}`,
        body: `${r.message}\n\n${JSON.stringify(r, null, 2)}`,
      });
    } catch (err) {
      notifyError('License check failed', err);
    } finally {
      setRunning(null);
    }
  }

  return (
    <div className="feature-card">
      <div className="feature-card-header">
        <div className="feature-card-title">
          <Shield size={16} /> Security &amp; Quality Gates
        </div>
        <div className="feature-card-actions">
          <button className="btn btn-sm btn-secondary" onClick={handleSave} disabled={saving}>
            {saving ? 'Saving…' : 'Save'}
          </button>
          <button
            className="btn btn-sm btn-primary"
            onClick={handleRunAll}
            disabled={running !== null}
          >
            {running === 'all' ? 'Running…' : 'Run all'}
          </button>
        </div>
      </div>
      <div className="feature-card-body">
        {loading ? (
          <div className="feature-card-empty">
            <Loader2 size={14} className="spin" /> Loading…
          </div>
        ) : cfg ? (
          <>
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 8 }}>
              <div className="form-row">
                <label>Secret scanning</label>
                <select
                  className="input"
                  value={cfg.secret_scanning}
                  onChange={(e) => updateMode('secret_scanning', e.target.value as GateMode)}
                >
                  {MODES.map((m) => (
                    <option key={m} value={m}>
                      {m}
                    </option>
                  ))}
                </select>
              </div>
              <div className="form-row">
                <label>Dependency scanning</label>
                <select
                  className="input"
                  value={cfg.dependency_scanning}
                  onChange={(e) => updateMode('dependency_scanning', e.target.value as GateMode)}
                >
                  {MODES.map((m) => (
                    <option key={m} value={m}>
                      {m}
                    </option>
                  ))}
                </select>
              </div>
              <div className="form-row">
                <label>Commit lint</label>
                <select
                  className="input"
                  value={cfg.commit_lint}
                  onChange={(e) => updateMode('commit_lint', e.target.value as GateMode)}
                >
                  {MODES.map((m) => (
                    <option key={m} value={m}>
                      {m}
                    </option>
                  ))}
                </select>
              </div>
              <div className="form-row">
                <label>SAST (semgrep)</label>
                <select
                  className="input"
                  value={cfg.sast}
                  onChange={(e) => updateMode('sast', e.target.value as GateMode)}
                >
                  {MODES.map((m) => (
                    <option key={m} value={m}>
                      {m}
                    </option>
                  ))}
                </select>
              </div>
              <div className="form-row">
                <label>Container scan</label>
                <select
                  className="input"
                  value={cfg.container_scan}
                  onChange={(e) => updateMode('container_scan', e.target.value as GateMode)}
                >
                  {MODES.map((m) => (
                    <option key={m} value={m}>
                      {m}
                    </option>
                  ))}
                </select>
              </div>
              <div className="form-row">
                <label>IaC scan</label>
                <select
                  className="input"
                  value={cfg.iac_scan}
                  onChange={(e) => updateMode('iac_scan', e.target.value as GateMode)}
                >
                  {MODES.map((m) => (
                    <option key={m} value={m}>
                      {m}
                    </option>
                  ))}
                </select>
              </div>
              <div className="form-row">
                <label>License check</label>
                <select
                  className="input"
                  value={cfg.license_check}
                  onChange={(e) => updateMode('license_check', e.target.value as GateMode)}
                >
                  {MODES.map((m) => (
                    <option key={m} value={m}>
                      {m}
                    </option>
                  ))}
                </select>
              </div>
            </div>

            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 8 }}>
              <div className="form-row">
                <label>Dep audit threshold</label>
                <input
                  className="input"
                  value={cfg.audit_severity_threshold}
                  onChange={(e) => updateMode('audit_severity_threshold', e.target.value)}
                  placeholder="low|medium|high|critical"
                />
              </div>
              <div className="form-row">
                <label>SAST threshold</label>
                <input
                  className="input"
                  value={cfg.sast_severity_threshold}
                  onChange={(e) => updateMode('sast_severity_threshold', e.target.value)}
                  placeholder="low|medium|high|critical"
                />
              </div>
              <div className="form-row">
                <label>Container threshold</label>
                <input
                  className="input"
                  value={cfg.container_severity_threshold}
                  onChange={(e) => updateMode('container_severity_threshold', e.target.value)}
                  placeholder="low|medium|high|critical"
                />
              </div>
            </div>

            <div className="form-row">
              <label>Container images (one per line; falls back to detected Dockerfiles)</label>
              <textarea
                className="input"
                rows={2}
                value={cfg.container_images.join('\n')}
                onChange={(e) =>
                  updateMode(
                    'container_images',
                    e.target.value
                      .split('\n')
                      .map((s) => s.trim())
                      .filter(Boolean)
                  )
                }
                placeholder="ghcr.io/your-org/your-app:tag"
              />
            </div>

            <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
              <button
                className="btn btn-sm btn-ghost"
                onClick={handleRunSecretScan}
                disabled={running !== null}
              >
                {running === 'secret' ? '…' : 'Secret scan'}
              </button>
              <button
                className="btn btn-sm btn-ghost"
                onClick={handleRunAudit}
                disabled={running !== null}
              >
                {running === 'audit' ? '…' : 'Dependency audit'}
              </button>
              <button
                className="btn btn-sm btn-ghost"
                onClick={handleRunCommitLint}
                disabled={running !== null}
              >
                {running === 'commit' ? '…' : 'Commit lint'}
              </button>
              <button
                className="btn btn-sm btn-ghost"
                onClick={handleRunSast}
                disabled={running !== null}
              >
                {running === 'sast' ? '…' : 'SAST (semgrep)'}
              </button>
              <button
                className="btn btn-sm btn-ghost"
                onClick={handleRunContainer}
                disabled={running !== null}
              >
                {running === 'container' ? '…' : 'Container scan'}
              </button>
              <button
                className="btn btn-sm btn-ghost"
                onClick={handleRunIac}
                disabled={running !== null}
              >
                {running === 'iac' ? '…' : 'IaC scan'}
              </button>
              <button
                className="btn btn-sm btn-ghost"
                onClick={handleRunLicense}
                disabled={running !== null}
              >
                {running === 'license' ? '…' : 'License check'}
              </button>
              <button
                className="btn btn-sm btn-ghost"
                onClick={handleBaseline}
                disabled={running !== null}
              >
                {running === 'baseline' ? '…' : 'Create secret-scan baseline'}
              </button>
            </div>

            {entries.length > 0 && (
              <div className="gates-results">
                <div className="gates-results-header">
                  <strong style={{ fontSize: 'var(--font-size-sm)' }}>
                    Results ({entries.length})
                  </strong>
                  <button
                    className="btn btn-xs btn-ghost"
                    onClick={() => setEntries([])}
                    title="Clear all results"
                  >
                    Clear
                  </button>
                </div>
                {entries.map((e) => {
                  const color =
                    e.status === 'pass'
                      ? 'var(--color-success)'
                      : e.status === 'fail'
                        ? 'var(--color-failed)'
                        : 'var(--color-text-muted)';
                  const Icon = e.status === 'pass' ? ShieldCheck : AlertTriangle;
                  return (
                    <div key={e.id} className="gates-result-card">
                      <button
                        type="button"
                        className="gates-result-header"
                        onClick={() => toggleEntry(e.id)}
                        aria-expanded={e.expanded}
                      >
                        {e.expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                        <Icon size={14} style={{ color }} />
                        <span className="gates-result-label" style={{ color }}>
                          {e.label}
                        </span>
                        <span className="gates-result-summary">{e.summary}</span>
                        <span className="gates-result-actions">
                          <span
                            role="button"
                            tabIndex={0}
                            className="btn-icon btn-xs"
                            title="Copy result"
                            onClick={(ev) => {
                              ev.stopPropagation();
                              copyEntry(e);
                            }}
                            onKeyDown={(ev) => {
                              if (ev.key === 'Enter' || ev.key === ' ') {
                                ev.preventDefault();
                                copyEntry(e);
                              }
                            }}
                          >
                            <Copy size={12} />
                          </span>
                          <span
                            role="button"
                            tabIndex={0}
                            className="btn-icon btn-xs"
                            title="Dismiss"
                            onClick={(ev) => {
                              ev.stopPropagation();
                              removeEntry(e.id);
                            }}
                            onKeyDown={(ev) => {
                              if (ev.key === 'Enter' || ev.key === ' ') {
                                ev.preventDefault();
                                removeEntry(e.id);
                              }
                            }}
                          >
                            <X size={12} />
                          </span>
                        </span>
                      </button>
                      {e.expanded && <pre className="gates-result-body">{e.body}</pre>}
                    </div>
                  );
                })}
              </div>
            )}
          </>
        ) : null}
      </div>
    </div>
  );
}

export default GatesCard;
