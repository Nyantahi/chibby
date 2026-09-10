import { Link } from 'react-router-dom';
import { HeartCrack, ShieldAlert, ShieldCheck, TriangleAlert } from 'lucide-react';
import type { PipelineRun, RollbackOutcome } from '../../types';

/** Router state carried through so links keep the caller's project/tab context. */
export interface RunLinkState {
  projectId?: string;
  tab?: string;
}

interface Props {
  run: PipelineRun;
  linkState?: RunLinkState;
}

interface OutcomeCopy {
  /** CSS modifier suffix — maps to a `.rollback-notice-*` tone. */
  tone: string;
  title: string;
  detail: string;
  linkLabel: string;
}

const OUTCOME_COPY: Record<RollbackOutcome, OutcomeCopy> = {
  succeeded: {
    tone: 'success',
    title: 'Automatically rolled back',
    detail: 'The last known-good release was redeployed in place of this one.',
    linkLabel: 'View the restored run',
  },
  failed: {
    tone: 'failed',
    title: 'Automatic rollback FAILED — manual intervention required',
    detail:
      'This release is unhealthy and the rollback did not restore the previous one. ' +
      'Nothing is protecting production right now.',
    linkLabel: 'View the failed rollback run',
  },
  skipped: {
    tone: 'warning',
    title: 'Automatic rollback skipped — the bad release is still live',
    detail: 'A rollback policy is configured, but a guard refused to run it.',
    linkLabel: 'View the rollback run',
  },
};

function OutcomeIcon({ outcome }: { outcome: RollbackOutcome }) {
  if (outcome === 'succeeded') return <ShieldCheck size={16} />;
  if (outcome === 'failed') return <TriangleAlert size={16} />;
  return <ShieldAlert size={16} />;
}

/**
 * Post-deploy health-check failure and what auto-rollback did about it.
 * Renders nothing unless the backend flagged a `health_failure_stage`, which it
 * sets only when a stage's commands passed but its health check did not.
 */
function RollbackNotice({ run, linkState }: Props) {
  const stage = run.health_failure_stage;
  if (!stage) return null;

  const outcome = run.rollback_outcome;
  const copy = outcome ? OUTCOME_COPY[outcome] : null;

  return (
    <div className={`rollback-notice rollback-notice-${copy?.tone ?? 'plain'}`}>
      <div className="rollback-notice-header">
        <HeartCrack size={16} />
        <span>Health check failed on stage &quot;{stage}&quot;</span>
      </div>
      <p className="rollback-notice-sub">
        The stage&apos;s commands succeeded, but the post-deploy health check did not pass.
      </p>

      {copy && outcome && (
        <div className="rollback-notice-outcome">
          <strong className="rollback-notice-title">
            <OutcomeIcon outcome={outcome} /> {copy.title}
          </strong>
          <p className="rollback-notice-detail">{copy.detail}</p>
          {outcome === 'skipped' && run.rollback_skip_reason && (
            <p className="rollback-notice-reason">Reason: {run.rollback_skip_reason}</p>
          )}
          {run.rollback_run_id && (
            <Link to={`/run/${run.rollback_run_id}`} state={linkState} className="text-link">
              {copy.linkLabel}
            </Link>
          )}
        </div>
      )}
    </div>
  );
}

export default RollbackNotice;
