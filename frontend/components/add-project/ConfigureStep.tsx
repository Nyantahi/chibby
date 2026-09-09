import { ArrowLeft, ArrowRight, Plus } from 'lucide-react';
import type { Pipeline } from '../../types';
import type { PipelineSource } from './constants';

interface Suggestion {
  name: string;
  commands: string[];
  reason: string;
}

interface ConfigureStepProps {
  draft: Pipeline;
  pipelineSource: PipelineSource;
  stageSelection: Record<number, boolean>;
  suggestions: Suggestion[];
  anySelected: boolean;
  loading: boolean;
  onToggleStage: (idx: number) => void;
  onAddSuggestion: (name: string, commands: string[]) => void;
  onBack: () => void;
  onContinue: () => void;
}

function ConfigureStep({
  draft,
  pipelineSource,
  stageSelection,
  suggestions,
  anySelected,
  loading,
  onToggleStage,
  onAddSuggestion,
  onBack,
  onContinue,
}: ConfigureStepProps) {
  return (
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
            <label key={idx} className={`wizard-stage-item${checked ? ' selected' : ' excluded'}`}>
              <input type="checkbox" checked={checked} onChange={() => onToggleStage(idx)} />
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
                onClick={() => onAddSuggestion(s.name, s.commands)}
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
        <button className="btn btn-secondary" onClick={onBack}>
          <ArrowLeft size={16} /> Back
        </button>
        <button className="btn btn-primary" onClick={onContinue} disabled={loading}>
          {anySelected ? 'Continue' : 'Skip Pipeline & Create'}
          <ArrowRight size={16} />
        </button>
      </div>
    </div>
  );
}

export default ConfigureStep;
