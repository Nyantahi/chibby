import { useCallback, useEffect, useState } from 'react';
import { Check, Copy, GitFork, Loader2, Trash2 } from 'lucide-react';
import type { HookKind, HookSpec, HookState, InstallMode, InstallReport } from '../../types';
import { gitHookStatus, installGitHooks, uninstallGitHooks } from '../../services/api';
import { notifyError, notifySuccess } from '../../services/notify';
import {
  HOOK_DESCRIPTIONS,
  HOOK_LABELS,
  HOOK_STATE_BADGE,
  HOOK_STATE_LABELS,
  formatList,
  newHookSpec,
  parseList,
} from './helpers';

interface HookCardProps {
  repoPath: string;
  kind: HookKind;
  spec?: HookSpec;
  environments: string[];
  onSpecChange: (spec: HookSpec | undefined) => void;
}

function HookCard({ repoPath, kind, spec, environments, onSpecChange }: HookCardProps) {
  const [status, setStatus] = useState<HookState | null>(null);
  const [report, setReport] = useState<InstallReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [copied, setCopied] = useState(false);

  const effective = spec ?? newHookSpec();
  const [stagesText, setStagesText] = useState(formatList(effective.stages));

  const refreshStatus = useCallback(() => {
    gitHookStatus(repoPath, kind)
      .then(setStatus)
      .catch(() => setStatus(null));
  }, [repoPath, kind]);

  useEffect(() => {
    refreshStatus();
  }, [refreshStatus]);

  function patchSpec(patch: Partial<HookSpec>) {
    onSpecChange({ ...effective, ...patch });
  }

  async function handleInstall(mode?: InstallMode) {
    setBusy(true);
    setCopied(false);
    try {
      const next = { ...effective, stages: parseList(stagesText) };
      onSpecChange(next);
      const result = await installGitHooks(repoPath, kind, next, mode);
      setReport(result);
      if (result.installed) notifySuccess(result.message);
      refreshStatus();
    } catch (err) {
      notifyError(`Install ${HOOK_LABELS[kind]} hook failed`, err);
    } finally {
      setBusy(false);
    }
  }

  async function handleRemove() {
    setBusy(true);
    try {
      await uninstallGitHooks(repoPath, kind);
      setReport(null);
      onSpecChange(undefined);
      notifySuccess(`Removed Chibby's ${HOOK_LABELS[kind]} block`);
      refreshStatus();
    } catch (err) {
      notifyError(`Remove ${HOOK_LABELS[kind]} hook failed`, err);
    } finally {
      setBusy(false);
    }
  }

  async function handleCopySnippet() {
    if (!report) return;
    try {
      await navigator.clipboard.writeText(report.snippet);
      setCopied(true);
    } catch (err) {
      notifyError('Copy failed', err);
    }
  }

  const installed = status === 'chibby_managed' || status === 'foreign_with_chibby_block';
  /* `installed: false` is the expected outcome when a foreign hook is present —
     it is a choice to offer, not a failure to report. */
  const conflict = report !== null && !report.installed;

  return (
    <div className="feature-card">
      <div className="feature-card-header">
        <div className="feature-card-title">
          <GitFork size={16} /> {HOOK_LABELS[kind]}
          {status && (
            <span className={`badge badge-${HOOK_STATE_BADGE[status]}`}>
              {HOOK_STATE_LABELS[status]}
            </span>
          )}
        </div>
        <div className="feature-card-actions">
          <button
            className="btn btn-sm btn-secondary"
            onClick={() => handleInstall()}
            disabled={busy}
          >
            {busy ? <Loader2 size={14} className="spin" /> : <Check size={14} />}
            {installed ? 'Reinstall' : 'Install'}
          </button>
          <button
            className="btn btn-sm btn-ghost"
            onClick={handleRemove}
            disabled={busy || !installed}
            title="Remove Chibby's block, leaving any hook of your own intact"
          >
            <Trash2 size={14} /> Remove
          </button>
        </div>
      </div>

      <div className="feature-card-body">
        <p className="section-hint">{HOOK_DESCRIPTIONS[kind]}</p>

        <div className="trigger-editor-grid">
          <div className="form-group">
            <label className="form-label" htmlFor={`${kind}-stages`}>
              Stages
            </label>
            <input
              id={`${kind}-stages`}
              className="input input-sm"
              value={stagesText}
              onChange={(e) => setStagesText(e.target.value)}
              onBlur={() => patchSpec({ stages: parseList(stagesText) })}
              placeholder="test, lint — empty runs every stage"
            />
          </div>

          <div className="form-group">
            <label className="form-label" htmlFor={`${kind}-env`}>
              Environment
            </label>
            <select
              id={`${kind}-env`}
              className="input input-sm"
              value={effective.environment ?? ''}
              onChange={(e) => patchSpec({ environment: e.target.value || undefined })}
            >
              <option value="">None</option>
              {environments.map((env) => (
                <option key={env} value={env}>
                  {env}
                </option>
              ))}
            </select>
          </div>

          <div className="form-group">
            <label className="form-label" htmlFor={`${kind}-pipeline`}>
              Pipeline file
            </label>
            <input
              id={`${kind}-pipeline`}
              className="input input-sm"
              value={effective.pipeline_file ?? ''}
              onChange={(e) => patchSpec({ pipeline_file: e.target.value || undefined })}
              placeholder="pipeline (default)"
            />
          </div>
        </div>

        <label className="settings-toggle">
          <input
            type="checkbox"
            checked={effective.blocking}
            onChange={(e) => patchSpec({ blocking: e.target.checked })}
          />
          <span>Block the git operation when the run fails</span>
        </label>

        {report?.installed && (
          <p className="hook-result-ok">
            <Check size={12} /> {report.message}
            {report.backup_path && <> Your previous hook was backed up to {report.backup_path}.</>}
          </p>
        )}

        {conflict && report && (
          <div className="hook-conflict">
            <p className="hook-conflict-title">
              You already have your own {HOOK_LABELS[kind]} hook
            </p>
            <p className="hook-conflict-message">{report.message}</p>
            <p className="hook-conflict-message">
              Nothing was changed. Paste the block below into your hook by hand, or pick one of
              these:
            </p>
            <pre className="hook-snippet">
              <code>{report.snippet}</code>
            </pre>
            <div className="hook-conflict-actions">
              <button className="btn btn-sm btn-ghost" onClick={handleCopySnippet}>
                <Copy size={14} /> {copied ? 'Copied' : 'Copy snippet'}
              </button>
              <button
                className="btn btn-sm btn-secondary"
                onClick={() => handleInstall('append')}
                disabled={busy}
                title="Insert Chibby's block after the shebang and keep your hook"
              >
                Append to my hook
              </button>
              <button
                className="btn btn-sm btn-warning"
                onClick={() => handleInstall('force')}
                disabled={busy}
                title="Back your hook up alongside it, then replace it with Chibby's"
              >
                Back up &amp; replace
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export default HookCard;
