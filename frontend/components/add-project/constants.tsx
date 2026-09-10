import { Wand2, Container, Package, Tag, SkipForward, Server, Plane, Cloud } from 'lucide-react';
import type { DetectedScript, Stage, DeploymentMethod } from '../../types';

export type WizardStep = 'select' | 'source' | 'configure' | 'deploy' | 'review' | 'done';
export type PipelineSource = 'auto' | 'github' | 'template';

export const WIZARD_STEPS: { key: WizardStep; label: string }[] = [
  { key: 'select', label: 'Select' },
  { key: 'source', label: 'Source' },
  { key: 'configure', label: 'CI Stages' },
  { key: 'deploy', label: 'Deploy' },
  { key: 'review', label: 'Review' },
];

// Deployment method display information
export interface DeployMethodDisplay {
  method: DeploymentMethod;
  label: string;
  description: string;
  icon: React.ReactNode;
  requiresSshHost: boolean;
  requiresRegistry: boolean;
  requiresHealthCheck: boolean;
  requiresPlatformProject: boolean;
}

export const DEPLOY_METHOD_INFO: DeployMethodDisplay[] = [
  {
    method: 'auto_detect',
    label: 'Auto-detect',
    description: 'Use GitHub Actions deploy workflow',
    icon: <Wand2 size={20} />,
    requiresSshHost: false,
    requiresRegistry: false,
    requiresHealthCheck: false,
    requiresPlatformProject: false,
  },
  {
    method: 'docker_compose_ssh',
    label: 'Docker Compose SSH',
    description: 'Deploy with docker compose over SSH',
    icon: <Container size={20} />,
    requiresSshHost: true,
    requiresRegistry: false,
    requiresHealthCheck: true,
    requiresPlatformProject: false,
  },
  {
    method: 'docker_registry',
    label: 'Docker Registry',
    description: 'Push to registry, pull on server',
    icon: <Container size={20} />,
    requiresSshHost: true,
    requiresRegistry: true,
    requiresHealthCheck: false,
    requiresPlatformProject: false,
  },
  {
    method: 'cargo_publish',
    label: 'Cargo Publish',
    description: 'Publish to crates.io',
    icon: <Package size={20} />,
    requiresSshHost: false,
    requiresRegistry: false,
    requiresHealthCheck: false,
    requiresPlatformProject: false,
  },
  {
    method: 'npm_publish',
    label: 'npm Publish',
    description: 'Publish to npm registry',
    icon: <Package size={20} />,
    requiresSshHost: false,
    requiresRegistry: false,
    requiresHealthCheck: false,
    requiresPlatformProject: false,
  },
  {
    method: 'github_release',
    label: 'GitHub Release',
    description: 'Create release with binaries',
    icon: <Tag size={20} />,
    requiresSshHost: false,
    requiresRegistry: false,
    requiresHealthCheck: false,
    requiresPlatformProject: false,
  },
  {
    method: 'ssh_rsync',
    label: 'SSH + rsync',
    description: 'Sync files to server via rsync',
    icon: <Server size={20} />,
    requiresSshHost: true,
    requiresRegistry: false,
    requiresHealthCheck: false,
    requiresPlatformProject: false,
  },
  {
    method: 'flyio',
    label: 'Fly.io',
    description: 'Deploy to Fly.io',
    icon: <Plane size={20} />,
    requiresSshHost: false,
    requiresRegistry: false,
    requiresHealthCheck: true,
    requiresPlatformProject: true,
  },
  {
    method: 'render',
    label: 'Render',
    description: 'Deploy to Render',
    icon: <Cloud size={20} />,
    requiresSshHost: false,
    requiresRegistry: false,
    requiresHealthCheck: false,
    requiresPlatformProject: false,
  },
  {
    method: 'railway',
    label: 'Railway',
    description: 'Deploy to Railway',
    icon: <Cloud size={20} />,
    requiresSshHost: false,
    requiresRegistry: false,
    requiresHealthCheck: false,
    requiresPlatformProject: false,
  },
  {
    method: 'netlify',
    label: 'Netlify',
    description: 'Deploy to Netlify',
    icon: <Cloud size={20} />,
    requiresSshHost: false,
    requiresRegistry: false,
    requiresHealthCheck: false,
    requiresPlatformProject: false,
  },
  {
    method: 'vercel',
    label: 'Vercel',
    description: 'Deploy to Vercel',
    icon: <Cloud size={20} />,
    requiresSshHost: false,
    requiresRegistry: false,
    requiresHealthCheck: false,
    requiresPlatformProject: false,
  },
  {
    method: 's3_static',
    label: 'S3 Static',
    description: 'Deploy to AWS S3',
    icon: <Cloud size={20} />,
    requiresSshHost: false,
    requiresRegistry: false,
    requiresHealthCheck: false,
    requiresPlatformProject: true,
  },
  {
    method: 'skip',
    label: 'Skip',
    description: 'CI only, no deployment',
    icon: <SkipForward size={20} />,
    requiresSshHost: false,
    requiresRegistry: false,
    requiresHealthCheck: false,
    requiresPlatformProject: false,
  },
];

// Missing-stage suggestion rules: detected file -> keyword to look for in commands -> suggested stage
const SUGGESTION_RULES: {
  filePattern: string;
  keywords: string[];
  stages: { name: string; commands: string[] }[];
}[] = [
  {
    filePattern: 'Makefile',
    keywords: ['make'],
    stages: [
      { name: 'make-build', commands: ['make build'] },
      { name: 'make-test', commands: ['make test'] },
    ],
  },
  {
    filePattern: 'deploy.sh',
    keywords: ['deploy.sh'],
    stages: [{ name: 'deploy', commands: ['./deploy.sh'] }],
  },
  {
    filePattern: 'Dockerfile',
    keywords: ['docker'],
    stages: [{ name: 'docker-build', commands: ['docker build .'] }],
  },
  {
    filePattern: 'docker-compose',
    keywords: ['docker compose', 'docker-compose'],
    stages: [{ name: 'docker-compose', commands: ['docker compose up -d'] }],
  },
  {
    filePattern: 'Cargo.toml',
    keywords: ['cargo'],
    stages: [
      { name: 'cargo-build', commands: ['cargo build'] },
      { name: 'cargo-test', commands: ['cargo test'] },
    ],
  },
  {
    filePattern: 'go.mod',
    keywords: ['go build', 'go test'],
    stages: [
      { name: 'go-build', commands: ['go build ./...'] },
      { name: 'go-test', commands: ['go test ./...'] },
    ],
  },
  {
    filePattern: 'pyproject.toml',
    keywords: ['pip', 'pytest', 'python'],
    stages: [{ name: 'python-test', commands: ['pip install -e .', 'pytest'] }],
  },
  {
    filePattern: 'setup.py',
    keywords: ['pip', 'pytest', 'python'],
    stages: [{ name: 'python-test', commands: ['pip install -e .', 'pytest'] }],
  },
];

export function computeSuggestions(
  scripts: DetectedScript[],
  stages: Stage[]
): { name: string; commands: string[]; reason: string }[] {
  const allCommands = stages
    .flatMap((s) => s.commands)
    .join(' ')
    .toLowerCase();

  const suggestions: { name: string; commands: string[]; reason: string }[] = [];

  for (const rule of SUGGESTION_RULES) {
    const hasFile = scripts.some((s) =>
      s.file_name.toLowerCase().includes(rule.filePattern.toLowerCase())
    );
    if (!hasFile) continue;

    const covered = rule.keywords.some((kw) => allCommands.includes(kw.toLowerCase()));
    if (covered) continue;

    for (const stage of rule.stages) {
      suggestions.push({
        ...stage,
        reason: `${rule.filePattern} detected`,
      });
    }
  }

  return suggestions;
}
