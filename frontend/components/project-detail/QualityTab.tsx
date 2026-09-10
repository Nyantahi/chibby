import type { Environment, PipelineRun } from '../../types';
import GatesCard from '../GatesCard';
import CleanupCard from '../CleanupCard';
import DeploymentHistoryCard from '../DeploymentHistoryCard';

interface QualityTabProps {
  repoPath: string;
  environments: Environment[];
  runs: PipelineRun[];
}

function QualityTab({ repoPath, environments, runs }: QualityTabProps) {
  return (
    <div className="cards-stack">
      <GatesCard repoPath={repoPath} />
      <CleanupCard repoPath={repoPath} />
      <DeploymentHistoryCard repoPath={repoPath} environments={environments} runs={runs} />
    </div>
  );
}

export default QualityTab;
