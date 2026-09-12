import { useCallback, useEffect, useState } from 'react';
import { Info, Loader2, Save, Laptop, Users } from 'lucide-react';
import type { TriggersConfig, TriggerStateEntry } from '../../types';
import {
  fireTriggerNow,
  getTriggerState,
  loadTriggers,
  loadTriggersLocal,
  saveTriggers,
} from '../../services/api';
import { notifyError, notifySuccess } from '../../services/notify';
import HelpTip from '../HelpTip';
import SchedulesSection from '../triggers/SchedulesSection';
import WatchesSection from '../triggers/WatchesSection';
import HooksSection from '../triggers/HooksSection';
import { HELP } from './helpText';

const EMPTY_CONFIG: TriggersConfig = {
  enabled: false,
  schedules: [],
  watches: [],
  hooks: {},
};

interface TriggersTabProps {
  repoPath: string;
  environments: string[];
}

/** Which file the editor is reading and writing. */
type Scope = 'shared' | 'local';

const SCOPE_FILE: Record<Scope, string> = {
  shared: 'triggers.toml',
  local: 'triggers.local.toml',
};

/**
 * The editor works on exactly one file at a time, never on the merged view:
 * saving the merge would publish a developer's machine-local triggers into the
 * committed file, and copying it into the local file would shadow every later
 * team edit.
 */
function TriggersTab({ repoPath, environments }: TriggersTabProps) {
  const [scope, setScope] = useState<Scope>('shared');
  const [config, setConfig] = useState<TriggersConfig | null>(null);
  const [triggerState, setTriggerState] = useState<Record<string, TriggerStateEntry>>({});
  const [loadError, setLoadError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [firingId, setFiringId] = useState<string | null>(null);

  const refreshState = useCallback(() => {
    getTriggerState(repoPath)
      .then(setTriggerState)
      .catch(() => setTriggerState({}));
  }, [repoPath]);

  useEffect(() => {
    let ignore = false;
    const load = scope === 'local' ? loadTriggersLocal(repoPath) : loadTriggers(repoPath, false);
    load
      .then((cfg) => {
        if (!ignore) {
          setConfig(cfg ?? EMPTY_CONFIG);
          setLoadError(null);
        }
      })
      .catch((err) => {
        if (!ignore) {
          setConfig(EMPTY_CONFIG);
          setLoadError(String(err));
        }
      });
    return () => {
      ignore = true;
    };
  }, [repoPath, scope]);

  useEffect(() => {
    refreshState();
  }, [refreshState]);

  async function handleSave() {
    if (!config) return;
    setSaving(true);
    try {
      await saveTriggers(repoPath, config, scope === 'local');
      notifySuccess(
        scope === 'local'
          ? 'Saved to triggers.local.toml (this machine only)'
          : 'Saved to triggers.toml'
      );
    } catch (err) {
      notifyError('Save triggers failed', err);
    } finally {
      setSaving(false);
    }
  }

  async function handleRunNow(triggerId: string) {
    setFiringId(triggerId);
    try {
      const run = await fireTriggerNow(repoPath, triggerId);
      notifySuccess(`Trigger "${triggerId}" fired`, `Run ${run.id} — ${run.status}`);
      refreshState();
    } catch (err) {
      notifyError(`Trigger "${triggerId}" failed`, err);
    } finally {
      setFiringId(null);
    }
  }

  if (!config) {
    return (
      <div className="feature-card-empty">
        <Loader2 size={14} className="spin" /> Loading triggers…
      </div>
    );
  }

  return (
    <>
      {loadError && <div className="alert alert-error">{loadError}</div>}

      <section className="section">
        <div className="section-header-row">
          <h3 className="section-title">
            <Info size={16} />
            Triggers
            <HelpTip label="Triggers">{HELP.triggers}</HelpTip>
          </h3>
          <div className="trigger-save-actions">
            <button
              className={`btn btn-sm ${scope === 'shared' ? 'btn-secondary' : 'btn-ghost'}`}
              onClick={() => setScope('shared')}
              disabled={saving}
              title="Edit .chibby/triggers.toml — committed, shared with the team"
            >
              <Users size={14} /> Shared
            </button>
            <button
              className={`btn btn-sm ${scope === 'local' ? 'btn-secondary' : 'btn-ghost'}`}
              onClick={() => setScope('local')}
              disabled={saving}
              title="Edit .chibby/triggers.local.toml — gitignored, so it never fires on a teammate's machine"
            >
              <Laptop size={14} /> This machine
            </button>
            <button className="btn btn-sm btn-primary" onClick={handleSave} disabled={saving}>
              <Save size={14} /> {saving ? 'Saving…' : `Save ${SCOPE_FILE[scope]}`}
            </button>
          </div>
        </div>

        <div className="trigger-note">
          <p>
            Editing <code>.chibby/{SCOPE_FILE[scope]}</code>
            {scope === 'local'
              ? ' — gitignored, and layered over the shared file at run time.'
              : ' — committed, and shared with everyone on the repo.'}
          </p>
          <p>
            Schedules and file watches only fire <strong>while something is running</strong> — this
            app open, or <code>chibby schedule</code> / <code>chibby watch</code> in a terminal.
            Chibby installs no background daemon.
          </p>
          <p>
            To fire without the app open, wire <code>chibby schedule --once</code> into launchd,
            systemd or Task Scheduler. Git hooks are the exception — git runs those itself.
          </p>
        </div>

        <label className="settings-toggle">
          <input
            type="checkbox"
            checked={config.enabled}
            onChange={(e) => setConfig({ ...config, enabled: e.target.checked })}
          />
          <span>Triggers enabled for this repo</span>
        </label>
        <p className="section-hint">
          The master switch. Off means no schedule ticks and no watchers, whatever is configured
          below.
        </p>
      </section>

      <SchedulesSection
        schedules={config.schedules}
        triggerState={triggerState}
        environments={environments}
        firingId={firingId}
        onChange={(schedules) => setConfig({ ...config, schedules })}
        onRunNow={handleRunNow}
      />

      <WatchesSection
        watches={config.watches}
        triggerState={triggerState}
        environments={environments}
        firingId={firingId}
        onChange={(watches) => setConfig({ ...config, watches })}
        onRunNow={handleRunNow}
      />

      <HooksSection
        repoPath={repoPath}
        hooks={config.hooks}
        environments={environments}
        onChange={(hooks) => setConfig({ ...config, hooks })}
      />
    </>
  );
}

export default TriggersTab;
