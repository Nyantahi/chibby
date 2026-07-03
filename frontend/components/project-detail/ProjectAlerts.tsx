import { CheckCircle, AlertTriangle, Copy } from 'lucide-react';
import type { PreflightResult, PipelineValidation } from '../../types';
import { formatPreflightError } from './helpers';

interface ProjectAlertsProps {
  error: string | null;
  preflightResult: PreflightResult | null;
  validation: PipelineValidation | null;
}

function ProjectAlerts({ error, preflightResult, validation }: ProjectAlertsProps) {
  return (
    <>
      {error && <div className="alert alert-error">{error}</div>}

      {/* Preflight result */}
      {preflightResult && (
        <div
          className={`preflight-result ${
            preflightResult.passed ? 'preflight-pass' : 'preflight-fail'
          }`}
        >
          <div className="preflight-header">
            {preflightResult.passed ? (
              <>
                <CheckCircle size={16} /> Preflight passed
              </>
            ) : (
              <>
                <AlertTriangle size={16} /> Preflight failed
              </>
            )}
          </div>
          {preflightResult.errors.length > 0 && (
            <ul className="preflight-errors">
              {preflightResult.errors.map((err, i) => (
                <li key={i}>{formatPreflightError(err)}</li>
              ))}
            </ul>
          )}
          {preflightResult.warnings.length > 0 && (
            <ul className="preflight-warnings">
              {preflightResult.warnings.map((w, i) => (
                <li key={i}>{w}</li>
              ))}
            </ul>
          )}
        </div>
      )}

      {/* Pipeline validation warnings */}
      {validation && validation.warnings.length > 0 && (
        <div className="validation-warnings">
          <div className="validation-header">
            <AlertTriangle size={16} />
            Pipeline Issues Detected
          </div>
          <ul className="validation-list">
            {validation.warnings.map((w, i) => (
              <li key={i} className={`validation-item validation-${w.severity}`}>
                <div className="validation-stage">Stage: {w.stage_name}</div>
                <div className="validation-command">
                  <code>{w.command}</code>
                </div>
                <div className="validation-message">{w.message}</div>
                {w.suggestion && <div className="validation-suggestion">{w.suggestion}</div>}
              </li>
            ))}
          </ul>
        </div>
      )}

      {/* File conflicts */}
      {validation && validation.file_conflicts && validation.file_conflicts.length > 0 && (
        <div className="file-conflicts">
          <div className="conflicts-header">
            <Copy size={16} />
            Duplicate Config Files Detected
          </div>
          <ul className="conflicts-list">
            {validation.file_conflicts.map((conflict, i) => (
              <li key={i} className="conflict-item">
                <div className="conflict-category">{conflict.category}</div>
                <div className="conflict-files">
                  {conflict.files.map((file, fi) => (
                    <span
                      key={fi}
                      className={`conflict-file ${conflict.active_file === file ? 'conflict-file-active' : ''}`}
                    >
                      {file}
                      {conflict.active_file === file && (
                        <span className="active-badge">active</span>
                      )}
                    </span>
                  ))}
                </div>
                <div className="conflict-message">{conflict.message}</div>
              </li>
            ))}
          </ul>
        </div>
      )}
    </>
  );
}

export default ProjectAlerts;
