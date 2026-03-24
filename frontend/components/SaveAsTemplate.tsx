import { useState } from 'react';
import { X, Save, AlertCircle } from 'lucide-react';
import { saveCustomTemplate } from '../services/api';
import type { Pipeline, PipelineTemplate, TemplateMeta } from '../types';

interface Props {
  /** The current pipeline to save as a template. */
  pipeline: Pipeline;
  /** Optional repo path for project-scope saving. */
  repoPath?: string;
  onSaved: (template: PipelineTemplate) => void;
  onCancel: () => void;
}

const CATEGORY_OPTIONS = [
  'rust',
  'node',
  'python',
  'go',
  'docker',
  'deployment',
  'other',
];

function SaveAsTemplate({ pipeline, repoPath, onSaved, onCancel }: Props) {
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [category, setCategory] = useState('other');
  const [tagsInput, setTagsInput] = useState('');
  const [scope, setScope] = useState<'user' | 'project'>('user');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSave() {
    if (!name.trim()) {
      setError('Template name is required.');
      return;
    }

    const tags = tagsInput
      .split(',')
      .map((t) => t.trim())
      .filter(Boolean);

    const meta: TemplateMeta = {
      name: name.trim(),
      description: description.trim(),
      author: '',
      version: '1.0.0',
      category,
      tags,
      project_types: [],
      required_tools: [],
      template_type: 'pipeline',
    };

    const template: PipelineTemplate = {
      meta,
      source: scope === 'user' ? 'user' : 'project',
      pipeline: { ...pipeline },
    };

    try {
      setSaving(true);
      setError(null);
      await saveCustomTemplate(template, scope, repoPath);
      onSaved(template);
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="modal-backdrop" onClick={onCancel}>
      <div className="modal" onClick={(e) => e.stopPropagation()} style={{ maxWidth: 480 }}>
        <div className="modal-header">
          <h3>Save as Template</h3>
          <button className="btn-icon" onClick={onCancel}>
            <X size={16} />
          </button>
        </div>

        <div className="modal-body">
          <p className="text-muted" style={{ fontSize: '0.8rem', marginBottom: '1rem' }}>
            Save the current pipeline as a reusable template.
          </p>

          <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
            {/* Name */}
            <div>
              <label style={{ display: 'block', fontSize: '0.8rem', fontWeight: 600, marginBottom: 4 }}>
                Template Name <span style={{ color: 'var(--danger)' }}>*</span>
              </label>
              <input
                className="input"
                type="text"
                placeholder="e.g. My Django Pipeline"
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            </div>

            {/* Description */}
            <div>
              <label style={{ display: 'block', fontSize: '0.8rem', fontWeight: 600, marginBottom: 4 }}>
                Description
              </label>
              <input
                className="input"
                type="text"
                placeholder="Brief description of what this template does"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
              />
            </div>

            {/* Category */}
            <div>
              <label style={{ display: 'block', fontSize: '0.8rem', fontWeight: 600, marginBottom: 4 }}>
                Category
              </label>
              <select
                className="input"
                value={category}
                onChange={(e) => setCategory(e.target.value)}
              >
                {CATEGORY_OPTIONS.map((c) => (
                  <option key={c} value={c}>
                    {c.charAt(0).toUpperCase() + c.slice(1)}
                  </option>
                ))}
              </select>
            </div>

            {/* Tags */}
            <div>
              <label style={{ display: 'block', fontSize: '0.8rem', fontWeight: 600, marginBottom: 4 }}>
                Tags
              </label>
              <input
                className="input"
                type="text"
                placeholder="Comma-separated, e.g. python, web, deploy"
                value={tagsInput}
                onChange={(e) => setTagsInput(e.target.value)}
              />
              <div className="text-muted" style={{ fontSize: '0.7rem', marginTop: 2 }}>
                Separate multiple tags with commas.
              </div>
            </div>

            {/* Scope */}
            <div>
              <label style={{ display: 'block', fontSize: '0.8rem', fontWeight: 600, marginBottom: 4 }}>
                Save To
              </label>
              <div style={{ display: 'flex', gap: '0.75rem' }}>
                <label style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: '0.8rem', cursor: 'pointer' }}>
                  <input
                    type="radio"
                    name="scope"
                    checked={scope === 'user'}
                    onChange={() => setScope('user')}
                  />
                  My Templates (global)
                </label>
                <label style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: '0.8rem', cursor: 'pointer' }}>
                  <input
                    type="radio"
                    name="scope"
                    checked={scope === 'project'}
                    onChange={() => setScope('project')}
                    disabled={!repoPath}
                  />
                  Project Templates
                </label>
              </div>
              <div className="text-muted" style={{ fontSize: '0.7rem', marginTop: 4 }}>
                {scope === 'user'
                  ? 'Saved to ~/.chibby/templates/ — available across all projects.'
                  : 'Saved to .chibby/templates/ — can be committed to version control.'}
              </div>
            </div>
          </div>

          {/* Preview */}
          <div style={{ marginTop: '1rem' }}>
            <div style={{ fontSize: '0.8rem', fontWeight: 600, marginBottom: 4 }}>
              Pipeline Preview ({pipeline.stages.length} stages)
            </div>
            <div
              style={{
                fontSize: '0.75rem',
                padding: '0.5rem',
                background: 'var(--bg)',
                borderRadius: 6,
                fontFamily: 'var(--font-mono, monospace)',
                maxHeight: 120,
                overflow: 'auto',
              }}
            >
              {pipeline.stages.map((s, i) => (
                <div key={i} style={{ marginBottom: 2 }}>
                  <span style={{ color: 'var(--accent)' }}>{s.name}</span>
                  <span className="text-muted"> — {s.commands.join(' && ')}</span>
                </div>
              ))}
            </div>
          </div>

          {error && (
            <div
              style={{
                marginTop: '0.75rem',
                padding: '0.5rem 0.75rem',
                borderRadius: 6,
                background: 'var(--danger-bg, rgba(239, 68, 68, 0.1))',
                color: 'var(--danger)',
                fontSize: '0.8rem',
                display: 'flex',
                alignItems: 'center',
                gap: '0.5rem',
              }}
            >
              <AlertCircle size={14} />
              {error}
            </div>
          )}
        </div>

        <div className="modal-footer">
          <button className="btn btn-secondary" onClick={onCancel} disabled={saving}>
            Cancel
          </button>
          <button
            className="btn btn-primary"
            onClick={handleSave}
            disabled={saving || !name.trim()}
          >
            <Save size={14} />
            {saving ? 'Saving…' : 'Save Template'}
          </button>
        </div>
      </div>
    </div>
  );
}

export default SaveAsTemplate;
