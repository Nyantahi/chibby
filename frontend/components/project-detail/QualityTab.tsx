import type { Environment } from '../../types';
import GatesCard from '../GatesCard';
import CleanupCard from '../CleanupCard';
import DeploymentHistoryCard from '../DeploymentHistoryCard';

interface QualityTabProps {
  repoPath: string;
  environments: Environment[];
}

function QualityTab({ repoPath, environments }: QualityTabProps) {
  return (
    <div className="cards-stack">
      <GatesCard repoPath={repoPath} />
      <CleanupCard repoPath={repoPath} />
      <DeploymentHistoryCard repoPath={repoPath} environments={environments} />
    </div>
  );
}

export default QualityTab;
