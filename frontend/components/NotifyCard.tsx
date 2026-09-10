import { useEffect, useState } from 'react';
import { Bell, Loader2, Plus, Trash2 } from 'lucide-react';
import HelpTip from './HelpTip';
import { HELP } from './project-detail/helpText';
import { loadNotifyConfig, saveNotifyConfig, sendTestNotification } from '../services/api';
import { notifyError, notifySuccess } from '../services/notify';
import type { NotifyChannel, NotifyConfig, NotifyOn, NotifyTarget } from '../types';

interface Props {
  repoPath: string;
}

const CHANNELS: NotifyChannel[] = ['desktop', 'webhook'];
const ON: NotifyOn[] = ['always', 'success', 'failure'];

function NotifyCard({ repoPath }: Props) {
  const [cfg, setCfg] = useState<NotifyConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);

  useEffect(() => {
    loadNotifyConfig(repoPath)
      .then(setCfg)
      .catch((err) => notifyError('Load notify config failed', err))
      .finally(() => setLoading(false));
  }, [repoPath]);

  function updateTarget(i: number, patch: Partial<NotifyTarget>) {
    setCfg((prev) => {
      if (!prev) return prev;
      const next = [...prev.targets];
      next[i] = { ...next[i], ...patch };
      return { ...prev, targets: next };
    });
  }

  function addTarget() {
    setCfg((prev) =>
      prev ? { ...prev, targets: [...prev.targets, { channel: 'desktop', on: 'always' }] } : prev
    );
  }

  function removeTarget(i: number) {
    setCfg((prev) => (prev ? { ...prev, targets: prev.targets.filter((_, j) => j !== i) } : prev));
  }

  async function handleSave() {
    if (!cfg) return;
    setSaving(true);
    try {
      await saveNotifyConfig(repoPath, cfg);
      notifySuccess('Notify config saved');
    } catch (err) {
      notifyError('Save failed', err);
    } finally {
      setSaving(false);
    }
  }

  async function handleTest() {
    setTesting(true);
    try {
      const msg = await sendTestNotification(repoPath);
      notifySuccess('Test sent', msg);
    } catch (err) {
      notifyError('Test failed', err);
    } finally {
      setTesting(false);
    }
  }

  return (
    <div className="feature-card">
      <div className="feature-card-header">
        <div className="feature-card-title">
          <Bell size={16} /> Notifications
          <HelpTip label="Notifications">{HELP.notifications}</HelpTip>
        </div>
        <div className="feature-card-actions">
          <button
            className="btn btn-sm btn-ghost"
            onClick={handleTest}
            disabled={testing || !cfg?.enabled}
          >
            {testing ? 'Sending…' : 'Send test'}
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
                onChange={(e) => setCfg({ ...cfg, enabled: e.target.checked })}
              />
              Enabled
            </label>

            {cfg.targets.map((t, i) => (
              <div key={i} className="fc-row">
                <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
                  <select
                    className="input"
                    style={{ width: 120 }}
                    value={t.channel}
                    onChange={(e) => updateTarget(i, { channel: e.target.value as NotifyChannel })}
                  >
                    {CHANNELS.map((c) => (
                      <option key={c} value={c}>
                        {c}
                      </option>
                    ))}
                  </select>
                  <select
                    className="input"
                    style={{ width: 100 }}
                    value={t.on}
                    onChange={(e) => updateTarget(i, { on: e.target.value as NotifyOn })}
                  >
                    {ON.map((o) => (
                      <option key={o} value={o}>
                        {o}
                      </option>
                    ))}
                  </select>
                  {t.channel === 'webhook' && (
                    <input
                      className="input"
                      style={{ flex: 1, minWidth: 200 }}
                      placeholder="https://hooks.slack.com/…"
                      value={t.url ?? ''}
                      onChange={(e) => updateTarget(i, { url: e.target.value })}
                    />
                  )}
                </div>
                <button className="btn btn-xs btn-danger-icon" onClick={() => removeTarget(i)}>
                  <Trash2 size={12} />
                </button>
              </div>
            ))}

            <button className="btn btn-sm btn-ghost" onClick={addTarget}>
              <Plus size={12} /> Add target
            </button>

            {/* Runs no human was watching — schedules and file watches. */}
            <div className="notify-unattended">
              <p className="notify-unattended-title">Unattended runs</p>
              <p className="section-hint">
                Schedules and file watches run with nobody looking. These escalate past the settings
                above.
              </p>
              <label className="settings-toggle">
                <input
                  type="checkbox"
                  checked={cfg.unattended.always_on_failure}
                  onChange={(e) =>
                    setCfg({
                      ...cfg,
                      unattended: { ...cfg.unattended, always_on_failure: e.target.checked },
                    })
                  }
                />
                <span>Always notify when an unattended run fails</span>
              </label>
              <label className="settings-toggle">
                <input
                  type="checkbox"
                  checked={cfg.unattended.on_success}
                  onChange={(e) =>
                    setCfg({
                      ...cfg,
                      unattended: { ...cfg.unattended, on_success: e.target.checked },
                    })
                  }
                />
                <span>Also notify when an unattended run succeeds</span>
              </label>
            </div>
          </>
        ) : null}
      </div>
    </div>
  );
}

export default NotifyCard;
