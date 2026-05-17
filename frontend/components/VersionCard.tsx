import { useEffect, useState } from 'react';
import { Tag, Loader2, Copy } from 'lucide-react';
import { bumpVersion, detectVersions, generateChangelog } from '../services/api';
import { notifyError, notifySuccess } from '../services/notify';
import type { BumpLevel, BumpResult, ChangelogEntry, VersionInfo } from '../types';

interface Props {
  repoPath: string;
}

function VersionCard({ repoPath }: Props) {
  const [info, setInfo] = useState<VersionInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [bumping, setBumping] = useState<BumpLevel | null>(null);
  const [createTag, setCreateTag] = useState(true);
  const [lastBump, setLastBump] = useState<BumpResult | null>(null);
  const [changelog, setChangelog] = useState<ChangelogEntry[] | null>(null);
  const [loadingChangelog, setLoadingChangelog] = useState(false);

  useEffect(() => {
    setLoading(true);
    detectVersions(repoPath)
      .then(setInfo)
      .catch((err) => notifyError('Failed to detect versions', err))
      .finally(() => setLoading(false));
  }, [repoPath]);

  async function handleBump(level: BumpLevel) {
    setBumping(level);
    try {
      const r = await bumpVersion(repoPath, level, undefined, createTag);
      setLastBump(r);
      notifySuccess(`Bumped to ${r.new_version}`, r.git_tag ?? '');
      const next = await detectVersions(repoPath);
      setInfo(next);
    } catch (err) {
      notifyError('Bump failed', err);
    } finally {
      setBumping(null);
    }
  }

  async function handleChangelog() {
    setLoadingChangelog(true);
    try {
      const entries = await generateChangelog(repoPath, info?.latest_tag);
      setChangelog(entries);
    } catch (err) {
      notifyError('Changelog failed', err);
    } finally {
      setLoadingChangelog(false);
    }
  }

  function copyChangelog() {
    if (!changelog) return;
    const text = changelog.map((c) => `- ${c.subject} (${c.hash.slice(0, 7)})`).join('\n');
    navigator.clipboard.writeText(text);
    notifySuccess('Changelog copied');
  }

  return (
    <div className="feature-card">
      <div className="feature-card-header">
        <div className="feature-card-title">
          <Tag size={16} /> Version
        </div>
      </div>
      <div className="feature-card-body">
        {loading ? (
          <div className="feature-card-empty">
            <Loader2 size={14} className="spin" /> Detecting…
          </div>
        ) : !info ? (
          <div className="feature-card-empty">No version files detected.</div>
        ) : (
          <>
            <div className="bw-summary" style={{ marginBottom: 0 }}>
              <span>
                current:{' '}
                <strong>{info.current_version ?? '—'}</strong>
              </span>
              <span>
                files: <strong>{info.files.length}</strong>
              </span>
              <span>
                tag: <strong>{info.latest_tag ?? '—'}</strong>
              </span>
              <span style={{ color: info.is_consistent ? 'var(--color-success)' : 'var(--color-failed)' }}>
                {info.is_consistent ? 'consistent ✓' : 'inconsistent !'}
              </span>
            </div>
            <table className="kv-table">
              <thead>
                <tr>
                  <th>File</th>
                  <th>Version</th>
                </tr>
              </thead>
              <tbody>
                {info.files.map((f) => (
                  <tr key={f.path}>
                    <td>
                      <code>{f.path}</code>
                    </td>
                    <td>{f.version}</td>
                  </tr>
                ))}
              </tbody>
            </table>

            <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
              <button
                className="btn btn-sm btn-secondary"
                onClick={() => handleBump('patch')}
                disabled={bumping !== null}
              >
                {bumping === 'patch' ? 'Bumping…' : 'Bump patch'}
              </button>
              <button
                className="btn btn-sm btn-secondary"
                onClick={() => handleBump('minor')}
                disabled={bumping !== null}
              >
                {bumping === 'minor' ? 'Bumping…' : 'Bump minor'}
              </button>
              <button
                className="btn btn-sm btn-secondary"
                onClick={() => handleBump('major')}
                disabled={bumping !== null}
              >
                {bumping === 'major' ? 'Bumping…' : 'Bump major'}
              </button>
              <label
                style={{ fontSize: 'var(--font-size-xs)', color: 'var(--color-text-muted)', display: 'flex', gap: 4, alignItems: 'center', marginLeft: 'auto' }}
              >
                <input
                  type="checkbox"
                  checked={createTag}
                  onChange={(e) => setCreateTag(e.target.checked)}
                  disabled={bumping !== null}
                />
                Create git tag
              </label>
            </div>

            {lastBump && (
              <div className="text-muted" style={{ fontSize: 'var(--font-size-xs)' }}>
                Last bump: {lastBump.old_version} → <strong>{lastBump.new_version}</strong>
                {lastBump.git_tag && <> (tag: <code>{lastBump.git_tag}</code>)</>}
              </div>
            )}

            <div style={{ display: 'flex', gap: 8 }}>
              <button
                className="btn btn-sm btn-ghost"
                onClick={handleChangelog}
                disabled={loadingChangelog}
              >
                {loadingChangelog ? 'Loading…' : 'Generate changelog'}
              </button>
              {changelog && changelog.length > 0 && (
                <button className="btn btn-sm btn-ghost" onClick={copyChangelog}>
                  <Copy size={12} /> Copy
                </button>
              )}
            </div>

            {changelog && (
              <table className="kv-table">
                <thead>
                  <tr>
                    <th>Commit</th>
                    <th>Subject</th>
                  </tr>
                </thead>
                <tbody>
                  {changelog.length === 0 ? (
                    <tr>
                      <td colSpan={2} style={{ color: 'var(--color-text-muted)' }}>
                        No commits since {info.latest_tag ?? 'beginning of history'}.
                      </td>
                    </tr>
                  ) : (
                    changelog.map((c) => (
                      <tr key={c.hash}>
                        <td>
                          <code>{c.hash.slice(0, 7)}</code>
                        </td>
                        <td>{c.subject}</td>
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            )}
          </>
        )}
      </div>
    </div>
  );
}

export default VersionCard;
