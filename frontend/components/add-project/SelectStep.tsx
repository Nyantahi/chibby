import { FolderOpen, Search, BookTemplate, X } from 'lucide-react';
import type { PipelineTemplate } from '../../types';

interface SelectStepProps {
  pendingTemplate: PipelineTemplate | null;
  repoPath: string;
  repoName: string;
  loading: boolean;
  onRepoPathChange: (value: string) => void;
  onRepoNameChange: (value: string) => void;
  onBrowse: () => void;
  onScan: () => void;
  onRemoveTemplate: () => void;
}

function SelectStep({
  pendingTemplate,
  repoPath,
  repoName,
  loading,
  onRepoPathChange,
  onRepoNameChange,
  onBrowse,
  onScan,
  onRemoveTemplate,
}: SelectStepProps) {
  return (
    <div className="onboarding-card">
      <div className="onboarding-icon">
        <FolderOpen size={32} />
      </div>
      <h3>Select a repository</h3>
      <p>Enter the local path to your project repository.</p>

      {pendingTemplate && (
        <div className="alert alert-success add-project-template-banner">
          <BookTemplate size={14} />
          Template selected: <strong>{pendingTemplate.meta.name}</strong>
          <button
            className="btn btn-icon add-project-template-remove"
            onClick={onRemoveTemplate}
            title="Remove template selection"
          >
            <X size={14} />
          </button>
        </div>
      )}

      <div className="form-group">
        <label htmlFor="repo-path">Repository Path</label>
        <div className="input-with-action">
          <input
            id="repo-path"
            type="text"
            className="input"
            placeholder="/Users/you/projects/my-app"
            value={repoPath}
            onChange={(e) => onRepoPathChange(e.target.value)}
          />
          <button type="button" className="btn btn-secondary btn-browse" onClick={onBrowse}>
            Browse
          </button>
        </div>
      </div>

      <div className="form-group">
        <label htmlFor="repo-name">Project Name (optional)</label>
        <input
          id="repo-name"
          type="text"
          className="input"
          placeholder="Auto-detected from path"
          value={repoName}
          onChange={(e) => onRepoNameChange(e.target.value)}
        />
      </div>

      <div className="form-actions">
        <button className="btn btn-primary" onClick={onScan} disabled={loading}>
          {loading ? 'Scanning...' : pendingTemplate ? 'Scan & Apply Template' : 'Scan Repository'}
          <Search size={16} />
        </button>
      </div>
    </div>
  );
}

export default SelectStep;
