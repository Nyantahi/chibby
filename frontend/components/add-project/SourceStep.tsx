import { FileCode2, Wand2, GitBranch, BookTemplate, ArrowLeft } from 'lucide-react';
import { FileTypeIcon } from '../FileTypeIcon';
import type { DetectedScript, WorkflowInfo } from '../../types';
import type { PipelineSource } from './constants';

interface SourceStepProps {
  scripts: DetectedScript[];
  workflows: WorkflowInfo[];
  loading: boolean;
  onPickSource: (source: PipelineSource) => void;
  onOpenTemplateBrowser: () => void;
  onBack: () => void;
  onSkip: () => void;
}

function SourceStep({
  scripts,
  workflows,
  loading,
  onPickSource,
  onOpenTemplateBrowser,
  onBack,
  onSkip,
}: SourceStepProps) {
  return (
    <div className="onboarding-card onboarding-card--wide">
      <div className="onboarding-icon">
        <FileCode2 size={32} />
      </div>
      <h3>Choose pipeline source</h3>
      <p>
        Found {scripts.length} build file{scripts.length !== 1 ? 's' : ''}
        {workflows.length > 0 &&
          ` and ${workflows.length} GitHub Actions workflow${workflows.length !== 1 ? 's' : ''}`}
        .
      </p>

      {scripts.length > 0 && (
        <ul className="detected-list">
          {scripts.map((s) => (
            <li key={s.file_path} className="detected-item">
              <FileTypeIcon scriptType={s.script_type} />
              <span className="detected-name">{s.file_name}</span>
              <span className="detected-type">{s.script_type}</span>
            </li>
          ))}
        </ul>
      )}

      <div className="wizard-source-options">
        <button
          className="wizard-source-card"
          onClick={() => onPickSource('auto')}
          disabled={loading}
        >
          <Wand2 size={24} className="source-icon" />
          <span className="source-title">Auto-detect</span>
          <span className="source-desc">Generate a pipeline from your detected build files.</span>
        </button>

        <button
          className="wizard-source-card"
          onClick={() => onPickSource('github')}
          disabled={loading || workflows.length === 0}
        >
          <GitBranch size={24} className="source-icon" />
          <span className="source-title">GitHub Actions</span>
          <span className="source-desc">
            {workflows.length > 0
              ? 'Import stages from your existing CI workflows.'
              : 'No workflows found in .github/workflows/'}
          </span>
        </button>

        <button className="wizard-source-card" onClick={onOpenTemplateBrowser} disabled={loading}>
          <BookTemplate size={24} className="source-icon" />
          <span className="source-title">From Template</span>
          <span className="source-desc">Choose from built-in or custom pipeline templates.</span>
        </button>
      </div>

      <div className="form-actions">
        <button className="btn btn-secondary" onClick={onBack}>
          <ArrowLeft size={16} /> Back
        </button>
        <button className="btn btn-secondary" onClick={onSkip} disabled={loading}>
          Skip Pipeline
        </button>
      </div>
    </div>
  );
}

export default SourceStep;
