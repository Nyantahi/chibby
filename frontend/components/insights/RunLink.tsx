import { Link } from 'react-router-dom';
import { NO_VALUE, shortRunId } from '../../utils/insights';

interface RunLinkProps {
  runId: string | null;
}

/**
 * Link to a run referenced by the index. Run summaries outlive their logs, so
 * the target may be a run whose record was pruned — the title says so, and
 * RunDetail renders a "no longer available" state rather than an error.
 */
function RunLink({ runId }: RunLinkProps) {
  if (!runId) return <span className="text-muted">{NO_VALUE}</span>;

  return (
    <Link
      to={`/run/${runId}`}
      className="insights-run-link"
      title="Open this run. Older runs keep their summary after their logs are pruned."
    >
      {shortRunId(runId)}
    </Link>
  );
}

export default RunLink;
