import { useState, useEffect } from 'react';
import { useNavigate, useLocation } from 'react-router-dom';
import { Check } from 'lucide-react';
import { open } from '@tauri-apps/plugin-dialog';
import {
  addProject,
  autoBootstrapForProject,
  detectScripts,
  generatePipelineWithDeploy,
  savePipeline,
  getGithubWorkflows,
  workflowsToPipelineStages,
  detectDeploymentMethod,
  getSuggestedDeployMethods,
  detectProjectType,
} from '../services/api';
import { notifySuccess } from '../services/notify';
import { repoNameFromPath } from '../utils/format';
import TemplateBrowser from './TemplateBrowser';
import TemplateVariableDialog from './TemplateVariableDialog';
import BootstrapWizardModal from './BootstrapWizardModal';
import {
  WIZARD_STEPS,
  DEPLOY_METHOD_INFO,
  computeSuggestions,
  type WizardStep,
  type PipelineSource,
} from './add-project/constants';
import SelectStep from './add-project/SelectStep';
import SourceStep from './add-project/SourceStep';
import ConfigureStep from './add-project/ConfigureStep';
import DeployStep from './add-project/DeployStep';
import ReviewStep from './add-project/ReviewStep';
import type {
  BootstrapReport,
  DetectedScript,
  Pipeline,
  PipelineTemplate,
  Stage,
  WorkflowInfo,
  DeploymentMethod,
  DeploymentConfig,
  ProjectType,
} from '../types';

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
  // Pick up template passed from the Templates page via router state.
  // Reading once via useState lazy init avoids the "setState in effect" warning
  // that would otherwise fire if we mirrored location.state -> component state.
  const incomingTemplate = useState(
    () =>
      (location.state as { template?: PipelineTemplate; editMode?: boolean } | null)?.template ??
      null
  )[0];
  const [pipelineSource, setPipelineSource] = useState<PipelineSource>(
    incomingTemplate ? 'template' : 'auto'
  );
  const [stageSelection, setStageSelection] = useState<Record<number, boolean>>({});
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Template state
  const [showTemplateBrowser, setShowTemplateBrowser] = useState(false);
  const [selectedTemplate, setSelectedTemplate] = useState<PipelineTemplate | null>(null);
  // Template pre-selected from the Templates page — waits for repo selection before applying
  const [pendingTemplate, setPendingTemplate] = useState<PipelineTemplate | null>(incomingTemplate);

  // Deployment state
  const [projectType, setProjectType] = useState<ProjectType>('Unknown');
  const [detectedDeployMethod, setDetectedDeployMethod] = useState<DeploymentMethod>('skip');
  const [suggestedDeployMethods, setSuggestedDeployMethods] = useState<DeploymentMethod[]>([]);
  const [selectedDeployMethod, setSelectedDeployMethod] = useState<DeploymentMethod>('skip');
  const [deployConfig, setDeployConfig] = useState<DeploymentConfig>({
    method: 'skip',
    dry_run_first: true,
  });
  const [bootstrapReview, setBootstrapReview] = useState<BootstrapReport | null>(null);
  const [bootstrapTarget, setBootstrapTarget] = useState<string | null>(null);

  // Clear router state once on mount so refreshing doesn't re-apply the template.
  // (The template itself is read via the useState lazy initializer above.)
  useEffect(() => {
    if (incomingTemplate) {
      window.history.replaceState({}, '');
    }
    // Intentional one-shot; incomingTemplate is captured at mount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const currentStepIdx = WIZARD_STEPS.findIndex((s) => s.key === step);

  async function handleBrowse() {
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
  }

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

  async function handleAutoBootstrap(projectPath: string): Promise<boolean> {
    try {
      const outcome = await autoBootstrapForProject(projectPath);
      if (outcome.mode === 'silent' && outcome.applied) {
        notifySuccess('Bootstrap applied', 'Detected env/secrets written to .chibby/');
        return false;
      }
      if (outcome.mode === 'confirm' && outcome.report && outcome.report.detected.length > 0) {
        setBootstrapTarget(projectPath);
        setBootstrapReview(outcome.report);
        return true;
      }
    } catch (err) {
      // Auto-bootstrap is best-effort. Don't block project creation.
      console.warn('auto_bootstrap_for_project failed', err);
    }
    return false;
  }

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
      const deferred = await handleAutoBootstrap(repoPath);
      if (!deferred) {
        setTimeout(() => navigate('/projects'), 800);
      }
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
      const deferred = await handleAutoBootstrap(repoPath);
      if (!deferred) navigate('/projects');
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
            <span key={ws.key} className="wizard-step-group">
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

      {step === 'select' && (
        <SelectStep
          pendingTemplate={pendingTemplate}
          repoPath={repoPath}
          repoName={repoName}
          loading={loading}
          onRepoPathChange={setRepoPath}
          onRepoNameChange={setRepoName}
          onBrowse={handleBrowse}
          onScan={handleScan}
          onRemoveTemplate={() => {
            setPendingTemplate(null);
            setPipelineSource('auto');
          }}
        />
      )}

      {step === 'source' && (
        <SourceStep
          scripts={scripts}
          workflows={workflows}
          loading={loading}
          onPickSource={handlePickSource}
          onOpenTemplateBrowser={() => {
            setPipelineSource('template');
            setShowTemplateBrowser(true);
          }}
          onBack={() => setStep('select')}
          onSkip={handleSkipPipeline}
        />
      )}

      {step === 'configure' && draft && (
        <ConfigureStep
          draft={draft}
          pipelineSource={pipelineSource}
          stageSelection={stageSelection}
          suggestions={suggestions}
          anySelected={anySelected}
          loading={loading}
          onToggleStage={toggleStage}
          onAddSuggestion={addSuggestion}
          onBack={() => setStep('source')}
          onContinue={() => (anySelected ? setStep('deploy') : handleCreate())}
        />
      )}

      {step === 'deploy' && (
        <DeployStep
          projectType={projectType}
          detectedDeployMethod={detectedDeployMethod}
          availableDeployMethods={availableDeployMethods}
          selectedDeployMethod={selectedDeployMethod}
          selectedMethodInfo={selectedMethodInfo}
          deployConfig={deployConfig}
          setDeployConfig={setDeployConfig}
          loading={loading}
          onSelectDeployMethod={handleSelectDeployMethod}
          onBack={() => setStep('configure')}
          onContinue={() => setStep('review')}
        />
      )}

      {step === 'review' && draft && (
        <ReviewStep
          repoName={repoName}
          repoPath={repoPath}
          projectType={projectType}
          pipelineSource={pipelineSource}
          selectedStages={selectedStages}
          selectedDeployMethod={selectedDeployMethod}
          selectedMethodInfo={selectedMethodInfo}
          deployConfig={deployConfig}
          loading={loading}
          onBack={() => setStep('deploy')}
          onCreate={handleCreate}
        />
      )}

      {step === 'done' && (
        <div className="onboarding-card">
          <h3>Project added!</h3>
          <p>Redirecting to projects...</p>
        </div>
      )}

      {/* Template browser modal */}
      {showTemplateBrowser && (
        <div className="modal-backdrop" onClick={() => setShowTemplateBrowser(false)}>
          <div className="modal add-project-template-modal" onClick={(e) => e.stopPropagation()}>
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

      {bootstrapReview && bootstrapTarget && (
        <BootstrapWizardModal
          repoPath={bootstrapTarget}
          initialReport={bootstrapReview}
          onClose={() => {
            setBootstrapReview(null);
            setBootstrapTarget(null);
            navigate('/projects');
          }}
          onApplied={() => {
            setBootstrapReview(null);
            setBootstrapTarget(null);
            navigate('/projects');
          }}
        />
      )}
    </div>
  );
}

export default AddProject;
