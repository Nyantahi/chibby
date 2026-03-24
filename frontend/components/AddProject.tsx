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
} from 'lucide-react';
import { open } from '@tauri-apps/plugin-dialog';
import {
  addProject,
  detectScripts,
  generatePipeline,
  savePipeline,
  getGithubWorkflows,
  workflowsToPipelineStages,
} from '../services/api';
import { repoNameFromPath } from '../utils/format';
import { FileTypeIcon } from './FileTypeIcon';
import TemplateBrowser from './TemplateBrowser';
import TemplateVariableDialog from './TemplateVariableDialog';
import type { DetectedScript, Pipeline, PipelineTemplate, Stage, WorkflowInfo } from '../types';

type WizardStep = 'select' | 'source' | 'configure' | 'review' | 'done';
type PipelineSource = 'auto' | 'github' | 'template';

const WIZARD_STEPS: { key: WizardStep; label: string }[] = [
  { key: 'select', label: 'Select' },
  { key: 'source', label: 'Source' },
  { key: 'configure', label: 'Configure' },
  { key: 'review', label: 'Review' },
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

  // Pick up template passed from the Templates page via router state
  useEffect(() => {
    const state = location.state as { template?: PipelineTemplate; editMode?: boolean } | null;
    if (state?.template) {
      setSelectedTemplate(state.template);
      setPipelineSource('template');
      // Clear state so refreshing doesn't re-trigger
      window.history.replaceState({}, '');
    }
  }, [location.state]);

  const currentStepIdx = WIZARD_STEPS.findIndex((s) => s.key === step);

  // Scan repo: detect scripts, generate auto pipeline, check for workflows
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

      const [detected, pipeline, wfs] = await Promise.all([
        detectScripts(repoPath),
        generatePipeline(repoPath, name),
        getGithubWorkflows(repoPath).catch(() => [] as WorkflowInfo[]),
      ]);

      setScripts(detected);
      setAutoDraft(pipeline);
      setDraft(pipeline);
      setWorkflows(wfs);

      // Initialize stage selection from auto draft
      const sel: Record<number, boolean> = {};
      pipeline.stages.forEach((_, i) => {
        sel[i] = true;
      });
      setStageSelection(sel);

      setStep('source');
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
      await addProject(repoName, repoPath);

      setStep('done');
      setTimeout(() => navigate('/projects'), 800);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

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
              {loading ? 'Scanning...' : 'Scan Repository'}
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
              onClick={() => (anySelected ? setStep('review') : handleCreate())}
              disabled={loading}
            >
              {anySelected ? 'Continue' : 'Skip Pipeline & Create'}
              <ArrowRight size={16} />
            </button>
          </div>
        </div>
      )}

      {/* Step 4: Review */}
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
              <span className="wizard-summary-label">Source</span>
              <span className="wizard-summary-value">
                {pipelineSource === 'github' ? 'GitHub Actions' : 'Auto-detected'}
              </span>
            </div>
            <div className="wizard-summary-row">
              <span className="wizard-summary-label">Stages</span>
              <span className="wizard-summary-value">{selectedStages.length}</span>
            </div>
          </div>

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

          <div className="form-actions" style={{ marginTop: 'var(--space-xl)' }}>
            <button className="btn btn-secondary" onClick={() => setStep('configure')}>
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
          <div className="modal" onClick={(e) => e.stopPropagation()} style={{ maxWidth: 700, maxHeight: '80vh', overflow: 'auto' }}>
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
            pipeline.stages.forEach((_, i) => { sel[i] = true; });
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
