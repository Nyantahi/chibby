import { useState } from 'react';
import { ChevronDown, ChevronRight, Plus, Trash2, TriangleAlert, X } from 'lucide-react';
import {
  DEFAULT_ROLLBACK_POLICY,
  type Backoff,
  type RollbackMode,
  type RollbackPolicy,
  type Stage,
  type StageRetry,
  type StageWhen,
} from '../../types';

interface Props {
  stage: Stage;
  /** Same shape as `PipelineEditor.updateStage(idx, updates)`. */
  onChange: (updates: Partial<Stage>) => void;
}

// Mirrors the Rust serde defaults in engine/models/pipeline.rs (default_attempts / default_retry_delay).
const DEFAULT_RETRY: StageRetry = { attempts: 2, delay_secs: 5, backoff: 'fixed' };
const EMPTY_WHEN: StageWhen = { branch: [], branch_not: [], environment: [], environment_not: [] };

/** Split a comma-separated input into a trimmed glob list. */
function parseGlobs(value: string): string[] {
  return value
    .split(',')
    .map((v) => v.trim())
    .filter(Boolean);
}

/** How many advanced options are configured, for the collapsed summary. */
function countConfigured(stage: Stage): number {
  const whenSet =
    !!stage.when &&
    stage.when.branch.length +
      stage.when.branch_not.length +
      stage.when.environment.length +
      stage.when.environment_not.length >
      0;
  return [
    stage.timeout_secs !== undefined,
    !!stage.retry,
    whenSet,
    Object.keys(stage.env ?? {}).length > 0,
    rollbackMode(stage) !== 'off',
  ].filter(Boolean).length;
}

/** Effective rollback mode for this stage; absent policy means `off`. */
function rollbackMode(stage: Stage): RollbackMode {
  return stage.on_health_failure?.mode ?? 'off';
}

/**
 * Timeout / retry / `when` / per-stage env / auto-rollback editing for one stage.
 * Collapsed by default — these are advanced options most stages never need.
 */
function StageAdvancedFields({ stage, onChange }: Props) {
  const [open, setOpen] = useState(false);
  const configured = countConfigured(stage);
  const when = stage.when;
  const retry = stage.retry;
  const env = stage.env ?? {};
  const mode = rollbackMode(stage);
  const rollbackCommands = stage.rollback_commands ?? [''];
  // A rollback policy only ever fires from a failed health check.
  const rollbackIsDead = mode !== 'off' && !stage.health_check;

  function handleModeChange(next: RollbackMode) {
    if (next === 'off') {
      onChange({ on_health_failure: undefined });
      return;
    }
    const policy: RollbackPolicy = { ...DEFAULT_ROLLBACK_POLICY, ...stage.on_health_failure };
    onChange({
      on_health_failure: { ...policy, mode: next },
      rollback_commands:
        next === 'commands' ? (stage.rollback_commands ?? ['']) : stage.rollback_commands,
    });
  }

  function handleUpdatePolicy(updates: Partial<RollbackPolicy>) {
    if (!stage.on_health_failure) return;
    onChange({ on_health_failure: { ...stage.on_health_failure, ...updates } });
  }

  function handleUpdateRollbackCommand(cmdIdx: number, value: string) {
    const cmds = [...rollbackCommands];
    cmds[cmdIdx] = value;
    onChange({ rollback_commands: cmds });
  }

  function handleAddRollbackCommand() {
    onChange({ rollback_commands: [...rollbackCommands, ''] });
  }

  function handleRemoveRollbackCommand(cmdIdx: number) {
    const cmds = rollbackCommands.filter((_, i) => i !== cmdIdx);
    onChange({ rollback_commands: cmds.length > 0 ? cmds : [''] });
  }

  function handleToggleRetry() {
    onChange({ retry: retry ? undefined : { ...DEFAULT_RETRY } });
  }

  function handleUpdateRetry(updates: Partial<StageRetry>) {
    if (!retry) return;
    onChange({ retry: { ...retry, ...updates } });
  }

  function handleToggleWhen() {
    onChange({ when: when ? undefined : { ...EMPTY_WHEN } });
  }

  function handleUpdateWhen(updates: Partial<StageWhen>) {
    if (!when) return;
    onChange({ when: { ...when, ...updates } });
  }

  function handleEnvAdd() {
    onChange({ env: { ...env, [`VAR_${Object.keys(env).length + 1}`]: '' } });
  }

  // Rows are addressed by position, not by name: the name is what the user is
  // editing, so a name-keyed row would be a different row on every keystroke.
  function handleEnvRename(index: number, newKey: string) {
    replaceEnvRow(index, ([, value]) => [newKey, value]);
  }

  function handleEnvValue(index: number, value: string) {
    replaceEnvRow(index, ([key]) => [key, value]);
  }

  function replaceEnvRow(index: number, replace: (row: [string, string]) => [string, string]) {
    const rows = Object.entries(env).map((row, i) => (i === index ? replace(row) : row));
    onChange({ env: Object.fromEntries(rows) });
  }

  function handleEnvRemove(index: number) {
    const rows = Object.entries(env).filter((_, i) => i !== index);
    onChange({ env: rows.length > 0 ? Object.fromEntries(rows) : undefined });
  }

  return (
    <div className="pe-advanced-section">
      <button className="pe-advanced-toggle" onClick={() => setOpen(!open)}>
        {open ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        Advanced
        {configured > 0 && <span className="badge badge-neutral">{configured}</span>}
      </button>

      {open && (
        <div className="pe-advanced-fields">
          {/* Timeout + retry policy */}
          <div className="pe-field-row">
            <div className="form-group form-group-inline">
              <label htmlFor={`timeout-${stage.name}`}>Timeout (s)</label>
              <input
                id={`timeout-${stage.name}`}
                className="input input-sm input-narrow"
                type="number"
                min={1}
                value={stage.timeout_secs ?? ''}
                placeholder="none"
                onChange={(e) =>
                  onChange({ timeout_secs: e.target.value ? Number(e.target.value) : undefined })
                }
              />
            </div>
            <div className="form-group form-group-inline pe-checkbox-group">
              <label>
                <input type="checkbox" checked={!!retry} onChange={handleToggleRetry} /> Retry on
                failure
              </label>
            </div>
            {retry && (
              <>
                <div className="form-group form-group-inline">
                  <label htmlFor={`retry-attempts-${stage.name}`}>Attempts</label>
                  <input
                    id={`retry-attempts-${stage.name}`}
                    className="input input-sm input-narrow"
                    type="number"
                    min={1}
                    value={retry.attempts}
                    onChange={(e) => handleUpdateRetry({ attempts: Number(e.target.value) || 1 })}
                  />
                </div>
                <div className="form-group form-group-inline">
                  <label htmlFor={`retry-delay-${stage.name}`}>Delay (s)</label>
                  <input
                    id={`retry-delay-${stage.name}`}
                    className="input input-sm input-narrow"
                    type="number"
                    min={0}
                    value={retry.delay_secs}
                    onChange={(e) => handleUpdateRetry({ delay_secs: Number(e.target.value) || 0 })}
                  />
                </div>
                <div className="form-group form-group-inline">
                  <label htmlFor={`retry-backoff-${stage.name}`}>Backoff</label>
                  <select
                    id={`retry-backoff-${stage.name}`}
                    className="input input-sm"
                    value={retry.backoff}
                    onChange={(e) => handleUpdateRetry({ backoff: e.target.value as Backoff })}
                  >
                    <option value="fixed">Fixed</option>
                    <option value="exponential">Exponential</option>
                  </select>
                </div>
              </>
            )}
          </div>

          {/* Run conditions */}
          <div className="form-group form-group-inline pe-checkbox-group">
            <label>
              <input type="checkbox" checked={!!when} onChange={handleToggleWhen} /> Only run when
            </label>
          </div>
          {when && (
            <div className="pe-field-row">
              <div className="form-group form-group-inline">
                <label htmlFor={`when-branch-${stage.name}`}>Branch</label>
                <input
                  id={`when-branch-${stage.name}`}
                  className="input input-sm"
                  value={when.branch.join(', ')}
                  placeholder="main, release/*"
                  onChange={(e) => handleUpdateWhen({ branch: parseGlobs(e.target.value) })}
                />
              </div>
              <div className="form-group form-group-inline">
                <label htmlFor={`when-branch-not-${stage.name}`}>Branch except</label>
                <input
                  id={`when-branch-not-${stage.name}`}
                  className="input input-sm"
                  value={when.branch_not.join(', ')}
                  placeholder="wip/*"
                  onChange={(e) => handleUpdateWhen({ branch_not: parseGlobs(e.target.value) })}
                />
              </div>
              <div className="form-group form-group-inline">
                <label htmlFor={`when-env-${stage.name}`}>Environment</label>
                <input
                  id={`when-env-${stage.name}`}
                  className="input input-sm"
                  value={when.environment.join(', ')}
                  placeholder="production"
                  onChange={(e) => handleUpdateWhen({ environment: parseGlobs(e.target.value) })}
                />
              </div>
              <div className="form-group form-group-inline">
                <label htmlFor={`when-env-not-${stage.name}`}>Environment except</label>
                <input
                  id={`when-env-not-${stage.name}`}
                  className="input input-sm"
                  value={when.environment_not.join(', ')}
                  placeholder="staging"
                  onChange={(e) =>
                    handleUpdateWhen({ environment_not: parseGlobs(e.target.value) })
                  }
                />
              </div>
            </div>
          )}

          {/* Auto-rollback on health check failure */}
          <div className="pe-field-row">
            <div className="form-group form-group-inline">
              <label htmlFor={`rollback-mode-${stage.name}`}>On health check failure</label>
              <select
                id={`rollback-mode-${stage.name}`}
                className="input input-sm"
                value={mode}
                onChange={(e) => handleModeChange(e.target.value as RollbackMode)}
              >
                <option value="off">Off</option>
                <option value="last_good">Roll back to last good</option>
                <option value="commands">Run rollback commands</option>
              </select>
            </div>
            {stage.on_health_failure && (
              <>
                <div className="form-group form-group-inline pe-checkbox-group">
                  <label>
                    <input
                      type="checkbox"
                      checked={stage.on_health_failure.verify_health}
                      onChange={(e) => handleUpdatePolicy({ verify_health: e.target.checked })}
                    />{' '}
                    Verify health after rollback
                  </label>
                </div>
                <div className="form-group form-group-inline">
                  <label htmlFor={`rollback-attempts-${stage.name}`}>Max attempts</label>
                  <input
                    id={`rollback-attempts-${stage.name}`}
                    className="input input-sm input-narrow"
                    type="number"
                    min={1}
                    value={stage.on_health_failure.max_attempts}
                    onChange={(e) =>
                      handleUpdatePolicy({ max_attempts: Number(e.target.value) || 1 })
                    }
                  />
                </div>
              </>
            )}
          </div>
          {rollbackIsDead && (
            <p className="pe-field-hint pe-field-hint-warning">
              <TriangleAlert size={12} /> This stage has no health check, so the rollback will never
              fire. Add a health check to make it do anything.
            </p>
          )}
          {mode === 'commands' && (
            <div className="pe-commands-section">
              <div className="pe-commands-header">
                <span className="env-vars-label">Rollback Commands</span>
                <button
                  className="btn btn-icon btn-sm"
                  onClick={handleAddRollbackCommand}
                  title="Add rollback command"
                >
                  <Plus size={12} />
                </button>
              </div>
              {rollbackCommands.map((cmd, ci) => (
                <div key={ci} className="pe-command-row">
                  <span className="pe-command-prefix">$</span>
                  <input
                    className="input input-sm pe-command-input"
                    value={cmd}
                    onChange={(e) => handleUpdateRollbackCommand(ci, e.target.value)}
                    placeholder="command..."
                  />
                  {rollbackCommands.length > 1 && (
                    <button
                      className="btn btn-icon btn-sm"
                      onClick={() => handleRemoveRollbackCommand(ci)}
                    >
                      <X size={12} />
                    </button>
                  )}
                </div>
              ))}
            </div>
          )}

          {/* Per-stage environment variables */}
          <div className="env-vars-section">
            <div className="env-vars-header">
              <span className="env-vars-label">Stage Environment</span>
              <button className="btn btn-icon btn-sm" onClick={handleEnvAdd} title="Add variable">
                <Plus size={12} />
              </button>
            </div>
            {Object.entries(env).map(([key, value], index) => (
              // Keyed by position: the row's identity is where it sits, and
              // keying by the name being typed would make every keystroke a
              // different row — remounting the input and dropping focus.
              <div key={index} className="env-var-row">
                <input
                  className="input input-sm"
                  value={key}
                  onChange={(e) => handleEnvRename(index, e.target.value)}
                  aria-label={`Variable ${index + 1} name`}
                />
                <input
                  className="input input-sm"
                  value={value}
                  onChange={(e) => handleEnvValue(index, e.target.value)}
                  aria-label={`Variable ${index + 1} value`}
                />
                <button
                  className="btn btn-icon btn-sm btn-danger-icon"
                  onClick={() => handleEnvRemove(index)}
                  title="Remove variable"
                >
                  <Trash2 size={12} />
                </button>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

export default StageAdvancedFields;
