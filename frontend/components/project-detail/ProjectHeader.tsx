import { Play, GitBranch, Trash2, Shield, Loader2, Square } from 'lucide-react';
import type { ProjectInfo, EnvironmentsConfig, GitInfo } from '../../types';

interface ProjectHeaderProps {
  project: ProjectInfo;
  gitInfo: GitInfo | null;
  pipelineNames: string[];
  selectedPipeline: string;
  onSwitchPipeline: (name: string) => void;
  envsConfig: EnvironmentsConfig;
  selectedEnv: string;
  onSelectedEnvChange: (value: string) => void;
  running: boolean;
  preflighting: boolean;
  onPreflight: () => void;
  onRun: () => void;
  onCancel: () => void;
  onDelete: () => void;
}

function ProjectHeader({
  project,
  gitInfo,
  pipelineNames,
  selectedPipeline,
  onSwitchPipeline,
  envsConfig,
  selectedEnv,
  onSelectedEnvChange,
  running,
  preflighting,
  onPreflight,
  onRun,
  onCancel,
  onDelete,
}: ProjectHeaderProps) {
  return (
    <header className="page-header">
      <div>
        <h2 className="page-title">{project.project.name}</h2>
        <p className="page-subtitle">{project.project.path}</p>
        {gitInfo?.branch && (
          <div className="git-info">
            <GitBranch size={14} />
            <span className="git-branch">{gitInfo.branch}</span>
            {gitInfo.commit && <span className="git-commit">{gitInfo.commit}</span>}
            {gitInfo.is_dirty && (
              <span className="git-dirty" title="Uncommitted changes">
                modified
              </span>
            )}
            {gitInfo.ahead !== undefined && gitInfo.ahead > 0 && (
              <span className="git-ahead" title={`${gitInfo.ahead} commits ahead of remote`}>
                +{gitInfo.ahead}
              </span>
            )}
            {gitInfo.behind !== undefined && gitInfo.behind > 0 && (
              <span className="git-behind" title={`${gitInfo.behind} commits behind remote`}>
                -{gitInfo.behind}
              </span>
            )}
          </div>
        )}
      </div>
      <div className="header-actions">
        {/* Pipeline selector - always visible for consistent UI */}
        <select
          className="input input-sm env-select"
          value={selectedPipeline}
          onChange={(e) => onSwitchPipeline(e.target.value)}
          disabled={running || pipelineNames.length <= 1}
        >
          {pipelineNames.length === 0 ? (
            <option value="pipeline">Pipeline</option>
          ) : (
            pipelineNames.map((name) => (
              <option key={name} value={name}>
                {name === 'pipeline' ? 'CI' : name.charAt(0).toUpperCase() + name.slice(1)}
              </option>
            ))
          )}
        </select>

        {/* Deploy target selector - always visible for consistent UI */}
        <select
          className="input input-sm env-select"
          value={selectedEnv}
          onChange={(e) => onSelectedEnvChange(e.target.value)}
          disabled={running || !envsConfig?.environments?.length}
        >
          <option value="">No target</option>
          {envsConfig?.environments?.map((env) => (
            <option key={env.name} value={env.name}>
              {env.name}
            </option>
          ))}
        </select>

        {/* Preflight button */}
        {selectedEnv && (
          <button className="btn btn-secondary" onClick={onPreflight} disabled={preflighting}>
            <Shield size={16} />
            {preflighting ? 'Checking...' : 'Preflight'}
          </button>
        )}

        <button
          className="btn btn-primary"
          onClick={onRun}
          disabled={running || !project.has_pipeline}
        >
          {running ? <Loader2 size={16} className="spin" /> : <Play size={16} />}
          {running ? 'Running...' : 'Run Pipeline'}
        </button>
        {running && (
          <button className="btn btn-danger" onClick={onCancel} title="Stop pipeline">
            <Square size={16} />
            Stop
          </button>
        )}
        <button className="btn btn-danger-outline" onClick={onDelete}>
          <Trash2 size={16} />
        </button>
      </div>
    </header>
  );
}

export default ProjectHeader;
