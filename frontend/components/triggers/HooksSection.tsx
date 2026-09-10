import { GitFork } from 'lucide-react';
import type { HookSpec, HooksConfig } from '../../types';
import HookCard from './HookCard';
import { HOOK_KINDS } from './helpers';

interface HooksSectionProps {
  repoPath: string;
  hooks: HooksConfig;
  environments: string[];
  onChange: (hooks: HooksConfig) => void;
}

function HooksSection({ repoPath, hooks, environments, onChange }: HooksSectionProps) {
  function handleSpecChange(kind: keyof HooksConfig, spec: HookSpec | undefined) {
    onChange({ ...hooks, [kind]: spec });
  }

  return (
    <section className="section">
      <div className="section-header-row">
        <h3 className="section-title">
          <GitFork size={16} />
          Git hooks
        </h3>
      </div>

      <p className="section-hint">
        Unlike schedules and watches, hooks need nothing running in the background — git invokes
        them itself. Chibby writes a clearly marked block into the hook file and never touches
        anything outside it.
      </p>

      <div className="trigger-list">
        {HOOK_KINDS.map((kind) => (
          <HookCard
            key={kind}
            repoPath={repoPath}
            kind={kind}
            spec={hooks[kind]}
            environments={environments}
            onSpecChange={(spec) => handleSpecChange(kind, spec)}
          />
        ))}
      </div>
    </section>
  );
}

export default HooksSection;
