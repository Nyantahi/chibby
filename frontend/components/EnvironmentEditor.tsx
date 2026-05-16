import { useState } from 'react';
import { Server, Plus, Trash2, Wifi, X } from 'lucide-react';
import { saveEnvironments, testSshConnection } from '../services/api';
import type { EnvironmentsConfig, Environment } from '../types';

/** Validate that a string is a legal shell environment variable name. */
function isValidEnvVarName(name: string): boolean {
  return /^[A-Za-z_][A-Za-z0-9_]*$/.test(name);
}

interface Props {
  repoPath: string;
  config: EnvironmentsConfig;
  onSaved: () => void;
}

function EnvironmentEditor({ repoPath, config, onSaved }: Props) {
  const [envs, setEnvs] = useState<Environment[]>(config.environments);
  const [editing, setEditing] = useState<number | null>(null);
  const [adding, setAdding] = useState(false);
  const [draft, setDraft] = useState<Environment>(emptyEnv());
  const [sshStatus, setSshStatus] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function emptyEnv(): Environment {
    return { name: '', ssh_host: undefined, ssh_port: undefined, variables: {} };
  }

  async function handleSave() {
    // Validate all variable names before saving
    for (const env of envs) {
      for (const key of Object.keys(env.variables)) {
        if (!isValidEnvVarName(key)) {
          setError(
            `Invalid variable name "${key}" in environment "${env.name}". ` +
              'Names must start with a letter or underscore, followed by letters, digits, or underscores.'
          );
          return;
        }
      }
    }
    try {
      setSaving(true);
      setError(null);
      await saveEnvironments(repoPath, { environments: envs });
      onSaved();
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  }

  function handleAdd() {
    if (!draft.name.trim()) return;
    setEnvs([...envs, { ...draft }]);
    setDraft(emptyEnv());
    setAdding(false);
  }

  function handleRemove(idx: number) {
    setEnvs(envs.filter((_, i) => i !== idx));
  }

  function handleUpdate(idx: number, updates: Partial<Environment>) {
    setEnvs(envs.map((e, i) => (i === idx ? { ...e, ...updates } : e)));
  }

  function handleVarChange(envIdx: number, key: string, value: string) {
    const env = envs[envIdx];
    const vars = { ...env.variables, [key]: value };
    handleUpdate(envIdx, { variables: vars });
  }

  function handleVarRemove(envIdx: number, key: string) {
    const env = envs[envIdx];
    const vars = { ...env.variables };
    delete vars[key];
    handleUpdate(envIdx, { variables: vars });
  }

  function handleVarAdd(envIdx: number) {
    const env = envs[envIdx];
    const newKey = `VAR_${Object.keys(env.variables).length + 1}`;
    handleUpdate(envIdx, { variables: { ...env.variables, [newKey]: '' } });
  }

  async function handleTestSsh(env: Environment) {
    if (!env.ssh_host) return;
    setSshStatus((prev) => ({ ...prev, [env.name]: 'testing...' }));
    try {
      await testSshConnection(env.ssh_host, env.ssh_port ?? undefined);
      setSshStatus((prev) => ({ ...prev, [env.name]: 'connected' }));
    } catch (err) {
      setSshStatus((prev) => ({ ...prev, [env.name]: `failed: ${err}` }));
    }
  }

  const hasChanges = JSON.stringify(config.environments) !== JSON.stringify(envs);

  return (
    <div className="env-editor">
      <div className="section-header-row">
        <h3 className="section-title">
          <Server size={16} /> Environments
        </h3>
        <div className="section-header-actions">
          {!adding && (
            <button className="btn btn-secondary btn-sm" onClick={() => setAdding(true)}>
              <Plus size={14} /> Add
            </button>
          )}
          {hasChanges && (
            <button className="btn btn-primary btn-sm" onClick={handleSave} disabled={saving}>
              {saving ? 'Saving...' : 'Save'}
            </button>
          )}
        </div>
      </div>

      {error && <div className="alert alert-error">{error}</div>}

      {/* Add new environment */}
      {adding && (
        <div className="env-card env-card-adding">
          <div className="env-card-row">
            <input
              className="input input-sm"
              placeholder="Environment name (e.g. production)"
              value={draft.name}
              onChange={(e) => setDraft({ ...draft, name: e.target.value })}
            />
            <input
              className="input input-sm"
              placeholder="SSH host (user@host)"
              value={draft.ssh_host ?? ''}
              onChange={(e) => setDraft({ ...draft, ssh_host: e.target.value || undefined })}
            />
            <input
              className="input input-sm input-narrow"
              placeholder="Port"
              type="number"
              value={draft.ssh_port ?? ''}
              onChange={(e) =>
                setDraft({
                  ...draft,
                  ssh_port: e.target.value ? Number(e.target.value) : undefined,
                })
              }
            />
            <button className="btn btn-primary btn-sm" onClick={handleAdd}>
              Add
            </button>
            <button
              className="btn btn-icon btn-sm"
              onClick={() => {
                setAdding(false);
                setDraft(emptyEnv());
              }}
            >
              <X size={14} />
            </button>
          </div>
        </div>
      )}

      {/* Environment list */}
      {envs.length === 0 && !adding ? (
        <p className="text-muted">
          No environments defined. Add one to enable SSH deployments and per-environment variables.
        </p>
      ) : (
        <div className="env-list">
          {envs.map((env, idx) => (
            <div key={idx} className="env-card">
              <div className="env-card-header">
                <strong className="env-name">{env.name}</strong>
                {env.ssh_host && (
                  <code className="env-host">
                    {env.ssh_host}
                    {env.ssh_port ? `:${env.ssh_port}` : ''}
                  </code>
                )}
                <div className="env-card-actions">
                  {env.ssh_host && (
                    <button
                      className="btn btn-icon btn-sm"
                      title="Test SSH connection"
                      onClick={() => handleTestSsh(env)}
                    >
                      <Wifi size={14} />
                    </button>
                  )}
                  <button
                    className="btn btn-icon btn-sm"
                    title={editing === idx ? 'Close' : 'Edit'}
                    onClick={() => setEditing(editing === idx ? null : idx)}
                  >
                    {editing === idx ? <X size={14} /> : <Server size={14} />}
                  </button>
                  <button
                    className="btn btn-icon btn-sm btn-danger-icon"
                    title="Remove environment"
                    onClick={() => handleRemove(idx)}
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              </div>

              {sshStatus[env.name] && (
                <div
                  className={`ssh-status ${
                    sshStatus[env.name] === 'connected'
                      ? 'ssh-ok'
                      : sshStatus[env.name] === 'testing...'
                        ? 'ssh-testing'
                        : 'ssh-fail'
                  }`}
                >
                  {sshStatus[env.name]}
                </div>
              )}

              {editing === idx && (
                <div className="env-edit-panel">
                  <div className="env-card-row">
                    <div className="form-group form-group-inline">
                      <label>SSH Host</label>
                      <input
                        className="input input-sm"
                        value={env.ssh_host ?? ''}
                        onChange={(e) =>
                          handleUpdate(idx, {
                            ssh_host: e.target.value || undefined,
                          })
                        }
                      />
                    </div>
                    <div className="form-group form-group-inline">
                      <label>Port</label>
                      <input
                        className="input input-sm input-narrow"
                        type="number"
                        value={env.ssh_port ?? ''}
                        onChange={(e) =>
                          handleUpdate(idx, {
                            ssh_port: e.target.value ? Number(e.target.value) : undefined,
                          })
                        }
                      />
                    </div>
                  </div>

                  <div className="env-vars-section">
                    <div className="env-vars-header">
                      <span className="env-vars-label">Variables</span>
                      <button className="btn btn-icon btn-sm" onClick={() => handleVarAdd(idx)}>
                        <Plus size={12} />
                      </button>
                    </div>
                    {Object.entries(env.variables).map(([key, val]) => (
                      <div key={key} className="env-var-row">
                        <input
                          className={`input input-sm${!isValidEnvVarName(key) ? ' input-error' : ''}`}
                          value={key}
                          title={
                            !isValidEnvVarName(key)
                              ? 'Must match [A-Za-z_][A-Za-z0-9_]* (e.g. MY_VAR)'
                              : undefined
                          }
                          onChange={(e) => {
                            const newName = e.target.value;
                            const vars = { ...env.variables };
                            delete vars[key];
                            vars[newName] = val;
                            handleUpdate(idx, { variables: vars });
                          }}
                        />
                        <input
                          className="input input-sm"
                          value={val}
                          onChange={(e) => handleVarChange(idx, key, e.target.value)}
                        />
                        <button
                          className="btn btn-icon btn-sm btn-danger-icon"
                          onClick={() => handleVarRemove(idx, key)}
                        >
                          <Trash2 size={12} />
                        </button>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export default EnvironmentEditor;
