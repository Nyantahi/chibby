import { useState, useEffect } from 'react';
import { X, AlertCircle } from 'lucide-react';
import { getTemplateVariables, applyTemplate } from '../services/api';
import type { Pipeline, PipelineTemplate, TemplateVariable } from '../types';

interface Props {
  template: PipelineTemplate;
  repoPath?: string;
  /** Pre-fill {{project_name}} with this value. */
  projectName?: string;
  /** Called with the resulting Pipeline after successful variable substitution. */
  onApplied: (pipeline: Pipeline) => void;
  onCancel: () => void;
}

function TemplateVariableDialog({ template, repoPath, projectName, onApplied, onCancel }: Props) {
  const [variables, setVariables] = useState<TemplateVariable[]>([]);
  const [values, setValues] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(true);
  const [applying, setApplying] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    loadVariables();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [template.meta.name]);

  async function loadVariables() {
    try {
      setLoading(true);
      setError(null);
      const vars = await getTemplateVariables(template.meta.name, repoPath);
      setVariables(vars);

      // Pre-fill defaults
      const defaults: Record<string, string> = {};
      for (const v of vars) {
        if (v.name === 'project_name' && projectName) {
          defaults[v.name] = projectName;
        } else if (v.default_value) {
          defaults[v.name] = v.default_value;
        } else {
          defaults[v.name] = '';
        }
      }
      setValues(defaults);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  async function handleApply() {
    // Validate required fields
    const missing = variables
      .filter((v) => v.required && !values[v.name]?.trim())
      .map((v) => v.name);

    if (missing.length > 0) {
      setError(`Missing required variables: ${missing.join(', ')}`);
      return;
    }

    try {
      setApplying(true);
      setError(null);
      const pipeline = await applyTemplate(template.meta.name, values, repoPath);
      onApplied(pipeline);
    } catch (err) {
      setError(String(err));
    } finally {
      setApplying(false);
    }
  }

  // If template has no variables, apply immediately
  useEffect(() => {
    if (!loading && variables.length === 0) {
      handleApply();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loading, variables.length]);

  // No variables — just a loading spinner while we apply
  if (!loading && variables.length === 0) {
    return null;
  }

  return (
    <div className="modal-backdrop" onClick={onCancel}>
      <div className="modal" onClick={(e) => e.stopPropagation()} style={{ maxWidth: 500 }}>
        <div className="modal-header">
          <h3>Configure Template: {template.meta.name}</h3>
          <button className="btn-icon" onClick={onCancel}>
            <X size={16} />
          </button>
        </div>

        <div className="modal-body">
          {loading ? (
            <div className="text-muted">Loading variables…</div>
          ) : (
            <>
              <p className="text-muted" style={{ fontSize: '0.8rem', marginBottom: '1rem' }}>
                Fill in the template variables below. Required fields are marked with *.
              </p>

              <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
                {variables.map((v) => (
                  <div key={v.name}>
                    <label
                      htmlFor={`var-${v.name}`}
                      style={{
                        display: 'block',
                        fontSize: '0.8rem',
                        fontWeight: 600,
                        marginBottom: 4,
                      }}
                    >
                      {v.name}
                      {v.required && <span style={{ color: 'var(--danger)' }}> *</span>}
                    </label>
                    {v.description && (
                      <div className="text-muted" style={{ fontSize: '0.7rem', marginBottom: 4 }}>
                        {v.description}
                      </div>
                    )}
                    {v.name === 'bump_level' ? (
                      <select
                        id={`var-${v.name}`}
                        className="input"
                        value={values[v.name] || 'patch'}
                        onChange={(e) =>
                          setValues((prev) => ({ ...prev, [v.name]: e.target.value }))
                        }
                      >
                        <option value="patch">patch (1.2.3 → 1.2.4)</option>
                        <option value="minor">minor (1.2.3 → 1.3.0)</option>
                        <option value="major">major (1.2.3 → 2.0.0)</option>
                      </select>
                    ) : (
                      <input
                        id={`var-${v.name}`}
                        className="input"
                        type="text"
                        value={values[v.name] || ''}
                        placeholder={v.default_value || v.name}
                        onChange={(e) =>
                          setValues((prev) => ({ ...prev, [v.name]: e.target.value }))
                        }
                      />
                    )}
                  </div>
                ))}
              </div>
            </>
          )}

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
          <button className="btn btn-secondary" onClick={onCancel} disabled={applying}>
            Cancel
          </button>
          <button className="btn btn-primary" onClick={handleApply} disabled={loading || applying}>
            {applying ? 'Applying…' : 'Apply Template'}
          </button>
        </div>
      </div>
    </div>
  );
}

export default TemplateVariableDialog;
