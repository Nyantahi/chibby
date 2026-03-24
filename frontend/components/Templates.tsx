import { useState, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { Upload, Download, X, Layers, Layout } from 'lucide-react';
import {
  deleteCustomTemplate,
  exportTemplate,
  importTemplate,
} from '../services/api';
import type { PipelineTemplate } from '../types';
import TemplateBrowser from './TemplateBrowser';

type Tab = 'browse' | 'my-templates';

function Templates() {
  const navigate = useNavigate();
  const [activeTab, setActiveTab] = useState<Tab>('browse');
  const [error, setError] = useState<string | null>(null);
  const [successMsg, setSuccessMsg] = useState<string | null>(null);

  // Import modal
  const [showImport, setShowImport] = useState(false);
  const [importText, setImportText] = useState('');
  const [importScope, setImportScope] = useState<'user' | 'project'>('user');
  const [importing, setImporting] = useState(false);

  // Export modal
  const [exportedToml, setExportedToml] = useState<string | null>(null);

  // Deletion confirmation
  const [deleteTarget, setDeleteTarget] = useState<{
    name: string;
    scope: 'user' | 'project';
  } | null>(null);
  const [deleting, setDeleting] = useState(false);

  // Force refresh key for TemplateBrowser
  const [refreshKey, setRefreshKey] = useState(0);

  const flash = useCallback((msg: string) => {
    setSuccessMsg(msg);
    setTimeout(() => setSuccessMsg(null), 3000);
  }, []);

  async function handleExport(template: PipelineTemplate) {
    try {
      setError(null);
      const toml = await exportTemplate(template.meta.name);
      setExportedToml(toml);
    } catch (err) {
      setError(String(err));
    }
  }

  async function handleImport() {
    if (!importText.trim()) return;
    try {
      setImporting(true);
      setError(null);
      await importTemplate(importText, importScope);
      setShowImport(false);
      setImportText('');
      setRefreshKey((k) => k + 1);
      flash('Template imported successfully');
    } catch (err) {
      setError(String(err));
    } finally {
      setImporting(false);
    }
  }

  async function handleDelete() {
    if (!deleteTarget) return;
    try {
      setDeleting(true);
      setError(null);
      await deleteCustomTemplate(deleteTarget.name, deleteTarget.scope);
      setDeleteTarget(null);
      setRefreshKey((k) => k + 1);
      flash('Template deleted');
    } catch (err) {
      setError(String(err));
    } finally {
      setDeleting(false);
    }
  }

  function handleApplyTemplate(template: PipelineTemplate) {
    navigate('/add-project', { state: { template, editMode: false } });
  }

  function handleEditTemplate(template: PipelineTemplate) {
    navigate('/add-project', { state: { template, editMode: true } });
  }

  return (
    <div className="page-content">
      <div className="page-header">
        <h2>Pipeline Templates</h2>
        <div className="page-header-actions">
          <button
            className="btn btn-secondary btn-sm"
            onClick={() => setShowImport(true)}
          >
            <Upload size={14} /> Import
          </button>
        </div>
      </div>

      {error && <div className="alert alert-error">{error}</div>}
      {successMsg && <div className="alert alert-success">{successMsg}</div>}

      {/* Tabs */}
      <div className="tabs">
        <button
          className={`tab ${activeTab === 'browse' ? 'tab-active' : ''}`}
          onClick={() => setActiveTab('browse')}
        >
          <Layout size={14} /> All Templates
        </button>
        <button
          className={`tab ${activeTab === 'my-templates' ? 'tab-active' : ''}`}
          onClick={() => setActiveTab('my-templates')}
        >
          <Layers size={14} /> My Templates
        </button>
      </div>

      {/* Tab content */}
      {activeTab === 'browse' && (
        <TemplateBrowser
          key={`browse-${refreshKey}`}
          onApply={handleApplyTemplate}
          onEdit={handleEditTemplate}
        />
      )}

      {activeTab === 'my-templates' && (
        <MyTemplatesList
          refreshKey={refreshKey}
          onExport={handleExport}
          onDelete={(name, scope) => setDeleteTarget({ name, scope })}
        />
      )}

      {/* Import modal */}
      {showImport && (
        <div className="modal-backdrop" onClick={() => setShowImport(false)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>
                <Upload size={16} /> Import Template
              </h3>
              <button className="btn btn-icon" onClick={() => setShowImport(false)}>
                <X size={14} />
              </button>
            </div>
            <div className="modal-body">
              <div className="form-group">
                <label>Template TOML</label>
                <textarea
                  className="input"
                  rows={12}
                  value={importText}
                  onChange={(e) => setImportText(e.target.value)}
                  placeholder="Paste template TOML content here..."
                  style={{ fontFamily: 'monospace', fontSize: '0.85rem' }}
                />
              </div>
              <div className="form-group">
                <label>Save To</label>
                <select
                  className="input"
                  value={importScope}
                  onChange={(e) => setImportScope(e.target.value as 'user' | 'project')}
                >
                  <option value="user">User Templates (global)</option>
                  <option value="project">Project Templates</option>
                </select>
              </div>
            </div>
            <div className="modal-footer">
              <button className="btn btn-secondary" onClick={() => setShowImport(false)}>
                Cancel
              </button>
              <button
                className="btn btn-primary"
                onClick={handleImport}
                disabled={!importText.trim() || importing}
              >
                {importing ? 'Importing...' : 'Import'}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Export / view modal */}
      {exportedToml && (
        <div className="modal-backdrop" onClick={() => setExportedToml(null)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>
                <Download size={16} /> Template TOML
              </h3>
              <button className="btn btn-icon" onClick={() => setExportedToml(null)}>
                <X size={14} />
              </button>
            </div>
            <div className="modal-body">
              <textarea
                className="input"
                rows={16}
                readOnly
                value={exportedToml}
                style={{ fontFamily: 'monospace', fontSize: '0.85rem' }}
              />
            </div>
            <div className="modal-footer">
              <button
                className="btn btn-secondary"
                onClick={() => {
                  navigator.clipboard.writeText(exportedToml);
                  flash('Copied to clipboard');
                }}
              >
                Copy
              </button>
              <button className="btn btn-primary" onClick={() => setExportedToml(null)}>
                Close
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Delete confirmation */}
      {deleteTarget && (
        <div className="modal-backdrop" onClick={() => setDeleteTarget(null)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>Delete Template</h3>
              <button className="btn btn-icon" onClick={() => setDeleteTarget(null)}>
                <X size={14} />
              </button>
            </div>
            <div className="modal-body">
              <p>
                Delete <strong>{deleteTarget.name}</strong> from{' '}
                {deleteTarget.scope === 'user' ? 'user' : 'project'} templates?
              </p>
            </div>
            <div className="modal-footer">
              <button className="btn btn-secondary" onClick={() => setDeleteTarget(null)}>
                Cancel
              </button>
              <button
                className="btn btn-danger"
                onClick={handleDelete}
                disabled={deleting}
              >
                {deleting ? 'Deleting...' : 'Delete'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

/** Sub-component that shows only user + project templates with delete actions. */
function MyTemplatesList({
  refreshKey,
  onExport,
  onDelete,
}: {
  refreshKey: number;
  onExport: (t: PipelineTemplate) => void;
  onDelete: (name: string, scope: 'user' | 'project') => void;
}) {
  return (
    <TemplateBrowser
      key={`my-${refreshKey}`}
      onApply={onExport}
      onEdit={(template) => {
        if (template.source !== 'built_in') {
          onDelete(
            template.meta.name,
            template.source as 'user' | 'project',
          );
        }
      }}
    />
  );
}

export default Templates;
