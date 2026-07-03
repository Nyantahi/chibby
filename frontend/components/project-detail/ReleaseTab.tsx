import VersionCard from '../VersionCard';
import ArtifactsCard from '../ArtifactsCard';
import UpdaterCard from '../UpdaterCard';
import NotifyCard from '../NotifyCard';

interface ReleaseTabProps {
  repoPath: string;
}

function ReleaseTab({ repoPath }: ReleaseTabProps) {
  return (
    <div className="cards-stack">
      <VersionCard repoPath={repoPath} />
      <ArtifactsCard repoPath={repoPath} />
      <UpdaterCard repoPath={repoPath} />
      <NotifyCard repoPath={repoPath} />
    </div>
  );
}

export default ReleaseTab;
