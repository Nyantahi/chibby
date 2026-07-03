import { Cog, ExternalLink, FolderOpen, FileCode } from 'lucide-react';
import type {
  ProjectInfo,
  Pipeline,
  PipelineRun,
  EnvironmentsConfig,
  SecretsConfig,
  GitInfo,
  DetectedScript,
  ProjectRecommendations,
} from '../../types';
import { statusClass, capitalize } from '../../utils/format';
import RecommendationsPanel from '../RecommendationsPanel';
import { FileTypeIcon } from '../FileTypeIcon';
import { openPath } from '../../services/openExternal';
import { getAppDataDir } from '../../services/api';
import { formatScriptType } from './helpers';

interface ProjectSidebarProps {
  project: ProjectInfo;
  pipeline: Pipeline | null;
  envsConfig: EnvironmentsConfig;
  secretsConfig: SecretsConfig;
  runs: PipelineRun[];
  gitInfo: GitInfo | null;
  detectedFiles: DetectedScript[];
  recommendations: ProjectRecommendations | null;
  loadingRecommendations: boolean;
}

function ProjectSidebar({
  project,
  pipeline,
  envsConfig,
  secretsConfig,
  runs,
  gitInfo,
  detectedFiles,
  recommendations,
  loadingRecommendations,
}: ProjectSidebarProps) {
  return (
    <aside className="project-sidebar">
      {/* Project Info */}
      <div className="project-sidebar-card">
        <h4 className="project-sidebar-title">
          <Cog size={14} />
          Project Info
        </h4>
        <div className="project-stats">
          <div className="project-stat-row">
            <span className="project-stat-label">Pipeline</span>
            <span className={`badge ${project.has_pipeline ? 'badge-success' : 'badge-pending'}`}>
              {project.has_pipeline ? 'Configured' : 'Not set'}
            </span>
          </div>
          <div className="project-stat-row">
            <span className="project-stat-label">Stages</span>
            <span className="project-stat-value">{pipeline?.stages.length ?? 0}</span>
          </div>
          <div className="project-stat-row">
            <span className="project-stat-label">Environments</span>
            <span className="project-stat-value">{envsConfig?.environments?.length ?? 0}</span>
          </div>
          <div className="project-stat-row">
            <span className="project-stat-label">Secrets</span>
            <span className="project-stat-value">{secretsConfig?.secrets?.length ?? 0}</span>
          </div>
          <div className="project-stat-row">
            <span className="project-stat-label">Total Runs</span>
            <span className="project-stat-value">{runs.length}</span>
          </div>
          {project.project.last_run_status && (
            <div className="project-stat-row">
              <span className="project-stat-label">Last Run</span>
              <span className={`badge badge-${statusClass(project.project.last_run_status)}`}>
                {capitalize(project.project.last_run_status)}
              </span>
            </div>
          )}
        </div>
      </div>

      {/* Quick Links */}
      <div className="project-sidebar-card">
        <h4 className="project-sidebar-title">
          <ExternalLink size={14} />
          Quick Links
        </h4>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
          <button
            className="btn btn-sm btn-ghost"
            onClick={() => openPath(`${project.project.path}/.chibby`)}
          >
            <FolderOpen size={12} /> Reveal .chibby/
          </button>
          <button
            className="btn btn-sm btn-ghost"
            onClick={async () => {
              const dir = await getAppDataDir();
              openPath(dir);
            }}
          >
            <FolderOpen size={12} /> Reveal data dir
          </button>
          {gitInfo?.branch && (
            <button className="btn btn-sm btn-ghost" onClick={() => openPath(project.project.path)}>
              <FolderOpen size={12} /> Open repo folder
            </button>
          )}
        </div>
      </div>

      {/* CI/CD Recommendations */}
      <RecommendationsPanel recommendations={recommendations} loading={loadingRecommendations} />

      {/* Detected CI/CD Files */}
      <div className="project-sidebar-card">
        <h4 className="project-sidebar-title">
          <FileCode size={14} />
          Detected Files
          <span className="badge badge-neutral">{detectedFiles.length}</span>
        </h4>
        {detectedFiles.length === 0 ? (
          <p className="text-muted text-xs">No CI/CD files detected.</p>
        ) : (
          <ul className="detected-files-list">
            {detectedFiles.map((script, i) => (
              <li key={i} className="detected-file-item" title={script.file_path}>
                <span className="detected-file-icon">
                  <FileTypeIcon scriptType={script.script_type} />
                </span>
                <span className="detected-file-name">{script.file_name}</span>
                <span className="detected-file-type">{formatScriptType(script.script_type)}</span>
              </li>
            ))}
          </ul>
        )}
      </div>
    </aside>
  );
}

export default ProjectSidebar;
