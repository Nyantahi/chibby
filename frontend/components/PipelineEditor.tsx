import { useState, useEffect } from 'react';
import {
  Plus,
  Trash2,
  GripVertical,
  ChevronDown,
  ChevronRight,
  ArrowUp,
  ArrowDown,
  X,
  Heart,
  Download,
  GitBranch,
  Save,
  Layers,
} from 'lucide-react';
import { savePipeline, getGithubWorkflows } from '../services/api';
import type {
  Pipeline,
  Stage,
  Backend,
  HealthCheck,
  WorkflowInfo,
  PipelineTemplate,
} from '../types';
import TemplateBrowser from './TemplateBrowser';
import TemplateVariableDialog from './TemplateVariableDialog';
import SaveAsTemplate from './SaveAsTemplate';

interface Props {
  repoPath: string;
  pipeline: Pipeline;
  onSaved: () => void;
}

function emptyStage(): Stage {
  return {
    name: '',
    commands: [''],
    backend: 'local',
    fail_fast: true,
  };
}

function PipelineEditor({ repoPath, pipeline, onSaved }: Props) {
  const [name, setName] = useState(pipeline.name);
  const [stages, setStages] = useState<Stage[]>(
    pipeline.stages.map((s) => ({ ...s, commands: [...s.commands] }))
  );
  const [expandedStage, setExpandedStage] = useState<number | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Stage templates (loaded from API)
  const [showStageBrowser, setShowStageBrowser] = useState(false);
  const [selectedStageTemplate, setSelectedStageTemplate] = useState<PipelineTemplate | null>(null);

  // Save as template
  const [showSaveAsTemplate, setShowSaveAsTemplate] = useState(false);

  // Import from GitHub Actions
  const [showImport, setShowImport] = useState(false);
  const [workflows, setWorkflows] = useState<WorkflowInfo[]>([]);
  const [loadingWorkflows, setLoadingWorkflows] = useState(false);
  const [selectedSteps, setSelectedSteps] = useState<Set<string>>(new Set());

  const hasChanges =
    name !== pipeline.name || JSON.stringify(stages) !== JSON.stringify(pipeline.stages);

  // Load workflows when import modal opens
  useEffect(() => {
    if (showImport && workflows.length === 0) {
      loadWorkflows();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [showImport]);

  async function loadWorkflows() {
    try {
      setLoadingWorkflows(true);
      const wfs = await getGithubWorkflows(repoPath);
      setWorkflows(wfs);
    } catch {
      // Workflows not found is fine
      setWorkflows([]);
    } finally {
      setLoadingWorkflows(false);
    }
  }

  function toggleStepSelection(jobName: string, stepIdx: number) {
    const key = `${jobName}:${stepIdx}`;
    const next = new Set(selectedSteps);
    if (next.has(key)) {
      next.delete(key);
    } else {
      next.add(key);
    }
    setSelectedSteps(next);
  }

  function importSelectedSteps() {
    // Collect selected steps and create stages
    const newStages: Stage[] = [];
    for (const wf of workflows) {
      for (const job of wf.jobs) {
        job.steps.forEach((step, idx) => {
          const key = `${job.name}:${idx}`;
          if (selectedSteps.has(key)) {
            const stageName = step.name || `${job.name} - step ${idx + 1}`;
            // Split multi-line run commands
            const commands = step.run
              .split('\n')
              .map((l) => l.trim())
              .filter((l) => l && !l.startsWith('#'));
            newStages.push({
              name: stageName,
              commands: commands.length > 0 ? commands : [step.run],
              backend: 'local',
              fail_fast: true,
            });
          }
        });
      }
    }
    if (newStages.length > 0) {
      setStages([...stages, ...newStages]);
    }
    setShowImport(false);
    setSelectedSteps(new Set());
  }

  async function handleSave() {
    // Validate
    const cleaned = stages.filter((s) => s.name.trim());
    if (cleaned.length === 0) {
      setError('Pipeline must have at least one stage.');
      return;
    }
    for (const s of cleaned) {
      if (s.commands.filter((c) => c.trim()).length === 0) {
        setError(`Stage "${s.name}" has no commands.`);
        return;
      }
    }

    try {
      setSaving(true);
      setError(null);
      const p: Pipeline = {
        name: name.trim() || 'Pipeline',
        stages: cleaned.map((s) => ({
          ...s,
          commands: s.commands.filter((c) => c.trim()),
        })),
      };
      await savePipeline(repoPath, p);
      onSaved();
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  }

  function updateStage(idx: number, updates: Partial<Stage>) {
    setStages(stages.map((s, i) => (i === idx ? { ...s, ...updates } : s)));
  }

  function removeStage(idx: number) {
    setStages(stages.filter((_, i) => i !== idx));
    if (expandedStage === idx) setExpandedStage(null);
  }

  function moveStage(idx: number, dir: -1 | 1) {
    const target = idx + dir;
    if (target < 0 || target >= stages.length) return;
    const next = [...stages];
    [next[idx], next[target]] = [next[target], next[idx]];
    setStages(next);
    if (expandedStage === idx) setExpandedStage(target);
    else if (expandedStage === target) setExpandedStage(idx);
  }

  function addStage() {
    setStages([...stages, emptyStage()]);
    setExpandedStage(stages.length);
  }

  function handleStageTemplateApply(template: PipelineTemplate) {
    setSelectedStageTemplate(template);
  }

  function handleStageTemplateApplied(applied: Pipeline) {
    // Add all stages from the applied template
    const newStages = applied.stages.map((s) => ({
      ...s,
      commands: [...s.commands],
      health_check: s.health_check ? { ...s.health_check } : undefined,
    }));
    setStages([...stages, ...newStages]);
    setExpandedStage(stages.length);
    setSelectedStageTemplate(null);
    setShowStageBrowser(false);
  }

  function updateCommand(stageIdx: number, cmdIdx: number, value: string) {
    const cmds = [...stages[stageIdx].commands];
    cmds[cmdIdx] = value;
    updateStage(stageIdx, { commands: cmds });
  }

  function addCommand(stageIdx: number) {
    const cmds = [...stages[stageIdx].commands, ''];
    updateStage(stageIdx, { commands: cmds });
  }

  function removeCommand(stageIdx: number, cmdIdx: number) {
    const cmds = stages[stageIdx].commands.filter((_, i) => i !== cmdIdx);
    updateStage(stageIdx, { commands: cmds.length > 0 ? cmds : [''] });
  }

  function toggleHealthCheck(stageIdx: number) {
    const stage = stages[stageIdx];
    if (stage.health_check) {
      updateStage(stageIdx, { health_check: undefined });
    } else {
      updateStage(stageIdx, {
        health_check: {
          command: 'curl -sf http://localhost:8080/health',
          retries: 3,
          delay_secs: 5,
        },
      });
    }
  }

  function updateHealthCheck(stageIdx: number, updates: Partial<HealthCheck>) {
    const hc = stages[stageIdx].health_check;
    if (!hc) return;
    updateStage(stageIdx, { health_check: { ...hc, ...updates } });
  }

  return (
    <div className="pipeline-editor">
      <div className="section-header-row">
        <h3 className="section-title">Pipeline Settings</h3>
        <div className="section-header-actions">
          <button
            className="btn btn-secondary btn-sm"
            onClick={() => setShowImport(true)}
            title="Import from GitHub Actions"
          >
            <Download size={14} /> Import CI
          </button>
          <button
            className="btn btn-secondary btn-sm"
            onClick={() => setShowStageBrowser(true)}
            title="Browse stage templates"
          >
            <Layers size={14} /> Stage Templates
          </button>
          <button className="btn btn-secondary btn-sm" onClick={addStage}>
            <Plus size={14} /> Add Stage
          </button>
          <button
            className="btn btn-secondary btn-sm"
            onClick={() => setShowSaveAsTemplate(true)}
            title="Save this pipeline as a reusable template"
          >
            <Save size={14} /> Save as Template
          </button>
          {hasChanges && (
            <button className="btn btn-primary btn-sm" onClick={handleSave} disabled={saving}>
              {saving ? 'Saving...' : 'Save Pipeline'}
            </button>
          )}
        </div>
      </div>

      {/* Import from GitHub Actions modal */}
      {showImport && (
        <div className="modal-backdrop" onClick={() => setShowImport(false)}>
          <div className="modal import-modal" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>
                <GitBranch size={16} /> Import from GitHub Actions
              </h3>
              <button className="btn btn-icon" onClick={() => setShowImport(false)}>
                <X size={14} />
              </button>
            </div>
            <div className="modal-body">
              {loadingWorkflows ? (
                <p className="text-muted">Loading workflows...</p>
              ) : workflows.length === 0 ? (
                <p className="text-muted">
                  No GitHub Actions workflows found in .github/workflows/
                </p>
              ) : (
                <div className="import-workflows">
                  {workflows.map((wf, wi) => (
                    <div key={wi} className="import-workflow">
                      <div className="import-workflow-header">
                        <strong>{wf.name || wf.file_name}</strong>
                        <span className="text-muted text-xs">{wf.file_name}</span>
                      </div>
                      {wf.jobs.map((job, ji) => (
                        <div key={ji} className="import-job">
                          <div className="import-job-header">{job.name}</div>
                          <div className="import-steps">
                            {job.steps.map((step, si) => {
                              const key = `${job.name}:${si}`;
                              const isSelected = selectedSteps.has(key);
                              return (
                                <label
                                  key={si}
                                  className={`import-step ${isSelected ? 'selected' : ''}`}
                                >
                                  <input
                                    type="checkbox"
                                    checked={isSelected}
                                    onChange={() => toggleStepSelection(job.name, si)}
                                  />
                                  <span className="import-step-name">
                                    {step.name || `Step ${si + 1}`}
                                  </span>
                                  <code className="import-step-cmd">
                                    {step.run.length > 60
                                      ? step.run.substring(0, 60) + '...'
                                      : step.run}
                                  </code>
                                </label>
                              );
                            })}
                          </div>
                        </div>
                      ))}
                    </div>
                  ))}
                </div>
              )}
            </div>
            <div className="modal-footer">
              <button className="btn btn-secondary" onClick={() => setShowImport(false)}>
                Cancel
              </button>
              <button
                className="btn btn-primary"
                onClick={importSelectedSteps}
                disabled={selectedSteps.size === 0}
              >
                Import {selectedSteps.size > 0 ? `(${selectedSteps.size})` : ''}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Stage template browser modal */}
      {showStageBrowser && (
        <div className="modal-backdrop" onClick={() => setShowStageBrowser(false)}>
          <div className="modal modal-lg" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>
                <Layers size={16} /> Stage Templates
              </h3>
              <button className="btn btn-icon" onClick={() => setShowStageBrowser(false)}>
                <X size={14} />
              </button>
            </div>
            <div className="modal-body">
              <TemplateBrowser
                repoPath={repoPath}
                filterType="stage"
                onApply={handleStageTemplateApply}
              />
            </div>
          </div>
        </div>
      )}

      {/* Template variable dialog for stage templates */}
      {selectedStageTemplate && (
        <TemplateVariableDialog
          template={selectedStageTemplate}
          repoPath={repoPath}
          onApplied={handleStageTemplateApplied}
          onCancel={() => setSelectedStageTemplate(null)}
        />
      )}

      {/* Save as template dialog */}
      {showSaveAsTemplate && (
        <SaveAsTemplate
          pipeline={{ name, stages: stages.filter((s) => s.name.trim()) }}
          repoPath={repoPath}
          onSaved={() => setShowSaveAsTemplate(false)}
          onCancel={() => setShowSaveAsTemplate(false)}
        />
      )}

      {error && <div className="alert alert-error">{error}</div>}

      {/* Pipeline name */}
      <div className="form-group">
        <label htmlFor="pipeline-name">Pipeline Name</label>
        <input
          id="pipeline-name"
          className="input"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="My Pipeline"
        />
      </div>

      {/* Stage list */}
      <div className="pe-stage-list">
        {stages.map((stage, idx) => {
          const isExpanded = expandedStage === idx;
          return (
            <div key={idx} className="pe-stage-card">
              {/* Stage header — always visible */}
              <div className="pe-stage-header">
                <GripVertical size={14} className="pe-grip" />
                <span className="stage-number">{idx + 1}</span>
                <button
                  className="pe-stage-toggle"
                  onClick={() => setExpandedStage(isExpanded ? null : idx)}
                >
                  {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                </button>
                {isExpanded ? (
                  <input
                    className="input input-sm pe-stage-name-input"
                    value={stage.name}
                    onChange={(e) => updateStage(idx, { name: e.target.value })}
                    placeholder="Stage name"
                    autoFocus
                  />
                ) : (
                  <strong className="pe-stage-name">
                    {stage.name || <span className="text-muted">unnamed</span>}
                  </strong>
                )}
                <span className="badge badge-neutral">{stage.backend}</span>
                {stage.health_check && (
                  <span className="badge badge-neutral" title="Health check configured">
                    HC
                  </span>
                )}
                <span className="pe-cmd-count text-muted text-xs">
                  {stage.commands.filter((c) => c.trim()).length} cmd
                  {stage.commands.filter((c) => c.trim()).length !== 1 ? 's' : ''}
                </span>
                <div className="pe-stage-actions">
                  <button
                    className="btn btn-icon btn-sm"
                    title="Move up"
                    disabled={idx === 0}
                    onClick={() => moveStage(idx, -1)}
                  >
                    <ArrowUp size={12} />
                  </button>
                  <button
                    className="btn btn-icon btn-sm"
                    title="Move down"
                    disabled={idx === stages.length - 1}
                    onClick={() => moveStage(idx, 1)}
                  >
                    <ArrowDown size={12} />
                  </button>
                  <button
                    className="btn btn-icon btn-sm btn-danger-icon"
                    title="Remove stage"
                    onClick={() => removeStage(idx)}
                  >
                    <Trash2 size={12} />
                  </button>
                </div>
              </div>

              {/* Stage detail — expanded */}
              {isExpanded && (
                <div className="pe-stage-body">
                  {/* Backend + working dir row */}
                  <div className="pe-field-row">
                    <div className="form-group form-group-inline">
                      <label>Backend</label>
                      <select
                        className="input input-sm"
                        value={stage.backend}
                        onChange={(e) => updateStage(idx, { backend: e.target.value as Backend })}
                      >
                        <option value="local">Local</option>
                        <option value="ssh">SSH</option>
                      </select>
                    </div>
                    <div className="form-group form-group-inline">
                      <label>Working Directory</label>
                      <input
                        className="input input-sm"
                        value={stage.working_dir ?? ''}
                        onChange={(e) =>
                          updateStage(idx, {
                            working_dir: e.target.value || undefined,
                          })
                        }
                        placeholder={stage.backend === 'ssh' ? '/remote/path' : 'relative/path'}
                      />
                    </div>
                    <div className="form-group form-group-inline pe-checkbox-group">
                      <label>
                        <input
                          type="checkbox"
                          checked={stage.fail_fast}
                          onChange={(e) => updateStage(idx, { fail_fast: e.target.checked })}
                        />{' '}
                        Fail fast
                      </label>
                    </div>
                  </div>

                  {/* Commands */}
                  <div className="pe-commands-section">
                    <div className="pe-commands-header">
                      <span className="env-vars-label">Commands</span>
                      <button
                        className="btn btn-icon btn-sm"
                        onClick={() => addCommand(idx)}
                        title="Add command"
                      >
                        <Plus size={12} />
                      </button>
                    </div>
                    {stage.commands.map((cmd, ci) => (
                      <div key={ci} className="pe-command-row">
                        <span className="pe-command-prefix">$</span>
                        <input
                          className="input input-sm pe-command-input"
                          value={cmd}
                          onChange={(e) => updateCommand(idx, ci, e.target.value)}
                          placeholder="command..."
                        />
                        {stage.commands.length > 1 && (
                          <button
                            className="btn btn-icon btn-sm"
                            onClick={() => removeCommand(idx, ci)}
                          >
                            <X size={12} />
                          </button>
                        )}
                      </div>
                    ))}
                  </div>

                  {/* Health check */}
                  <div className="pe-health-section">
                    <button className="pe-health-toggle" onClick={() => toggleHealthCheck(idx)}>
                      <Heart size={14} />
                      {stage.health_check ? 'Remove Health Check' : 'Add Health Check'}
                    </button>
                    {stage.health_check && (
                      <div className="pe-health-fields">
                        <div className="pe-field-row">
                          <div className="form-group form-group-inline" style={{ flex: 2 }}>
                            <label>Command</label>
                            <input
                              className="input input-sm"
                              value={stage.health_check.command}
                              onChange={(e) => updateHealthCheck(idx, { command: e.target.value })}
                              placeholder="curl -sf http://localhost/health"
                            />
                          </div>
                          <div className="form-group form-group-inline">
                            <label>Retries</label>
                            <input
                              className="input input-sm input-narrow"
                              type="number"
                              min={1}
                              value={stage.health_check.retries}
                              onChange={(e) =>
                                updateHealthCheck(idx, {
                                  retries: Number(e.target.value) || 1,
                                })
                              }
                            />
                          </div>
                          <div className="form-group form-group-inline">
                            <label>Delay (s)</label>
                            <input
                              className="input input-sm input-narrow"
                              type="number"
                              min={1}
                              value={stage.health_check.delay_secs}
                              onChange={(e) =>
                                updateHealthCheck(idx, {
                                  delay_secs: Number(e.target.value) || 1,
                                })
                              }
                            />
                          </div>
                        </div>
                      </div>
                    )}
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </div>

      {stages.length === 0 && (
        <div className="empty-state-small">
          <p>No stages. Click "Add Stage" to start building your pipeline.</p>
        </div>
      )}
    </div>
  );
}

export default PipelineEditor;
