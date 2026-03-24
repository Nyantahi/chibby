import { useEffect, useState } from 'react';
import { Key, Bell, Archive, Info, Eye, EyeOff, Check, Bot } from 'lucide-react';
import {
  loadAppSettings,
  saveAppSettings,
  setAppApiKey,
  deleteAppApiKey,
  hasAppApiKey,
  getAppDataDir,
  getAppVersion,
  getAgentStatus,
  rebuildAgent,
} from '../services/api';
import type { AppSettings, AgentSystemStatus } from '../types';

const API_PROVIDERS = [
  { id: 'openai', label: 'OpenAI' },
  { id: 'anthropic', label: 'Anthropic' },
];

function Settings() {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [successMsg, setSuccessMsg] = useState<string | null>(null);

  // API key state
  const [keyStatus, setKeyStatus] = useState<Record<string, boolean>>({});
  const [keyInputs, setKeyInputs] = useState<Record<string, string>>({});
  const [keyVisible, setKeyVisible] = useState<Record<string, boolean>>({});
  const [keySaving, setKeySaving] = useState<Record<string, boolean>>({});

  // Agent status
  const [agentStatus, setAgentStatus] = useState<AgentSystemStatus | null>(null);

  // About section
  const [appVersion, setAppVersion] = useState('');
  const [dataDir, setDataDir] = useState('');

  useEffect(() => {
    loadAll();
  }, []);

  async function loadAll() {
    try {
      setLoading(true);
      const [s, version, dir] = await Promise.all([
        loadAppSettings(),
        getAppVersion(),
        getAppDataDir(),
      ]);
      setSettings(s);
      setAppVersion(version);
      setDataDir(dir);

      // Check which API keys are set
      const statuses: Record<string, boolean> = {};
      for (const p of API_PROVIDERS) {
        statuses[p.id] = await hasAppApiKey(p.id);
      }
      setKeyStatus(statuses);

      // Load agent status
      const aStatus = await getAgentStatus();
      setAgentStatus(aStatus);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  async function handleSaveSettings() {
    if (!settings) return;
    try {
      setSaving(true);
      setError(null);
      await saveAppSettings(settings);
      setSuccessMsg('Settings saved');
      setTimeout(() => setSuccessMsg(null), 2000);
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  }

  async function handleSetKey(provider: string) {
    const value = keyInputs[provider];
    if (!value?.trim()) return;
    try {
      setKeySaving({ ...keySaving, [provider]: true });
      await setAppApiKey(provider, value.trim());
      setKeyStatus({ ...keyStatus, [provider]: true });
      setKeyInputs({ ...keyInputs, [provider]: '' });
      setKeyVisible({ ...keyVisible, [provider]: false });
      // Rebuild agent with new key
      const newStatus = await rebuildAgent();
      setAgentStatus(newStatus);
    } catch (err) {
      setError(String(err));
    } finally {
      setKeySaving({ ...keySaving, [provider]: false });
    }
  }

  async function handleDeleteKey(provider: string) {
    try {
      setKeySaving({ ...keySaving, [provider]: true });
      await deleteAppApiKey(provider);
      setKeyStatus({ ...keyStatus, [provider]: false });
      // Rebuild agent without removed key
      const newStatus = await rebuildAgent();
      setAgentStatus(newStatus);
    } catch (err) {
      setError(String(err));
    } finally {
      setKeySaving({ ...keySaving, [provider]: false });
    }
  }

  function updateSetting<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
    if (!settings) return;
    setSettings({ ...settings, [key]: value });
  }

  if (loading || !settings) {
    return (
      <div className="page">
        <div className="loading">Loading settings...</div>
      </div>
    );
  }

  return (
    <div className="page">
      <header className="page-header">
        <h2 className="page-title">Settings</h2>
      </header>

      {error && <div className="alert alert-error">{error}</div>}
      {successMsg && <div className="alert alert-success">{successMsg}</div>}

      {/* AI Agent Keys & CI/CD Agent — hidden until AI integration is built */}
      {false && (
        <>
        <section className="settings-section">
          <h3 className="settings-section-title">
            <Key size={16} /> AI Agent Keys
          </h3>
          <p className="settings-section-desc">
            API keys for AI-assisted features. Keys are stored securely in your OS keychain.
          </p>

          {API_PROVIDERS.map(({ id, label }) => (
            <div key={id} className="settings-key-row">
              <div className="settings-key-header">
                <span className="settings-key-label">{label}</span>
                {keyStatus[id] && (
                  <span className="settings-key-status">
                    <Check size={14} /> Configured
                  </span>
                )}
              </div>

              {keyStatus[id] ? (
                <button
                  className="btn btn-danger btn-sm"
                  onClick={() => handleDeleteKey(id)}
                  disabled={keySaving[id]}
                >
                  {keySaving[id] ? 'Removing...' : 'Remove Key'}
                </button>
              ) : (
                <div className="settings-key-input-row">
                  <div className="settings-key-input-wrap">
                    <input
                      type={keyVisible[id] ? 'text' : 'password'}
                      className="input"
                      placeholder={`Enter ${label} API key`}
                      value={keyInputs[id] || ''}
                      onChange={(e) => setKeyInputs({ ...keyInputs, [id]: e.target.value })}
                    />
                    <button
                      className="btn btn-icon"
                      onClick={() => setKeyVisible({ ...keyVisible, [id]: !keyVisible[id] })}
                      title={keyVisible[id] ? 'Hide' : 'Show'}
                    >
                      {keyVisible[id] ? <EyeOff size={14} /> : <Eye size={14} />}
                    </button>
                  </div>
                  <button
                    className="btn btn-primary btn-sm"
                    onClick={() => handleSetKey(id)}
                    disabled={!keyInputs[id]?.trim() || keySaving[id]}
                  >
                    {keySaving[id] ? 'Saving...' : 'Set Key'}
                  </button>
                </div>
              )}
            </div>
          ))}
        </section>

        {agentStatus && (() => {
          const status = agentStatus!;
          return (
          <section className="settings-section">
            <h3 className="settings-section-title">
              <Bot size={16} /> CI/CD Agent
            </h3>
            <p className="settings-section-desc">
              AI-powered analysis, pipeline generation, and failure recovery.
            </p>
            <div className="settings-info-row">
              <span className="settings-info-label">Status</span>
              <span className={`settings-info-value ${status.available ? 'text-green-400' : 'text-yellow-400'}`}>
                {status.available ? 'Active' : 'Not configured'}
              </span>
            </div>
            {status.error && (
              <p className="text-sm text-yellow-400 mt-1">{status.error}</p>
            )}
          </section>
          );
        })()}
        </>
      )}

      {/* Default Notifications */}
      <section className="settings-section">
        <h3 className="settings-section-title">
          <Bell size={16} /> Default Notifications
        </h3>
        <p className="settings-section-desc">Default notification preferences for new projects.</p>

        <label className="settings-toggle">
          <input
            type="checkbox"
            checked={settings.default_notify_on_success}
            onChange={(e) => updateSetting('default_notify_on_success', e.target.checked)}
          />
          <span>Notify on successful runs</span>
        </label>

        <label className="settings-toggle">
          <input
            type="checkbox"
            checked={settings.default_notify_on_failure}
            onChange={(e) => updateSetting('default_notify_on_failure', e.target.checked)}
          />
          <span>Notify on failed runs</span>
        </label>
      </section>

      {/* Default Retention */}
      <section className="settings-section">
        <h3 className="settings-section-title">
          <Archive size={16} /> Default Retention
        </h3>
        <p className="settings-section-desc">Default retention limits for new projects.</p>

        <div className="form-group">
          <label className="form-label">Artifact retention (versions to keep)</label>
          <input
            type="number"
            className="input input-sm"
            min={1}
            max={100}
            value={settings.default_artifact_retention}
            onChange={(e) =>
              updateSetting('default_artifact_retention', parseInt(e.target.value) || 5)
            }
          />
        </div>

        <div className="form-group">
          <label className="form-label">Run history retention (runs to keep)</label>
          <input
            type="number"
            className="input input-sm"
            min={1}
            max={1000}
            value={settings.default_run_retention}
            onChange={(e) => updateSetting('default_run_retention', parseInt(e.target.value) || 50)}
          />
        </div>
      </section>

      {/* Save button */}
      <div className="settings-actions">
        <button className="btn btn-primary" onClick={handleSaveSettings} disabled={saving}>
          {saving ? 'Saving...' : 'Save Settings'}
        </button>
      </div>

      {/* About */}
      <section className="settings-section settings-about">
        <h3 className="settings-section-title">
          <Info size={16} /> About
        </h3>
        <div className="settings-info-row">
          <span className="settings-info-label">Version</span>
          <span className="settings-info-value">{appVersion}</span>
        </div>
        <div className="settings-info-row">
          <span className="settings-info-label">Data directory</span>
          <span className="settings-info-value mono">{dataDir}</span>
        </div>
      </section>
    </div>
  );
}

export default Settings;
