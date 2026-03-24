import { useState, useEffect, useMemo } from 'react';
import { Search, Layout, Layers, Package, ChevronDown, ChevronRight } from 'lucide-react';
import { getTemplates } from '../services/api';
import type { PipelineTemplate, TemplateType, TemplateSource } from '../types';

interface Props {
  /** Optional repo path to include project-scoped templates. */
  repoPath?: string;
  /** Filter to only one template type (pipeline or stage). */
  filterType?: TemplateType;
  /** Called when the user wants to apply a template. */
  onApply?: (template: PipelineTemplate) => void;
  /** Called when the user wants to use a template as a starting point for editing. */
  onEdit?: (template: PipelineTemplate) => void;
}

const SOURCE_LABELS: Record<TemplateSource, string> = {
  built_in: 'Built-in',
  user: 'User',
  project: 'Project',
};

const SOURCE_BADGE_CLASS: Record<TemplateSource, string> = {
  built_in: 'badge-info',
  user: 'badge-success',
  project: 'badge-warning',
};

const CATEGORY_OPTIONS = [
  { value: '', label: 'All Categories' },
  { value: 'rust', label: 'Rust' },
  { value: 'node', label: 'Node.js' },
  { value: 'python', label: 'Python' },
  { value: 'go', label: 'Go' },
  { value: 'docker', label: 'Docker' },
  { value: 'deployment', label: 'Deployment' },
];

function TemplateBrowser({ repoPath, filterType, onApply, onEdit }: Props) {
  const [templates, setTemplates] = useState<PipelineTemplate[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Filters
  const [search, setSearch] = useState('');
  const [category, setCategory] = useState('');
  const [sourceFilter, setSourceFilter] = useState<TemplateSource | ''>('');
  const [typeFilter, setTypeFilter] = useState<TemplateType | ''>(filterType || '');

  // Expanded template detail
  const [expandedTemplate, setExpandedTemplate] = useState<string | null>(null);

  useEffect(() => {
    loadTemplates();
  }, [repoPath]);

  async function loadTemplates() {
    try {
      setLoading(true);
      setError(null);
      const result = await getTemplates(repoPath);
      setTemplates(result);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  const filtered = useMemo(() => {
    return templates.filter((t) => {
      // Type filter (from prop or user selection)
      if (filterType && t.meta.template_type !== filterType) return false;
      if (typeFilter && t.meta.template_type !== typeFilter) return false;

      // Category
      if (category && t.meta.category !== category) return false;

      // Source
      if (sourceFilter && t.source !== sourceFilter) return false;

      // Free-text search
      if (search) {
        const q = search.toLowerCase();
        const haystack = [
          t.meta.name,
          t.meta.description,
          t.meta.category,
          ...t.meta.tags,
        ]
          .join(' ')
          .toLowerCase();
        if (!haystack.includes(q)) return false;
      }

      return true;
    });
  }, [templates, search, category, sourceFilter, typeFilter, filterType]);

  const stageCount = (t: PipelineTemplate) => {
    if (t.pipeline) return t.pipeline.stages.length;
    if (t.stages) return t.stages.length;
    return 0;
  };

  if (loading) {
    return <div className="text-muted" style={{ padding: '1rem' }}>Loading templates…</div>;
  }

  if (error) {
    return <div className="text-danger" style={{ padding: '1rem' }}>Error: {error}</div>;
  }

  return (
    <div className="template-browser">
      {/* Filters */}
      <div className="template-filters" style={{ display: 'flex', gap: '0.5rem', marginBottom: '1rem', flexWrap: 'wrap' }}>
        <div className="input-with-action" style={{ flex: '1 1 200px' }}>
          <Search size={14} style={{ position: 'absolute', left: 8, top: '50%', transform: 'translateY(-50%)', color: 'var(--text-muted)' }} />
          <input
            className="input"
            type="text"
            placeholder="Search templates…"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            style={{ paddingLeft: 28 }}
          />
        </div>

        <select
          className="input"
          value={category}
          onChange={(e) => setCategory(e.target.value)}
          style={{ width: 'auto', minWidth: 140 }}
        >
          {CATEGORY_OPTIONS.map((o) => (
            <option key={o.value} value={o.value}>{o.label}</option>
          ))}
        </select>

        {!filterType && (
          <select
            className="input"
            value={typeFilter}
            onChange={(e) => setTypeFilter(e.target.value as TemplateType | '')}
            style={{ width: 'auto', minWidth: 130 }}
          >
            <option value="">All Types</option>
            <option value="pipeline">Full Pipelines</option>
            <option value="stage">Stage Snippets</option>
          </select>
        )}

        <select
          className="input"
          value={sourceFilter}
          onChange={(e) => setSourceFilter(e.target.value as TemplateSource | '')}
          style={{ width: 'auto', minWidth: 120 }}
        >
          <option value="">All Sources</option>
          <option value="built_in">Built-in</option>
          <option value="user">User</option>
          <option value="project">Project</option>
        </select>
      </div>

      {/* Count */}
      <div className="text-muted" style={{ fontSize: '0.8rem', marginBottom: '0.75rem' }}>
        {filtered.length} template{filtered.length !== 1 ? 's' : ''}
        {search || category || sourceFilter || typeFilter ? ' (filtered)' : ''}
      </div>

      {/* Template list */}
      {filtered.length === 0 ? (
        <div className="text-muted" style={{ padding: '2rem', textAlign: 'center' }}>
          {templates.length === 0
            ? 'No templates available.'
            : 'No templates match your filters.'}
        </div>
      ) : (
        <div className="template-grid">
          {filtered.map((t) => {
            const isExpanded = expandedTemplate === t.meta.name;
            const stages = t.pipeline?.stages || t.stages || [];

            return (
              <div
                key={`${t.source}-${t.meta.name}`}
                className="template-card"
              >
                {/* Header row */}
                <div
                  style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', cursor: 'pointer' }}
                  onClick={() => setExpandedTemplate(isExpanded ? null : t.meta.name)}
                >
                  {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}

                  {t.meta.template_type === 'pipeline' ? (
                    <Layout size={14} style={{ color: 'var(--accent)' }} />
                  ) : (
                    <Layers size={14} style={{ color: 'var(--text-muted)' }} />
                  )}

                  <strong style={{ flex: 1 }}>{t.meta.name}</strong>

                  <span className={`badge ${SOURCE_BADGE_CLASS[t.source]}`} style={{ fontSize: '0.7rem' }}>
                    {SOURCE_LABELS[t.source]}
                  </span>

                  <span className="badge badge-neutral" style={{ fontSize: '0.7rem' }}>
                    {stageCount(t)} stage{stageCount(t) !== 1 ? 's' : ''}
                  </span>
                </div>

                {/* Description */}
                <div className="text-muted" style={{ fontSize: '0.8rem', marginTop: '0.25rem', marginLeft: '2rem' }}>
                  {t.meta.description}
                </div>

                {/* Tags */}
                {t.meta.tags.length > 0 && (
                  <div style={{ display: 'flex', gap: '0.25rem', flexWrap: 'wrap', marginTop: '0.35rem', marginLeft: '2rem' }}>
                    {t.meta.tags.map((tag) => (
                      <span
                        key={tag}
                        style={{
                          fontSize: '0.65rem',
                          padding: '1px 6px',
                          borderRadius: 4,
                          background: 'var(--surface-raised, var(--border))',
                          color: 'var(--text-muted)',
                        }}
                      >
                        {tag}
                      </span>
                    ))}
                  </div>
                )}

                {/* Expanded detail */}
                {isExpanded && (
                  <div style={{ marginTop: '0.75rem', marginLeft: '2rem' }}>
                    {/* Stages preview */}
                    <div style={{ fontSize: '0.8rem', marginBottom: '0.5rem' }}>
                      <strong>Stages:</strong>
                    </div>
                    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem', marginBottom: '0.75rem' }}>
                      {stages.map((s, i) => (
                        <div
                          key={i}
                          style={{
                            fontSize: '0.75rem',
                            padding: '0.25rem 0.5rem',
                            background: 'var(--bg)',
                            borderRadius: 4,
                            fontFamily: 'var(--font-mono, monospace)',
                          }}
                        >
                          <span style={{ color: 'var(--accent)' }}>{s.name}</span>
                          <span className="text-muted"> — {s.commands.join(' && ')}</span>
                          {s.backend === 'ssh' && (
                            <span className="badge badge-warning" style={{ fontSize: '0.6rem', marginLeft: '0.5rem' }}>SSH</span>
                          )}
                        </div>
                      ))}
                    </div>

                    {/* Meta info */}
                    {t.meta.required_tools.length > 0 && (
                      <div className="text-muted" style={{ fontSize: '0.75rem', marginBottom: '0.5rem' }}>
                        <Package size={12} style={{ verticalAlign: 'middle', marginRight: 4 }} />
                        Requires: {t.meta.required_tools.join(', ')}
                      </div>
                    )}

                    {/* Actions */}
                    <div style={{ display: 'flex', gap: '0.5rem', marginTop: '0.5rem' }}>
                      {onApply && (
                        <button className="btn btn-primary" onClick={() => onApply(t)}>
                          Apply Template
                        </button>
                      )}
                      {onEdit && (
                        <button className="btn btn-secondary" onClick={() => onEdit(t)}>
                          Use as Starting Point
                        </button>
                      )}
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

export default TemplateBrowser;
