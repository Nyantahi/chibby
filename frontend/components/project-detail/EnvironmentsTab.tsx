import { Server, Key, ChevronDown, ChevronRight, Wand2, Download, Upload } from 'lucide-react';
import type { EnvironmentsConfig, SecretsConfig } from '../../types';
import EnvironmentEditor from '../EnvironmentEditor';
import SecretsManager from '../SecretsManager';

interface EnvironmentsTabProps {
  repoPath: string;
  envsConfig: EnvironmentsConfig;
  secretsConfig: SecretsConfig;
  showEnvSection: boolean;
  showSecretsSection: boolean;
  onToggleEnvSection: () => void;
  onToggleSecretsSection: () => void;
  onShowBootstrap: () => void;
  onShowImporter: () => void;
  onShowExportDotenv: () => void;
  onReload: () => void;
}

function EnvironmentsTab({
  repoPath,
  envsConfig,
  secretsConfig,
  showEnvSection,
  showSecretsSection,
  onToggleEnvSection,
  onToggleSecretsSection,
  onShowBootstrap,
  onShowImporter,
  onShowExportDotenv,
  onReload,
}: EnvironmentsTabProps) {
  return (
    <>
      <section className="section">
        <div
          className="section-toggle"
          style={{ justifyContent: 'space-between', cursor: 'default' }}
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <Server size={16} />
            <strong>Bootstrap & Import</strong>
          </div>
          <div style={{ display: 'flex', gap: 8 }}>
            <button className="btn btn-sm btn-secondary" onClick={onShowBootstrap}>
              <Wand2 size={14} />
              Bootstrap
            </button>
            <button className="btn btn-sm btn-secondary" onClick={onShowImporter}>
              <Download size={14} />
              Import…
            </button>
            <button
              className="btn btn-sm btn-ghost"
              onClick={onShowExportDotenv}
              disabled={!envsConfig.environments.length}
            >
              <Upload size={14} />
              Export .env
            </button>
          </div>
        </div>
        <p
          className="text-muted"
          style={{
            fontSize: 'var(--font-size-xs)',
            padding: '0 var(--space-md) var(--space-sm)',
          }}
        >
          Scan the repo for env / secret references, or pull from Vercel / Railway / Fly / a .env
          file.
        </p>
      </section>

      {/* Environments section */}
      <section className="section">
        <button className="section-toggle" onClick={onToggleEnvSection}>
          {showEnvSection ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
          <Server size={16} />
          Environments
          <span className="badge badge-neutral">{envsConfig?.environments?.length}</span>
        </button>
        {showEnvSection && (
          <EnvironmentEditor repoPath={repoPath} config={envsConfig} onSaved={onReload} />
        )}
      </section>

      {/* Secrets section */}
      <section className="section">
        <button className="section-toggle" onClick={onToggleSecretsSection}>
          {showSecretsSection ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
          <Key size={16} />
          Secrets
          <span className="badge badge-neutral">{secretsConfig?.secrets?.length}</span>
        </button>
        {showSecretsSection && (
          <SecretsManager
            repoPath={repoPath}
            config={secretsConfig}
            environments={envsConfig.environments}
            onSaved={onReload}
          />
        )}
      </section>
    </>
  );
}

export default EnvironmentsTab;
