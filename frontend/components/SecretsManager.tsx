import { useState, useEffect } from 'react';
import { Shield, Plus, Trash2, Check, AlertTriangle, X } from 'lucide-react';
import { saveSecretsConfig, checkSecretsStatus, setSecret, deleteSecret } from '../services/api';
import type { SecretsConfig, SecretRef, SecretStatus, Environment } from '../types';

interface Props {
  repoPath: string;
  config: SecretsConfig;
  environments: Environment[];
  onSaved: () => void;
}

function SecretsManager({ repoPath, config, environments, onSaved }: Props) {
  const [secrets, setSecrets] = useState<SecretRef[]>(config.secrets);
  const [statuses, setStatuses] = useState<Record<string, SecretStatus[]>>({});
  const [adding, setAdding] = useState(false);
  const [draftName, setDraftName] = useState('');
  const [draftEnvs, setDraftEnvs] = useState<string[]>([]);
  const [settingValue, setSettingValue] = useState<{
    name: string;
    env: string;
  } | null>(null);
  const [secretValue, setSecretValue] = useState('');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    loadStatuses();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [environments.length]);

  async function loadStatuses() {
    const result: Record<string, SecretStatus[]> = {};
    for (const env of environments) {
      try {
        result[env.name] = await checkSecretsStatus(repoPath, env.name);
      } catch {
        // Ignore errors loading status
      }
    }
    setStatuses(result);
  }

  async function handleSaveConfig() {
    try {
      setSaving(true);
      setError(null);
      await saveSecretsConfig(repoPath, { secrets });
      onSaved();
      await loadStatuses();
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  }

  function handleAdd() {
    if (!draftName.trim()) return;
    if (secrets.some((s) => s.name === draftName.trim())) {
      setError(`Secret "${draftName}" already exists`);
      return;
    }
    setSecrets([...secrets, { name: draftName.trim(), environments: draftEnvs }]);
    setDraftName('');
    setDraftEnvs([]);
    setAdding(false);
  }

  function handleRemoveSecret(name: string) {
    setSecrets(secrets.filter((s) => s.name !== name));
  }

  async function handleSetValue() {
    if (!settingValue || !secretValue) return;
    try {
      setError(null);
      await setSecret(repoPath, settingValue.env, settingValue.name, secretValue);
      setSettingValue(null);
      setSecretValue('');
      await loadStatuses();
    } catch (err) {
      setError(String(err));
    }
  }

  async function handleDeleteSecret(name: string, envName: string) {
    try {
      setError(null);
      await deleteSecret(repoPath, envName, name);
      await loadStatuses();
    } catch (err) {
      setError(String(err));
    }
  }

  function getStatus(secretName: string, envName: string): boolean | null {
    const envStatuses = statuses[envName];
    if (!envStatuses) return null;
    const s = envStatuses.find((st) => st.name === secretName);
    return s?.is_set ?? null;
  }

  const hasChanges = JSON.stringify(config.secrets) !== JSON.stringify(secrets);

  return (
    <div className="secrets-manager">
      <div className="section-header-row">
        <h3 className="section-title">
          <Shield size={16} /> Secrets
        </h3>
        <div className="section-header-actions">
          {!adding && (
            <button className="btn btn-secondary btn-sm" onClick={() => setAdding(true)}>
              <Plus size={14} /> Add
            </button>
          )}
          {hasChanges && (
            <button className="btn btn-primary btn-sm" onClick={handleSaveConfig} disabled={saving}>
              {saving ? 'Saving...' : 'Save'}
            </button>
          )}
        </div>
      </div>

      {error && <div className="alert alert-error">{error}</div>}

      {/* Add new secret ref */}
      {adding && (
        <div className="secret-add-form">
          <div className="env-card-row">
            <input
              className="input input-sm"
              placeholder="SECRET_NAME"
              value={draftName}
              onChange={(e) => setDraftName(e.target.value.toUpperCase())}
            />
            <select
              className="input input-sm"
              multiple
              value={draftEnvs}
              onChange={(e) => setDraftEnvs(Array.from(e.target.selectedOptions, (o) => o.value))}
            >
              {environments.map((env) => (
                <option key={env.name} value={env.name}>
                  {env.name}
                </option>
              ))}
            </select>
            <button className="btn btn-primary btn-sm" onClick={handleAdd}>
              Add
            </button>
            <button
              className="btn btn-icon btn-sm"
              onClick={() => {
                setAdding(false);
                setDraftName('');
                setDraftEnvs([]);
              }}
            >
              <X size={14} />
            </button>
          </div>
          <p className="text-muted text-xs">
            Leave environments empty to apply to all. Ctrl/Cmd-click to select multiple.
          </p>
        </div>
      )}

      {/* Set value overlay */}
      {settingValue && (
        <div className="secret-set-value">
          <p>
            Set <strong>{settingValue.name}</strong> for <strong>{settingValue.env}</strong>:
          </p>
          <div className="env-card-row">
            <input
              className="input input-sm"
              type="password"
              placeholder="Secret value"
              value={secretValue}
              onChange={(e) => setSecretValue(e.target.value)}
              autoFocus
            />
            <button className="btn btn-primary btn-sm" onClick={handleSetValue}>
              Save to Keychain
            </button>
            <button
              className="btn btn-icon btn-sm"
              onClick={() => {
                setSettingValue(null);
                setSecretValue('');
              }}
            >
              <X size={14} />
            </button>
          </div>
        </div>
      )}

      {/* Secret list */}
      {secrets.length === 0 && !adding ? (
        <p className="text-muted">
          No secrets defined. Secrets are stored in your OS keychain, never in config files.
        </p>
      ) : (
        <div className="secret-list">
          {secrets.map((secret) => (
            <div key={secret.name} className="secret-card">
              <div className="secret-card-header">
                <code className="secret-name">{secret.name}</code>
                {secret.environments.length > 0 ? (
                  <span className="text-muted text-xs">{secret.environments.join(', ')}</span>
                ) : (
                  <span className="text-muted text-xs">all environments</span>
                )}
                <button
                  className="btn btn-icon btn-sm btn-danger-icon"
                  title="Remove secret"
                  onClick={() => handleRemoveSecret(secret.name)}
                >
                  <Trash2 size={12} />
                </button>
              </div>
              <div className="secret-env-grid">
                {environments
                  .filter(
                    (env) =>
                      secret.environments.length === 0 || secret.environments.includes(env.name)
                  )
                  .map((env) => {
                    const isSet = getStatus(secret.name, env.name);
                    return (
                      <div key={env.name} className="secret-env-item">
                        <span className="secret-env-name">{env.name}</span>
                        {isSet === true ? (
                          <span className="secret-status secret-set">
                            <Check size={12} /> Set
                          </span>
                        ) : (
                          <span className="secret-status secret-missing">
                            <AlertTriangle size={12} /> Not set
                          </span>
                        )}
                        <button
                          className="btn btn-icon btn-sm"
                          title="Set value"
                          onClick={() => setSettingValue({ name: secret.name, env: env.name })}
                        >
                          <Shield size={12} />
                        </button>
                        {isSet && (
                          <button
                            className="btn btn-icon btn-sm btn-danger-icon"
                            title="Delete from keychain"
                            onClick={() => handleDeleteSecret(secret.name, env.name)}
                          >
                            <Trash2 size={12} />
                          </button>
                        )}
                      </div>
                    );
                  })}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export default SecretsManager;
