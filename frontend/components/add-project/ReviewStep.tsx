import { ArrowLeft, ArrowRight } from 'lucide-react';
import type { Stage, DeploymentMethod, DeploymentConfig } from '../../types';
import type { DeployMethodDisplay, PipelineSource } from './constants';

interface ReviewStepProps {
  repoName: string;
  repoPath: string;
  projectType: string;
  pipelineSource: PipelineSource;
  selectedStages: Stage[];
  selectedDeployMethod: DeploymentMethod;
  selectedMethodInfo: DeployMethodDisplay | undefined;
  deployConfig: DeploymentConfig;
  loading: boolean;
  onBack: () => void;
  onCreate: () => void;
}

function ReviewStep({
  repoName,
  repoPath,
  projectType,
  pipelineSource,
  selectedStages,
  selectedDeployMethod,
  selectedMethodInfo,
  deployConfig,
  loading,
  onBack,
  onCreate,
}: ReviewStepProps) {
  return (
    <div className="onboarding-card onboarding-card--wide">
      <h3>Review & create</h3>
      <p>Confirm your project setup before creating.</p>

      <div className="wizard-summary">
        <div className="wizard-summary-row">
          <span className="wizard-summary-label">Project</span>
          <span className="wizard-summary-value">{repoName}</span>
        </div>
        <div className="wizard-summary-row">
          <span className="wizard-summary-label">Path</span>
          <span className="wizard-summary-value">{repoPath}</span>
        </div>
        <div className="wizard-summary-row">
          <span className="wizard-summary-label">Project Type</span>
          <span className="wizard-summary-value">{projectType}</span>
        </div>
        <div className="wizard-summary-row">
          <span className="wizard-summary-label">CI Source</span>
          <span className="wizard-summary-value">
            {pipelineSource === 'github'
              ? 'GitHub Actions'
              : pipelineSource === 'template'
                ? 'Template'
                : 'Auto-detected'}
          </span>
        </div>
        <div className="wizard-summary-row">
          <span className="wizard-summary-label">CI Stages</span>
          <span className="wizard-summary-value">{selectedStages.length}</span>
        </div>
        <div className="wizard-summary-row">
          <span className="wizard-summary-label">Deployment</span>
          <span className="wizard-summary-value">
            {selectedMethodInfo?.label || 'None'}
            {deployConfig.ssh_host && ` (${deployConfig.ssh_host})`}
          </span>
        </div>
      </div>

      <h5 className="add-project-section-heading">CI Pipeline Stages</h5>
      <div className="stage-list">
        {selectedStages.map((stage, idx) => (
          <div key={idx} className="stage-card-mini">
            <span className="stage-number">{idx + 1}</span>
            <div className="stage-info">
              <strong>{stage.name}</strong>
              <code>{stage.commands.join(' && ')}</code>
            </div>
            <span className="badge badge-neutral">{stage.backend}</span>
          </div>
        ))}
      </div>

      {selectedDeployMethod !== 'skip' && (
        <>
          <h5 className="add-project-section-heading">CD Pipeline (deploy.toml)</h5>
          <p className="text-muted text-sm">
            A separate deploy pipeline will be created with{' '}
            {selectedMethodInfo?.label || 'deployment'} stages.
          </p>
        </>
      )}

      <div className="form-actions add-project-review-actions">
        <button className="btn btn-secondary" onClick={onBack}>
          <ArrowLeft size={16} /> Back
        </button>
        <button className="btn btn-primary" onClick={onCreate} disabled={loading}>
          {loading ? 'Creating...' : 'Create Project'}
          <ArrowRight size={16} />
        </button>
      </div>
    </div>
  );
}

export default ReviewStep;
