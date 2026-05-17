import { useEffect, useState } from 'react';
import { Package, Loader2, FolderOpen, FileSignature } from 'lucide-react';
import {
  checkSigningTools,
  collectArtifacts,
  listArtifactManifests,
  loadArtifactConfig,
  loadSigningConfig,
  saveArtifactConfig,
  saveSigningConfig,
  signArtifact,
} from '../services/api';
import { notifyError, notifySuccess } from '../services/notify';
import { openPath } from '../services/openExternal';
import type { ArtifactConfig, ArtifactManifest, SigningConfig } from '../types';

interface Props {
  repoPath: string;
}

function ArtifactsCard({ repoPath }: Props) {
  const [cfg, setCfg] = useState<ArtifactConfig | null>(null);
  const [signing, setSigning] = useState<SigningConfig | null>(null);
  const [manifests, setManifests] = useState<ArtifactManifest[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [collecting, setCollecting] = useState(false);
  const [tools, setTools] = useState<string[]>([]);
  const [signingArtifact, setSigningArtifact] = useState<string | null>(null);
  const [projectName, setProjectName] = useState('app');
  const [version, setVersion] = useState('0.0.0');

  useEffect(() => {
    let cancelled = false;
    Promise.all([
      loadArtifactConfig(repoPath),
      loadSigningConfig(repoPath),
      listArtifactManifests(repoPath).catch(() => []),
      checkSigningTools().catch(() => []),
    ])
      .then(([c, s, m, t]) => {
        if (cancelled) return;
        setCfg(c);
        setSigning(s);
        setManifests(m);
        setTools(t);
      })
      .catch((err) => !cancelled && notifyError('Failed to load artifact config', err))
      .finally(() => !cancelled && setLoading(false));
    return () => {
      cancelled = true;
    };
  }, [repoPath]);

  async function handleSaveConfig() {
    if (!cfg) return;
    setSaving(true);
    try {
      await saveArtifactConfig(repoPath, cfg);
      notifySuccess('Artifacts config saved');
    } catch (err) {
      notifyError('Save failed', err);
    } finally {
      setSaving(false);
    }
  }

  async function handleSaveSigning() {
    if (!signing) return;
    try {
      await saveSigningConfig(repoPath, signing);
      notifySuccess('Signing config saved');
    } catch (err) {
      notifyError('Save failed', err);
    }
  }

  async function handleCollect() {
    if (!cfg) return;
    setCollecting(true);
    try {
      const m = await collectArtifacts(repoPath, projectName, version);
      notifySuccess('Artifacts collected', `${m.artifacts.length} files`);
      const updated = await listArtifactManifests(repoPath).catch(() => []);
      setManifests(updated);
    } catch (err) {
      notifyError('Collection failed', err);
    } finally {
      setCollecting(false);
    }
  }

  async function handleSign(path: string) {
    setSigningArtifact(path);
    try {
      const r = await signArtifact(repoPath, path);
      if (r.success) notifySuccess('Signed', r.message);
      else notifyError(r.message);
    } catch (err) {
      notifyError('Sign failed', err);
    } finally {
      setSigningArtifact(null);
    }
  }

  function updateCfg<K extends keyof ArtifactConfig>(key: K, value: ArtifactConfig[K]) {
    setCfg((prev) => (prev ? { ...prev, [key]: value } : prev));
  }

  function updateSigning<K extends keyof SigningConfig>(key: K, value: SigningConfig[K]) {
    setSigning((prev) => (prev ? { ...prev, [key]: value } : prev));
  }

  return (
    <div className="feature-card">
      <div className="feature-card-header">
        <div className="feature-card-title">
          <Package size={16} /> Artifacts &amp; Signing
        </div>
        <div className="feature-card-actions">
          <button className="btn btn-sm btn-secondary" onClick={handleCollect} disabled={collecting || !cfg}>
            {collecting ? 'Collecting…' : 'Collect'}
          </button>
        </div>
      </div>
      <div className="feature-card-body">
        {loading ? (
          <div className="feature-card-empty">
            <Loader2 size={14} className="spin" /> Loading…
          </div>
        ) : (
          <>
            {cfg && (
              <>
                <div className="form-row">
                  <label>Output directory</label>
                  <input
                    className="input"
                    value={cfg.output_dir}
                    onChange={(e) => updateCfg('output_dir', e.target.value)}
                  />
                </div>
                <div style={{ display: 'flex', gap: 12 }}>
                  <div className="form-row" style={{ flex: 1 }}>
                    <label>Retention count</label>
                    <input
                      className="input"
                      type="number"
                      value={cfg.retention_count}
                      onChange={(e) => updateCfg('retention_count', parseInt(e.target.value, 10) || 0)}
                    />
                  </div>
                  <div className="form-row" style={{ flex: 1 }}>
                    <label>Upload to (optional)</label>
                    <input
                      className="input"
                      value={cfg.upload_to ?? ''}
                      onChange={(e) => updateCfg('upload_to', e.target.value || undefined)}
                      placeholder="s3://bucket/prefix"
                    />
                  </div>
                </div>
                <div className="form-row">
                  <label>Glob patterns (one per line)</label>
                  <textarea
                    className="input"
                    rows={3}
                    value={cfg.patterns.join('\n')}
                    onChange={(e) =>
                      updateCfg(
                        'patterns',
                        e.target.value.split('\n').map((s) => s.trim()).filter(Boolean)
                      )
                    }
                    placeholder="target/release/*.dmg"
                  />
                </div>
                <div style={{ display: 'flex', gap: 8 }}>
                  <div className="form-row" style={{ flex: 1 }}>
                    <label>Project name (for collect)</label>
                    <input
                      className="input"
                      value={projectName}
                      onChange={(e) => setProjectName(e.target.value)}
                    />
                  </div>
                  <div className="form-row" style={{ flex: 1 }}>
                    <label>Version</label>
                    <input
                      className="input"
                      value={version}
                      onChange={(e) => setVersion(e.target.value)}
                    />
                  </div>
                </div>
                <div>
                  <button className="btn btn-sm btn-secondary" onClick={handleSaveConfig} disabled={saving}>
                    {saving ? 'Saving…' : 'Save config'}
                  </button>
                </div>
              </>
            )}

            {/* Signing sub-section */}
            {signing && (
              <div style={{ marginTop: 12, paddingTop: 12, borderTop: '1px solid var(--color-border)' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 8 }}>
                  <FileSignature size={14} />
                  <strong>Signing</strong>
                  <span className="text-muted" style={{ fontSize: 'var(--font-size-2xs)', marginLeft: 'auto' }}>
                    tools: {tools.length ? tools.join(', ') : 'none detected'}
                  </span>
                </div>
                <label
                  style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 'var(--font-size-xs)' }}
                >
                  <input
                    type="checkbox"
                    checked={signing.enabled}
                    onChange={(e) => updateSigning('enabled', e.target.checked)}
                  />
                  Enabled
                </label>
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8, marginTop: 8 }}>
                  <div className="form-row">
                    <label>macOS identity</label>
                    <input
                      className="input"
                      value={signing.macos_identity ?? ''}
                      onChange={(e) => updateSigning('macos_identity', e.target.value || undefined)}
                    />
                  </div>
                  <div className="form-row">
                    <label>macOS team ID</label>
                    <input
                      className="input"
                      value={signing.macos_team_id ?? ''}
                      onChange={(e) => updateSigning('macos_team_id', e.target.value || undefined)}
                    />
                  </div>
                  <div className="form-row">
                    <label>Windows cert path</label>
                    <input
                      className="input"
                      value={signing.windows_cert_path ?? ''}
                      onChange={(e) =>
                        updateSigning('windows_cert_path', e.target.value || undefined)
                      }
                    />
                  </div>
                  <div className="form-row">
                    <label>Linux GPG key</label>
                    <input
                      className="input"
                      value={signing.linux_gpg_key ?? ''}
                      onChange={(e) => updateSigning('linux_gpg_key', e.target.value || undefined)}
                    />
                  </div>
                </div>
                <button className="btn btn-sm btn-secondary" onClick={handleSaveSigning}>
                  Save signing
                </button>
              </div>
            )}

            {/* Manifests */}
            <div style={{ marginTop: 12 }}>
              <strong style={{ fontSize: 'var(--font-size-sm)' }}>Manifests</strong>
              {manifests.length === 0 ? (
                <p className="text-muted" style={{ fontSize: 'var(--font-size-xs)' }}>
                  None yet — click Collect to gather configured patterns.
                </p>
              ) : (
                <table className="kv-table">
                  <thead>
                    <tr>
                      <th>Project</th>
                      <th>Version</th>
                      <th>Files</th>
                      <th>Created</th>
                      <th></th>
                    </tr>
                  </thead>
                  <tbody>
                    {manifests.map((m) => (
                      <tr key={`${m.project}-${m.version}-${m.created_at}`}>
                        <td>{m.project}</td>
                        <td>
                          <code>{m.version}</code>
                        </td>
                        <td>{m.artifacts.length}</td>
                        <td>{new Date(m.created_at).toLocaleString()}</td>
                        <td>
                          {m.artifacts[0]?.path && (
                            <button
                              className="btn btn-xs btn-ghost"
                              onClick={() => openPath(m.artifacts[0].path.replace(/\/[^/]*$/, ''))}
                              title="Reveal output dir"
                            >
                              <FolderOpen size={12} />
                            </button>
                          )}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>

            {/* Sign first manifest's artifacts (breadth: keep simple) */}
            {manifests[0]?.artifacts.length ? (
              <div style={{ marginTop: 8 }}>
                <strong style={{ fontSize: 'var(--font-size-sm)' }}>Sign artifacts (latest)</strong>
                <table className="kv-table">
                  <tbody>
                    {manifests[0].artifacts.map((a) => (
                      <tr key={a.path}>
                        <td>
                          <code>{a.file_name}</code>
                        </td>
                        <td style={{ textAlign: 'right' }}>
                          <button
                            className="btn btn-xs btn-ghost"
                            disabled={signingArtifact === a.path}
                            onClick={() => handleSign(a.path)}
                          >
                            <FileSignature size={12} />{' '}
                            {signingArtifact === a.path ? 'Signing…' : 'Sign'}
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            ) : null}
          </>
        )}
      </div>
    </div>
  );
}

export default ArtifactsCard;
