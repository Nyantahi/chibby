import { useEffect, useState } from 'react';
import { Shield, Loader2, ShieldCheck, AlertTriangle } from 'lucide-react';
import {
  createSecretScanBaseline,
  loadGatesConfig,
  runCommitLint,
  runDependencyAudit,
  runGates,
  runSecretScan,
  saveGatesConfig,
} from '../services/api';
import { notifyError, notifySuccess } from '../services/notify';
import type { GateMode, GatesConfig, GatesResult } from '../types';

interface Props {
  repoPath: string;
}

const MODES: GateMode[] = ['block', 'warn', 'off'];

function GatesCard({ repoPath }: Props) {
  const [cfg, setCfg] = useState<GatesConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [running, setRunning] = useState<string | null>(null);
  const [result, setResult] = useState<GatesResult | null>(null);
  const [single, setSingle] = useState<string | null>(null);

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

  async function handleRunAll() {
    setRunning('all');
    setResult(null);
    setSingle(null);
    try {
      const r = await runGates(repoPath);
      setResult(r);
      if (r.passed) notifySuccess('Gates passed');
    } catch (err) {
      notifyError('Run gates failed', err);
    } finally {
      setRunning(null);
    }
  }

  async function handleRunSecretScan() {
    setRunning('secret');
    setSingle(null);
    try {
      const r = await runSecretScan(repoPath);
      setSingle(`Secret scan: ${r.passed ? 'PASS' : 'FAIL'} (${r.findings.length} findings via ${r.scanner})\n${r.message}`);
    } catch (err) {
      notifyError('Secret scan failed', err);
    } finally {
      setRunning(null);
    }
  }

  async function handleRunAudit() {
    setRunning('audit');
    setSingle(null);
    try {
      const r = await runDependencyAudit(repoPath);
      setSingle(`Dependency audit: ${r.passed ? 'PASS' : 'FAIL'} (${r.findings.length} findings via ${r.scanner})\n${r.message}`);
    } catch (err) {
      notifyError('Dependency audit failed', err);
    } finally {
      setRunning(null);
    }
  }

  async function handleRunCommitLint() {
    setRunning('commit');
    setSingle(null);
    try {
      const r = await runCommitLint(repoPath);
      setSingle(`Commit lint: ${r.passed ? 'PASS' : 'FAIL'} (${r.violations.length} violations across ${r.commits_checked} commits)\n${r.message}`);
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
      notifySuccess('Baseline created', msg);
    } catch (err) {
      notifyError('Baseline failed', err);
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
          <button className="btn btn-sm btn-primary" onClick={handleRunAll} disabled={running !== null}>
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
            </div>

            <div className="form-row">
              <label>Audit severity threshold</label>
              <input
                className="input"
                value={cfg.audit_severity_threshold}
                onChange={(e) => updateMode('audit_severity_threshold', e.target.value)}
                placeholder="low|medium|high|critical"
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
                onClick={handleBaseline}
                disabled={running !== null}
              >
                {running === 'baseline' ? '…' : 'Create secret-scan baseline'}
              </button>
            </div>

            {result && (
              <div
                style={{
                  marginTop: 12,
                  border: '1px solid var(--color-border)',
                  borderRadius: 'var(--radius-sm)',
                  padding: 'var(--space-md)',
                }}
              >
                <strong
                  style={{
                    display: 'flex',
                    gap: 6,
                    alignItems: 'center',
                    color: result.passed ? 'var(--color-success)' : 'var(--color-failed)',
                  }}
                >
                  {result.passed ? <ShieldCheck size={14} /> : <AlertTriangle size={14} />}
                  {result.passed ? 'Passed' : 'Failed'}
                </strong>
                <pre style={{ marginTop: 8, fontSize: 'var(--font-size-2xs)', whiteSpace: 'pre-wrap' }}>
                  {JSON.stringify(result, null, 2)}
                </pre>
              </div>
            )}

            {single && (
              <pre
                style={{
                  marginTop: 8,
                  padding: 'var(--space-md)',
                  fontSize: 'var(--font-size-xs)',
                  whiteSpace: 'pre-wrap',
                  border: '1px solid var(--color-border)',
                  borderRadius: 'var(--radius-sm)',
                  background: 'var(--color-surface)',
                }}
              >
                {single}
              </pre>
            )}
          </>
        ) : null}
      </div>
    </div>
  );
}

export default GatesCard;
