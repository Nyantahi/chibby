import { useState, useEffect } from 'react';
import { useNavigate, useLocation } from 'react-router-dom';
import {
  FolderOpen,
  Search,
  FileCode2,
  ArrowRight,
  ArrowLeft,
  Check,
  Wand2,
  GitBranch,
  Plus,
  BookTemplate,
  X,
  Rocket,
  Package,
  Container,
  Cloud,
  Tag,
  SkipForward,
  Server,
  Plane,
} from 'lucide-react';
import { open } from '@tauri-apps/plugin-dialog';
import {
  addProject,
  detectScripts,
  generatePipelineWithDeploy,
  savePipeline,
  getGithubWorkflows,
  workflowsToPipelineStages,
  detectDeploymentMethod,
  getSuggestedDeployMethods,
  detectProjectType,
} from '../services/api';
import { repoNameFromPath } from '../utils/format';
import { FileTypeIcon } from './FileTypeIcon';
import TemplateBrowser from './TemplateBrowser';
import TemplateVariableDialog from './TemplateVariableDialog';
import type {
  DetectedScript,
  Pipeline,
  PipelineTemplate,
  Stage,
  WorkflowInfo,
  DeploymentMethod,
  DeploymentConfig,
  ProjectType,
} from '../types';

type WizardStep = 'select' | 'source' | 'configure' | 'deploy' | 'review' | 'done';
type PipelineSource = 'auto' | 'github' | 'template';

const WIZARD_STEPS: { key: WizardStep; label: string }[] = [
  { key: 'select', label: 'Select' },
  { key: 'source', label: 'Source' },
  { key: 'configure', label: 'CI Stages' },
  { key: 'deploy', label: 'Deploy' },
  { key: 'review', label: 'Review' },
];

// Deployment method display information
interface DeployMethodDisplay {
  method: DeploymentMethod;
  label: string;
  description: string;
  icon: React.ReactNode;
  requiresSshHost: boolean;
  requiresRegistry: boolean;
  requiresHealthCheck: boolean;
  requiresPlatformProject: boolean;
}

const DEPLOY_METHOD_INFO: DeployMethodDisplay[] = [
  {
    method: 'auto_detect',
    label: 'Auto-detect',
    description: 'Use GitHub Actions deploy workflow',
    icon: <Wand2 size={20} />,
    requiresSshHost: false,
    requiresRegistry: false,
    requiresHealthCheck: false,
    requiresPlatformProject: false,
  },
  {
    method: 'docker_compose_ssh',
    label: 'Docker Compose SSH',
    description: 'Deploy with docker compose over SSH',
    icon: <Container size={20} />,
    requiresSshHost: true,
    requiresRegistry: false,
    requiresHealthCheck: true,
    requiresPlatformProject: false,
  },
  {
    method: 'docker_registry',
    label: 'Docker Registry',
    description: 'Push to registry, pull on server',
    icon: <Container size={20} />,
    requiresSshHost: true,
    requiresRegistry: true,
    requiresHealthCheck: false,
    requiresPlatformProject: false,
  },
  {
    method: 'cargo_publish',
    label: 'Cargo Publish',
    description: 'Publish to crates.io',
    icon: <Package size={20} />,
    requiresSshHost: false,
    requiresRegistry: false,
    requiresHealthCheck: false,
    requiresPlatformProject: false,
  },
  {
    method: 'npm_publish',
    label: 'npm Publish',
    description: 'Publish to npm registry',
    icon: <Package size={20} />,
    requiresSshHost: false,
    requiresRegistry: false,
    requiresHealthCheck: false,
    requiresPlatformProject: false,
  },
  {
    method: 'github_release',
    label: 'GitHub Release',
    description: 'Create release with binaries',
    icon: <Tag size={20} />,
    requiresSshHost: false,
    requiresRegistry: false,
    requiresHealthCheck: false,
    requiresPlatformProject: false,
  },
  {
    method: 'ssh_rsync',
    label: 'SSH + rsync',
    description: 'Sync files to server via rsync',
    icon: <Server size={20} />,
    requiresSshHost: true,
    requiresRegistry: false,
    requiresHealthCheck: false,
    requiresPlatformProject: false,
  },
  {
    method: 'flyio',
    label: 'Fly.io',
    description: 'Deploy to Fly.io',
    icon: <Plane size={20} />,
    requiresSshHost: false,
    requiresRegistry: false,
    requiresHealthCheck: true,
    requiresPlatformProject: true,
  },
  {
    method: 'render',
    label: 'Render',
    description: 'Deploy to Render',
    icon: <Cloud size={20} />,
    requiresSshHost: false,
    requiresRegistry: false,
    requiresHealthCheck: false,
    requiresPlatformProject: false,
  },
  {
    method: 'railway',
    label: 'Railway',
    description: 'Deploy to Railway',
    icon: <Cloud size={20} />,
    requiresSshHost: false,
    requiresRegistry: false,
    requiresHealthCheck: false,
    requiresPlatformProject: false,
  },
  {
    method: 'netlify',
    label: 'Netlify',
    description: 'Deploy to Netlify',
    icon: <Cloud size={20} />,
    requiresSshHost: false,
    requiresRegistry: false,
    requiresHealthCheck: false,
    requiresPlatformProject: false,
  },
  {
    method: 'vercel',
    label: 'Vercel',
    description: 'Deploy to Vercel',
    icon: <Cloud size={20} />,
    requiresSshHost: false,
    requiresRegistry: false,
    requiresHealthCheck: false,
    requiresPlatformProject: false,
  },
  {
    method: 's3_static',
    label: 'S3 Static',
    description: 'Deploy to AWS S3',
    icon: <Cloud size={20} />,
    requiresSshHost: false,
    requiresRegistry: false,
    requiresHealthCheck: false,
    requiresPlatformProject: true,
  },
  {
    method: 'skip',
    label: 'Skip',
    description: 'CI only, no deployment',
    icon: <SkipForward size={20} />,
    requiresSshHost: false,
    requiresRegistry: false,
    requiresHealthCheck: false,
    requiresPlatformProject: false,
  },
];

// Missing-stage suggestion rules: detected file -> keyword to look for in commands -> suggested stage
const SUGGESTION_RULES: {
  filePattern: string;
  keywords: string[];
  stages: { name: string; commands: string[] }[];
}[] = [
  {
    filePattern: 'Makefile',
    keywords: ['make'],
    stages: [
      { name: 'make-build', commands: ['make build'] },
      { name: 'make-test', commands: ['make test'] },
    ],
  },
  {
    filePattern: 'deploy.sh',
    keywords: ['deploy.sh'],
    stages: [{ name: 'deploy', commands: ['./deploy.sh'] }],
  },
  {
    filePattern: 'Dockerfile',
    keywords: ['docker'],
    stages: [{ name: 'docker-build', commands: ['docker build .'] }],
  },
  {
    filePattern: 'docker-compose',
    keywords: ['docker compose', 'docker-compose'],
    stages: [{ name: 'docker-compose', commands: ['docker compose up -d'] }],
  },
  {
    filePattern: 'Cargo.toml',
    keywords: ['cargo'],
    stages: [
      { name: 'cargo-build', commands: ['cargo build'] },
      { name: 'cargo-test', commands: ['cargo test'] },
    ],
  },
  {
    filePattern: 'go.mod',
    keywords: ['go build', 'go test'],
    stages: [
      { name: 'go-build', commands: ['go build ./...'] },
      { name: 'go-test', commands: ['go test ./...'] },
    ],
  },
  {
    filePattern: 'pyproject.toml',
    keywords: ['pip', 'pytest', 'python'],
    stages: [{ name: 'python-test', commands: ['pip install -e .', 'pytest'] }],
  },
  {
    filePattern: 'setup.py',
    keywords: ['pip', 'pytest', 'python'],
    stages: [{ name: 'python-test', commands: ['pip install -e .', 'pytest'] }],
  },
];

function computeSuggestions(
  scripts: DetectedScript[],
  stages: Stage[]
): { name: string; commands: string[]; reason: string }[] {
  const allCommands = stages
    .flatMap((s) => s.commands)
    .join(' ')
    .toLowerCase();

  const suggestions: { name: string; commands: string[]; reason: string }[] = [];

  for (const rule of SUGGESTION_RULES) {
    const hasFile = scripts.some((s) =>
      s.file_name.toLowerCase().includes(rule.filePattern.toLowerCase())
    );
    if (!hasFile) continue;

    const covered = rule.keywords.some((kw) => allCommands.includes(kw.toLowerCase()));
    if (covered) continue;

    for (const stage of rule.stages) {
      suggestions.push({
        ...stage,
        reason: `${rule.filePattern} detected`,
      });
    }
  }

  return suggestions;
}

function AddProject() {
  const navigate = useNavigate();
  const location = useLocation();

  const [step, setStep] = useState<WizardStep>('select');
  const [repoPath, setRepoPath] = useState('');
  const [repoName, setRepoName] = useState('');
  const [scripts, setScripts] = useState<DetectedScript[]>([]);
  const [autoDraft, setAutoDraft] = useState<Pipeline | null>(null);
  const [draft, setDraft] = useState<Pipeline | null>(null);
  const [workflows, setWorkflows] = useState<WorkflowInfo[]>([]);
  const [pipelineSource, setPipelineSource] = useState<PipelineSource>('auto');
  const [stageSelection, setStageSelection] = useState<Record<number, boolean>>({});
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Template state
  const [showTemplateBrowser, setShowTemplateBrowser] = useState(false);
  const [selectedTemplate, setSelectedTemplate] = useState<PipelineTemplate | null>(null);
  // Template pre-selected from the Templates page — waits for repo selection before applying
  const [pendingTemplate, setPendingTemplate] = useState<PipelineTemplate | null>(null);

  // Deployment state
  const [projectType, setProjectType] = useState<ProjectType>('Unknown');
  const [detectedDeployMethod, setDetectedDeployMethod] = useState<DeploymentMethod>('skip');
  const [suggestedDeployMethods, setSuggestedDeployMethods] = useState<DeploymentMethod[]>([]);
  const [selectedDeployMethod, setSelectedDeployMethod] = useState<DeploymentMethod>('skip');
  const [deployConfig, setDeployConfig] = useState<DeploymentConfig>({
    method: 'skip',
    dry_run_first: true,
  });

  // Pick up template passed from the Templates page via router state
  useEffect(() => {
    const state = location.state as { template?: PipelineTemplate; editMode?: boolean } | null;
    if (state?.template) {
      setPendingTemplate(state.template);
      setPipelineSource('template');
      // Clear state so refreshing doesn't re-trigger
      window.history.replaceState({}, '');
    }
  }, [location.state]);

  const currentStepIdx = WIZARD_STEPS.findIndex((s) => s.key === step);

  // Scan repo: detect scripts, generate auto pipeline, check for workflows, detect deployment
  async function handleScan() {
    if (!repoPath.trim()) {
      setError('Please enter a repository path.');
      return;
    }
    try {
      setLoading(true);
      setError(null);
      const name = repoName.trim() || repoNameFromPath(repoPath);
      setRepoName(name);

      const [detected, pipeline, wfs, detectedDeploy, suggestedDeploys, projType] =
        await Promise.all([
          detectScripts(repoPath),
          generatePipelineWithDeploy(repoPath, name, undefined), // Generate without deploy for now
          getGithubWorkflows(repoPath).catch(() => [] as WorkflowInfo[]),
          detectDeploymentMethod(repoPath).catch(() => 'skip' as DeploymentMethod),
          getSuggestedDeployMethods(repoPath).catch(() => ['skip'] as DeploymentMethod[]),
          detectProjectType(repoPath).catch(() => 'Unknown' as ProjectType),
        ]);

      setScripts(detected);
      setAutoDraft(pipeline);
      setDraft(pipeline);
      setWorkflows(wfs);
      setProjectType(projType);
      setDetectedDeployMethod(detectedDeploy);
      setSuggestedDeployMethods(suggestedDeploys);
      setSelectedDeployMethod(detectedDeploy);
      setDeployConfig({
        method: detectedDeploy,
        dry_run_first: true,
      });

      // Initialize stage selection from auto draft
      const sel: Record<number, boolean> = {};
      pipeline.stages.forEach((_, i) => {
        sel[i] = true;
      });
      setStageSelection(sel);

      // If a template was pre-selected from the Templates page, skip Source
      // and open the variable dialog directly
      if (pendingTemplate) {
        setSelectedTemplate(pendingTemplate);
        setPendingTemplate(null);
      } else {
        setStep('source');
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  // Pick pipeline source and move to configure
  async function handlePickSource(source: PipelineSource) {
    setPipelineSource(source);

    if (source === 'auto' && autoDraft) {
      setDraft(autoDraft);
      const sel: Record<number, boolean> = {};
      autoDraft.stages.forEach((_, i) => {
        sel[i] = true;
      });
      setStageSelection(sel);
      setStep('configure');
    } else if (source === 'github') {
      try {
        setLoading(true);
        setError(null);
        const stages = await workflowsToPipelineStages(repoPath);
        const ghDraft: Pipeline = {
          name: autoDraft?.name || repoName,
          stages,
        };
        setDraft(ghDraft);
        const sel: Record<number, boolean> = {};
        stages.forEach((_, i) => {
          sel[i] = true;
        });
        setStageSelection(sel);
        setStep('configure');
      } catch (err) {
        setError(String(err));
      } finally {
        setLoading(false);
      }
    }
  }

  function toggleStage(idx: number) {
    setStageSelection((prev) => ({ ...prev, [idx]: !prev[idx] }));
  }

  function addSuggestion(name: string, commands: string[]) {
    if (!draft) return;
    const newStage: Stage = {
      name,
      commands,
      backend: 'local',
      fail_fast: true,
    };
    const newStages = [...draft.stages, newStage];
    setDraft({ ...draft, stages: newStages });
    setStageSelection((prev) => ({ ...prev, [newStages.length - 1]: true }));
  }

  const selectedStages = draft?.stages.filter((_, i) => stageSelection[i]) ?? [];
  const anySelected = selectedStages.length > 0;

  async function handleCreate() {
    try {
      setLoading(true);
      setError(null);

      if (draft && anySelected) {
        const filtered: Pipeline = { name: draft.name, stages: selectedStages };
        await savePipeline(repoPath, filtered);
      }

      // Generate deploy pipeline if deployment method is not skip
      if (deployConfig.method !== 'skip') {
        await generatePipelineWithDeploy(repoPath, repoName, deployConfig);
      }

      await addProject(repoName, repoPath);

      setStep('done');
      setTimeout(() => navigate('/projects'), 800);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  // Helper to select a deployment method
  function handleSelectDeployMethod(method: DeploymentMethod) {
    setSelectedDeployMethod(method);
    setDeployConfig((prev) => ({ ...prev, method }));
  }

  // Get available deploy methods for the current project
  const availableDeployMethods = DEPLOY_METHOD_INFO.filter((info) =>
    suggestedDeployMethods.includes(info.method)
  );

  // Get the selected method info
  const selectedMethodInfo = DEPLOY_METHOD_INFO.find(
    (info) => info.method === selectedDeployMethod
  );

  async function handleSkipPipeline() {
    try {
      setLoading(true);
      setError(null);
      await addProject(repoName || repoNameFromPath(repoPath), repoPath);
      navigate('/projects');
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  const suggestions =
    pipelineSource === 'github' && draft ? computeSuggestions(scripts, draft.stages) : [];

  return (
    <div className="page">
      <header className="page-header">
        <h2 className="page-title">Add Project</h2>
      </header>

      {/* Step indicator */}
      {step !== 'done' && (
        <div className="wizard-steps">
          {WIZARD_STEPS.map((ws, i) => (
            <span key={ws.key} style={{ display: 'contents' }}>
              <div
                className={`wizard-step${i === currentStepIdx ? ' active' : ''}${i < currentStepIdx ? ' completed' : ''}`}
              >
                <span className="wizard-step-circle">
                  {i < currentStepIdx ? <Check size={14} /> : i + 1}
                </span>
                <span className="wizard-step-label">{ws.label}</span>
              </div>
              {i < WIZARD_STEPS.length - 1 && (
                <div className={`wizard-connector${i < currentStepIdx ? ' completed' : ''}`} />
              )}
            </span>
          ))}
        </div>
      )}

      {error && <div className="alert alert-error">{error}</div>}

      {/* Step 1: Select repository */}
      {step === 'select' && (
        <div className="onboarding-card">
          <div className="onboarding-icon">
            <FolderOpen size={32} />
          </div>
          <h3>Select a repository</h3>
          <p>Enter the local path to your project repository.</p>

          {pendingTemplate && (
            <div
              className="alert alert-success"
              style={{ marginBottom: '1rem', display: 'flex', alignItems: 'center', gap: '0.5rem' }}
            >
              <BookTemplate size={14} />
              Template selected: <strong>{pendingTemplate.meta.name}</strong>
              <button
                className="btn btn-icon"
                style={{ marginLeft: 'auto' }}
                onClick={() => {
                  setPendingTemplate(null);
                  setPipelineSource('auto');
                }}
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
                onChange={(e) => setRepoPath(e.target.value)}
              />
              <button
                type="button"
                className="btn btn-secondary btn-browse"
                onClick={async () => {
                  const selected = await open({
                    directory: true,
                    multiple: false,
                    title: 'Select repository folder',
                  });
                  if (selected) {
                    setRepoPath(selected as string);
                    if (!repoName.trim()) {
                      setRepoName(repoNameFromPath(selected as string));
                    }
                  }
                }}
              >
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
              onChange={(e) => setRepoName(e.target.value)}
            />
          </div>

          <div className="form-actions">
            <button className="btn btn-primary" onClick={handleScan} disabled={loading}>
              {loading
                ? 'Scanning...'
                : pendingTemplate
                  ? 'Scan & Apply Template'
                  : 'Scan Repository'}
              <Search size={16} />
            </button>
          </div>
        </div>
      )}

      {/* Step 2: Choose pipeline source */}
      {step === 'source' && (
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
              onClick={() => handlePickSource('auto')}
              disabled={loading}
            >
              <Wand2 size={24} className="source-icon" />
              <span className="source-title">Auto-detect</span>
              <span className="source-desc">
                Generate a pipeline from your detected build files.
              </span>
            </button>

            <button
              className="wizard-source-card"
              onClick={() => handlePickSource('github')}
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

            <button
              className="wizard-source-card"
              onClick={() => {
                setPipelineSource('template');
                setShowTemplateBrowser(true);
              }}
              disabled={loading}
            >
              <BookTemplate size={24} className="source-icon" />
              <span className="source-title">From Template</span>
              <span className="source-desc">
                Choose from built-in or custom pipeline templates.
              </span>
            </button>
          </div>

          <div className="form-actions">
            <button className="btn btn-secondary" onClick={() => setStep('select')}>
              <ArrowLeft size={16} /> Back
            </button>
            <button className="btn btn-secondary" onClick={handleSkipPipeline} disabled={loading}>
              Skip Pipeline
            </button>
          </div>
        </div>
      )}

      {/* Step 3: Configure stages */}
      {step === 'configure' && draft && (
        <div className="onboarding-card onboarding-card--wide">
          <h3>Select pipeline stages</h3>
          <p>
            {pipelineSource === 'github'
              ? 'Stages imported from your GitHub Actions workflows. Toggle the ones you want.'
              : 'Stages auto-detected from your project. Toggle the ones you want.'}
          </p>

          <div className="wizard-stage-list">
            {draft.stages.map((stage, idx) => {
              const checked = stageSelection[idx] ?? false;
              return (
                <label
                  key={idx}
                  className={`wizard-stage-item${checked ? ' selected' : ' excluded'}`}
                >
                  <input type="checkbox" checked={checked} onChange={() => toggleStage(idx)} />
                  <span className="stage-number">{idx + 1}</span>
                  <div className="wizard-stage-info">
                    <strong>{stage.name}</strong>
                    <code>{stage.commands.join(' && ')}</code>
                  </div>
                  <span className="badge badge-neutral">{stage.backend}</span>
                </label>
              );
            })}
          </div>

          {/* Suggestions for GitHub Actions path */}
          {suggestions.length > 0 && (
            <div className="wizard-suggestions">
              <h5>Not covered by your workflows:</h5>
              {suggestions.map((s, i) => (
                <div key={i} className="wizard-suggestion-item">
                  <button
                    type="button"
                    className="btn-add-suggestion"
                    onClick={() => addSuggestion(s.name, s.commands)}
                  >
                    <Plus size={12} /> Add
                  </button>
                  <code>{s.commands.join(' && ')}</code>
                  <span className="suggestion-reason">{s.reason}</span>
                </div>
              ))}
            </div>
          )}

          <div className="form-actions">
            <button className="btn btn-secondary" onClick={() => setStep('source')}>
              <ArrowLeft size={16} /> Back
            </button>
            <button
              className="btn btn-primary"
              onClick={() => (anySelected ? setStep('deploy') : handleCreate())}
              disabled={loading}
            >
              {anySelected ? 'Continue' : 'Skip Pipeline & Create'}
              <ArrowRight size={16} />
            </button>
          </div>
        </div>
      )}

      {/* Step 4: Deploy */}
      {step === 'deploy' && (
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
                onClick={() => handleSelectDeployMethod(info.method)}
              >
                <span className="deploy-icon">{info.icon}</span>
                <span className="deploy-title">{info.label}</span>
                <span className="deploy-desc">{info.description}</span>
                {detectedDeployMethod === info.method && info.method !== 'skip' && (
                  <span className="badge badge-success" style={{ marginTop: '0.5rem' }}>
                    Detected
                  </span>
                )}
              </button>
            ))}
          </div>

          {/* Configuration form for methods that need it */}
          {selectedMethodInfo &&
            (selectedMethodInfo.requiresSshHost ||
              selectedMethodInfo.requiresRegistry ||
              selectedMethodInfo.requiresHealthCheck ||
              selectedMethodInfo.requiresPlatformProject) && (
              <div className="wizard-deploy-config">
                <h5>Configuration</h5>

                {selectedMethodInfo.requiresSshHost && (
                  <div className="form-group">
                    <label htmlFor="ssh-host">SSH Host</label>
                    <input
                      id="ssh-host"
                      type="text"
                      className="input"
                      placeholder="user@server.example.com"
                      value={deployConfig.ssh_host || ''}
                      onChange={(e) =>
                        setDeployConfig((prev) => ({ ...prev, ssh_host: e.target.value }))
                      }
                    />
                  </div>
                )}

                {selectedMethodInfo.requiresRegistry && (
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

                {selectedMethodInfo.requiresHealthCheck && (
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

                {selectedMethodInfo.requiresPlatformProject && (
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

                {(selectedDeployMethod === 'cargo_publish' ||
                  selectedDeployMethod === 'npm_publish') && (
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
            <button className="btn btn-secondary" onClick={() => setStep('configure')}>
              <ArrowLeft size={16} /> Back
            </button>
            <button
              className="btn btn-primary"
              onClick={() => setStep('review')}
              disabled={loading}
            >
              Continue
              <ArrowRight size={16} />
            </button>
          </div>
        </div>
      )}

      {/* Step 5: Review */}
      {step === 'review' && draft && (
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

          <h5 style={{ marginTop: 'var(--space-lg)' }}>CI Pipeline Stages</h5>
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
              <h5 style={{ marginTop: 'var(--space-lg)' }}>CD Pipeline (deploy.toml)</h5>
              <p className="text-muted text-sm">
                A separate deploy pipeline will be created with{' '}
                {selectedMethodInfo?.label || 'deployment'} stages.
              </p>
            </>
          )}

          <div className="form-actions" style={{ marginTop: 'var(--space-xl)' }}>
            <button className="btn btn-secondary" onClick={() => setStep('deploy')}>
              <ArrowLeft size={16} /> Back
            </button>
            <button className="btn btn-primary" onClick={handleCreate} disabled={loading}>
              {loading ? 'Creating...' : 'Create Project'}
              <ArrowRight size={16} />
            </button>
          </div>
        </div>
      )}

      {/* Done */}
      {step === 'done' && (
        <div className="onboarding-card">
          <h3>Project added!</h3>
          <p>Redirecting to projects...</p>
        </div>
      )}

      {/* Template browser modal */}
      {showTemplateBrowser && (
        <div className="modal-backdrop" onClick={() => setShowTemplateBrowser(false)}>
          <div
            className="modal"
            onClick={(e) => e.stopPropagation()}
            style={{ maxWidth: 700, maxHeight: '80vh', overflow: 'auto' }}
          >
            <div className="modal-header">
              <h3>Choose a Pipeline Template</h3>
              <button className="btn-icon" onClick={() => setShowTemplateBrowser(false)}>
                ✕
              </button>
            </div>
            <div className="modal-body">
              <TemplateBrowser
                repoPath={repoPath}
                filterType="pipeline"
                onApply={(t) => {
                  setSelectedTemplate(t);
                  setShowTemplateBrowser(false);
                }}
              />
            </div>
          </div>
        </div>
      )}

      {/* Template variable dialog */}
      {selectedTemplate && (
        <TemplateVariableDialog
          template={selectedTemplate}
          repoPath={repoPath}
          projectName={repoName}
          onApplied={(pipeline) => {
            setDraft(pipeline);
            const sel: Record<number, boolean> = {};
            pipeline.stages.forEach((_, i) => {
              sel[i] = true;
            });
            setStageSelection(sel);
            setSelectedTemplate(null);
            setStep('configure');
          }}
          onCancel={() => setSelectedTemplate(null)}
        />
      )}
    </div>
  );
}

export default AddProject;
