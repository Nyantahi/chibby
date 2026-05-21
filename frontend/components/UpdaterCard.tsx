import { useEffect, useState } from 'react';
import {
  ArrowUpCircle,
  KeyRound,
  Loader2,
  Send,
  Copy,
  ShieldCheck,
  AlertTriangle,
} from 'lucide-react';
import {
  checkTauriCli,
  deleteUpdateKey,
  generateLatestJson,
  generateUpdateKeys,
  hasUpdateKey,
  importUpdatePrivateKey,
  loadUpdaterConfig,
  publishUpdate,
  rotateUpdateKeys,
  saveUpdaterConfig,
  updaterPreflight,
} from '../services/api';
import { notifyError, notifySuccess } from '../services/notify';
import type { UpdaterConfig, UpdatePublishTarget } from '../types';

interface Props {
  repoPath: string;
}

const TARGETS: UpdatePublishTarget[] = ['github_release', 's3', 'scp', 'local'];

function UpdaterCard({ repoPath }: Props) {
  const [cfg, setCfg] = useState<UpdaterConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [hasKey, setHasKey] = useState(false);
  const [cliVersion, setCliVersion] = useState<string>('');
  const [preflightIssues, setPreflightIssues] = useState<string[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [importingKey, setImportingKey] = useState(false);
  const [keyInput, setKeyInput] = useState('');
  const [publishVersion, setPublishVersion] = useState('0.0.0');
  const [dryRun, setDryRun] = useState(true);

  useEffect(() => {
    let cancelled = false;
    Promise.all([
      loadUpdaterConfig(repoPath),
      hasUpdateKey(repoPath),
      checkTauriCli().catch(() => 'not found'),
    ])
      .then(([c, k, v]) => {
        if (cancelled) return;
        setCfg(c);
        setHasKey(k);
        setCliVersion(v);
      })
      .catch((err) => !cancelled && notifyError('Updater load failed', err))
      .finally(() => !cancelled && setLoading(false));
    return () => {
      cancelled = true;
    };
  }, [repoPath]);

  function updateCfg<K extends keyof UpdaterConfig>(key: K, value: UpdaterConfig[K]) {
    setCfg((p) => (p ? { ...p, [key]: value } : p));
  }

  async function handleSave() {
    if (!cfg) return;
    setSaving(true);
    try {
      await saveUpdaterConfig(repoPath, cfg);
      notifySuccess('Updater config saved');
    } catch (err) {
      notifyError('Save failed', err);
    } finally {
      setSaving(false);
    }
  }

  async function handleGenerateKeys() {
    setBusy('genkey');
    try {
      const r = await generateUpdateKeys(repoPath);
      notifySuccess(r.message);
      const k = await hasUpdateKey(repoPath);
      setHasKey(k);
      const next = await loadUpdaterConfig(repoPath);
      setCfg(next);
    } catch (err) {
      notifyError('Key gen failed', err);
    } finally {
      setBusy(null);
    }
  }

  async function handleRotate() {
    if (
      !window.confirm('Rotate signing keys? Existing installs will need an OTA before next update.')
    )
      return;
    setBusy('rotate');
    try {
      const r = await rotateUpdateKeys(repoPath);
      notifySuccess(r.message);
      const next = await loadUpdaterConfig(repoPath);
      setCfg(next);
    } catch (err) {
      notifyError('Rotate failed', err);
    } finally {
      setBusy(null);
    }
  }

  async function handleDeleteKey() {
    if (!window.confirm('Delete private key from keychain? Cannot be undone.')) return;
    setBusy('delkey');
    try {
      await deleteUpdateKey(repoPath);
      setHasKey(false);
      notifySuccess('Private key deleted');
    } catch (err) {
      notifyError('Delete key failed', err);
    } finally {
      setBusy(null);
    }
  }

  async function handleImportKey() {
    if (!keyInput.trim()) return;
    setImportingKey(true);
    try {
      await importUpdatePrivateKey(repoPath, keyInput.trim());
      setHasKey(true);
      setKeyInput('');
      notifySuccess('Private key imported');
    } catch (err) {
      notifyError('Import key failed', err);
    } finally {
      setImportingKey(false);
    }
  }

  async function handlePreflight() {
    setBusy('preflight');
    try {
      const issues = await updaterPreflight(repoPath);
      setPreflightIssues(issues);
      if (issues.length === 0) notifySuccess('Preflight passed');
    } catch (err) {
      notifyError('Preflight failed', err);
    } finally {
      setBusy(null);
    }
  }

  async function handleGenerateLatest() {
    setBusy('latest');
    try {
      const r = await generateLatestJson(repoPath, publishVersion);
      notifySuccess(`latest.json written: ${r.path}`, r.valid ? 'valid ✓' : 'invalid');
    } catch (err) {
      notifyError('Generate latest.json failed', err);
    } finally {
      setBusy(null);
    }
  }

  async function handlePublish() {
    setBusy('publish');
    try {
      const r = await publishUpdate(repoPath, publishVersion, dryRun);
      if (r.success) notifySuccess(r.message, `${r.uploaded_files.length} files`);
      else notifyError(r.message);
    } catch (err) {
      notifyError('Publish failed', err);
    } finally {
      setBusy(null);
    }
  }

  return (
    <div className="feature-card">
      <div className="feature-card-header">
        <div className="feature-card-title">
          <ArrowUpCircle size={16} /> Auto-updater
        </div>
        <div className="feature-card-actions">
          <button
            className="btn btn-sm btn-ghost"
            onClick={handlePreflight}
            disabled={busy !== null}
          >
            {busy === 'preflight' ? 'Checking…' : 'Preflight'}
          </button>
          <button className="btn btn-sm btn-secondary" onClick={handleSave} disabled={saving}>
            {saving ? 'Saving…' : 'Save'}
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
            <div className="bw-summary" style={{ marginBottom: 0 }}>
              <span>
                tauri cli: <strong>{cliVersion}</strong>
              </span>
              <span style={{ color: hasKey ? 'var(--color-success)' : 'var(--color-text-muted)' }}>
                {hasKey ? 'private key: stored ✓' : 'no private key'}
              </span>
            </div>

            <label
              style={{
                display: 'flex',
                gap: 6,
                alignItems: 'center',
                fontSize: 'var(--font-size-xs)',
              }}
            >
              <input
                type="checkbox"
                checked={cfg.enabled}
                onChange={(e) => updateCfg('enabled', e.target.checked)}
              />
              Enabled
            </label>

            <div className="form-row">
              <label>Public key</label>
              <textarea
                className="input"
                rows={2}
                value={cfg.public_key ?? ''}
                onChange={(e) => updateCfg('public_key', e.target.value || undefined)}
                placeholder="Base64 public key — set automatically when generating keys"
              />
            </div>

            <div className="form-row">
              <label>Base URL (where latest.json lives)</label>
              <input
                className="input"
                value={cfg.base_url ?? ''}
                onChange={(e) => updateCfg('base_url', e.target.value || undefined)}
                placeholder="https://updates.example.com"
              />
            </div>

            <div className="form-row">
              <label>Publish target</label>
              <select
                className="input"
                value={cfg.publish_target ?? 'github_release'}
                onChange={(e) => updateCfg('publish_target', e.target.value as UpdatePublishTarget)}
              >
                {TARGETS.map((t) => (
                  <option key={t} value={t}>
                    {t}
                  </option>
                ))}
              </select>
            </div>

            {cfg.publish_target === 'github_release' && (
              <div className="form-row">
                <label>GitHub repo (owner/name)</label>
                <input
                  className="input"
                  value={cfg.github_repo ?? ''}
                  onChange={(e) => updateCfg('github_repo', e.target.value || undefined)}
                />
              </div>
            )}
            {cfg.publish_target === 's3' && (
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 8 }}>
                <div className="form-row">
                  <label>S3 bucket</label>
                  <input
                    className="input"
                    value={cfg.s3_bucket ?? ''}
                    onChange={(e) => updateCfg('s3_bucket', e.target.value || undefined)}
                  />
                </div>
                <div className="form-row">
                  <label>Region</label>
                  <input
                    className="input"
                    value={cfg.s3_region ?? ''}
                    onChange={(e) => updateCfg('s3_region', e.target.value || undefined)}
                  />
                </div>
                <div className="form-row">
                  <label>Endpoint (custom S3)</label>
                  <input
                    className="input"
                    value={cfg.s3_endpoint ?? ''}
                    onChange={(e) => updateCfg('s3_endpoint', e.target.value || undefined)}
                  />
                </div>
              </div>
            )}
            {cfg.publish_target === 'scp' && (
              <div className="form-row">
                <label>SCP destination</label>
                <input
                  className="input"
                  value={cfg.scp_dest ?? ''}
                  onChange={(e) => updateCfg('scp_dest', e.target.value || undefined)}
                  placeholder="user@host:/path"
                />
              </div>
            )}
            {cfg.publish_target === 'local' && (
              <div className="form-row">
                <label>Local directory</label>
                <input
                  className="input"
                  value={cfg.local_dir ?? ''}
                  onChange={(e) => updateCfg('local_dir', e.target.value || undefined)}
                />
              </div>
            )}

            {/* Keys */}
            <div
              style={{ marginTop: 12, paddingTop: 12, borderTop: '1px solid var(--color-border)' }}
            >
              <strong
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 6,
                  fontSize: 'var(--font-size-sm)',
                }}
              >
                <KeyRound size={14} /> Keys
              </strong>
              <div style={{ display: 'flex', gap: 8, marginTop: 8 }}>
                <button
                  className="btn btn-sm btn-secondary"
                  onClick={handleGenerateKeys}
                  disabled={busy !== null}
                >
                  {hasKey ? 'Regenerate' : 'Generate'}
                </button>
                <button
                  className="btn btn-sm btn-secondary"
                  onClick={handleRotate}
                  disabled={busy !== null || !hasKey}
                >
                  Rotate
                </button>
                <button
                  className="btn btn-sm btn-danger-outline"
                  onClick={handleDeleteKey}
                  disabled={busy !== null || !hasKey}
                >
                  Delete
                </button>
                {cfg.public_key && (
                  <button
                    className="btn btn-sm btn-ghost"
                    onClick={() => {
                      navigator.clipboard.writeText(cfg.public_key!);
                      notifySuccess('Public key copied');
                    }}
                  >
                    <Copy size={12} /> Copy pub
                  </button>
                )}
              </div>
              <div style={{ display: 'flex', gap: 6, marginTop: 8 }}>
                <input
                  className="input"
                  style={{ flex: 1 }}
                  placeholder="Paste existing private key to import…"
                  value={keyInput}
                  onChange={(e) => setKeyInput(e.target.value)}
                />
                <button
                  className="btn btn-sm btn-ghost"
                  onClick={handleImportKey}
                  disabled={importingKey || !keyInput.trim()}
                >
                  {importingKey ? 'Importing…' : 'Import'}
                </button>
              </div>
            </div>

            {/* Publish */}
            <div
              style={{ marginTop: 12, paddingTop: 12, borderTop: '1px solid var(--color-border)' }}
            >
              <strong
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 6,
                  fontSize: 'var(--font-size-sm)',
                }}
              >
                <Send size={14} /> Publish
              </strong>
              <div style={{ display: 'flex', gap: 8, alignItems: 'flex-end', marginTop: 8 }}>
                <div className="form-row" style={{ flex: 1, marginBottom: 0 }}>
                  <label>Version</label>
                  <input
                    className="input"
                    value={publishVersion}
                    onChange={(e) => setPublishVersion(e.target.value)}
                  />
                </div>
                <label
                  style={{
                    display: 'flex',
                    gap: 4,
                    alignItems: 'center',
                    fontSize: 'var(--font-size-xs)',
                  }}
                >
                  <input
                    type="checkbox"
                    checked={dryRun}
                    onChange={(e) => setDryRun(e.target.checked)}
                  />
                  Dry run
                </label>
                <button
                  className="btn btn-sm btn-secondary"
                  onClick={handleGenerateLatest}
                  disabled={busy !== null}
                >
                  latest.json
                </button>
                <button
                  className="btn btn-sm btn-primary"
                  onClick={handlePublish}
                  disabled={busy !== null}
                >
                  {busy === 'publish' ? 'Publishing…' : dryRun ? 'Dry run' : 'Publish'}
                </button>
              </div>
            </div>

            {preflightIssues !== null && (
              <div
                style={{
                  marginTop: 12,
                  fontSize: 'var(--font-size-xs)',
                }}
              >
                {preflightIssues.length === 0 ? (
                  <div
                    style={{
                      color: 'var(--color-success)',
                      display: 'flex',
                      gap: 4,
                      alignItems: 'center',
                    }}
                  >
                    <ShieldCheck size={12} /> Preflight passed.
                  </div>
                ) : (
                  <div>
                    <strong
                      style={{
                        display: 'flex',
                        gap: 4,
                        alignItems: 'center',
                        color: 'var(--color-failed)',
                      }}
                    >
                      <AlertTriangle size={12} /> {preflightIssues.length} issues
                    </strong>
                    <ul style={{ paddingLeft: 16 }}>
                      {preflightIssues.map((s, i) => (
                        <li key={i}>{s}</li>
                      ))}
                    </ul>
                  </div>
                )}
              </div>
            )}
          </>
        ) : null}
      </div>
    </div>
  );
}

export default UpdaterCard;
