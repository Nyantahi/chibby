import { Rocket, ArrowLeft, ArrowRight } from 'lucide-react';
import type { Dispatch, SetStateAction } from 'react';
import type { DeploymentMethod, DeploymentConfig, ProjectType } from '../../types';
import type { DeployMethodDisplay } from './constants';

interface DeployStepProps {
  projectType: ProjectType;
  detectedDeployMethod: DeploymentMethod;
  availableDeployMethods: DeployMethodDisplay[];
  selectedDeployMethod: DeploymentMethod;
  selectedMethodInfo: DeployMethodDisplay | undefined;
  deployConfig: DeploymentConfig;
  setDeployConfig: Dispatch<SetStateAction<DeploymentConfig>>;
  loading: boolean;
  onSelectDeployMethod: (method: DeploymentMethod) => void;
  onBack: () => void;
  onContinue: () => void;
}

function DeployStep({
  projectType,
  detectedDeployMethod,
  availableDeployMethods,
  selectedDeployMethod,
  selectedMethodInfo,
  deployConfig,
  setDeployConfig,
  loading,
  onSelectDeployMethod,
  onBack,
  onContinue,
}: DeployStepProps) {
  const needsConfig =
    selectedMethodInfo &&
    (selectedMethodInfo.requiresSshHost ||
      selectedMethodInfo.requiresRegistry ||
      selectedMethodInfo.requiresHealthCheck ||
      selectedMethodInfo.requiresPlatformProject);

  return (
    <div className="onboarding-card onboarding-card--wide">
      <div className="onboarding-icon">
        <Rocket size={32} />
      </div>
      <h3>Configure Deployment</h3>
      <p>
        Choose how to deploy your <strong>{projectType}</strong> project, or skip to CI only.
        {detectedDeployMethod !== 'skip' && (
          <span className="text-muted">
            {' '}
            (Detected: <code>{detectedDeployMethod.replace(/_/g, ' ')}</code>)
          </span>
        )}
      </p>

      <div className="wizard-deploy-options">
        {availableDeployMethods.map((info) => (
          <button
            key={info.method}
            className={`wizard-deploy-card${selectedDeployMethod === info.method ? ' selected' : ''}`}
            onClick={() => onSelectDeployMethod(info.method)}
          >
            <span className="deploy-icon">{info.icon}</span>
            <span className="deploy-title">{info.label}</span>
            <span className="deploy-desc">{info.description}</span>
            {detectedDeployMethod === info.method && info.method !== 'skip' && (
              <span className="badge badge-success add-project-detected-badge">Detected</span>
            )}
          </button>
        ))}
      </div>

      {/* Configuration form for methods that need it */}
      {needsConfig && (
        <div className="wizard-deploy-config">
          <h5>Configuration</h5>

          {selectedMethodInfo?.requiresSshHost && (
            <div className="form-group">
              <label htmlFor="ssh-host">SSH Host</label>
              <input
                id="ssh-host"
                type="text"
                className="input"
                placeholder="user@server.example.com"
                value={deployConfig.ssh_host || ''}
                onChange={(e) => setDeployConfig((prev) => ({ ...prev, ssh_host: e.target.value }))}
              />
            </div>
          )}

          {selectedMethodInfo?.requiresRegistry && (
            <div className="form-group">
              <label htmlFor="docker-registry">Docker Registry</label>
              <input
                id="docker-registry"
                type="text"
                className="input"
                placeholder="ghcr.io/username"
                value={deployConfig.docker_registry || ''}
                onChange={(e) =>
                  setDeployConfig((prev) => ({ ...prev, docker_registry: e.target.value }))
                }
              />
            </div>
          )}

          {selectedMethodInfo?.requiresHealthCheck && (
            <div className="form-group">
              <label htmlFor="health-check-url">Health Check URL (optional)</label>
              <input
                id="health-check-url"
                type="text"
                className="input"
                placeholder="/health"
                value={deployConfig.health_check_url || ''}
                onChange={(e) =>
                  setDeployConfig((prev) => ({ ...prev, health_check_url: e.target.value }))
                }
              />
            </div>
          )}

          {selectedMethodInfo?.requiresPlatformProject && (
            <div className="form-group">
              <label htmlFor="platform-project">
                {selectedDeployMethod === 's3_static' ? 'S3 Bucket Name' : 'App/Project Name'}
              </label>
              <input
                id="platform-project"
                type="text"
                className="input"
                placeholder={selectedDeployMethod === 's3_static' ? 'my-bucket' : 'my-app'}
                value={deployConfig.platform_project || ''}
                onChange={(e) =>
                  setDeployConfig((prev) => ({ ...prev, platform_project: e.target.value }))
                }
              />
            </div>
          )}

          {(selectedDeployMethod === 'cargo_publish' || selectedDeployMethod === 'npm_publish') && (
            <div className="form-group">
              <label className="checkbox-label">
                <input
                  type="checkbox"
                  checked={deployConfig.dry_run_first ?? true}
                  onChange={(e) =>
                    setDeployConfig((prev) => ({ ...prev, dry_run_first: e.target.checked }))
                  }
                />
                Run dry-run first (recommended)
              </label>
            </div>
          )}

          {selectedDeployMethod === 'docker_compose_ssh' && (
            <div className="form-group">
              <label htmlFor="compose-file">Compose File (optional)</label>
              <input
                id="compose-file"
                type="text"
                className="input"
                placeholder="docker-compose.prod.yml"
                value={deployConfig.compose_file || ''}
                onChange={(e) =>
                  setDeployConfig((prev) => ({ ...prev, compose_file: e.target.value }))
                }
              />
            </div>
          )}
        </div>
      )}

      <div className="form-actions">
        <button className="btn btn-secondary" onClick={onBack}>
          <ArrowLeft size={16} /> Back
        </button>
        <button className="btn btn-primary" onClick={onContinue} disabled={loading}>
          Continue
          <ArrowRight size={16} />
        </button>
      </div>
    </div>
  );
}

export default DeployStep;
