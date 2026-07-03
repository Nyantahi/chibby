import type { PipelineRun } from '../../types';

export type TabId = 'pipeline' | 'history' | 'environments' | 'release' | 'quality';

/** Determine whether a run was full pipeline or partial (single/few stages). */
export function runScopeLabel(run: PipelineRun): { label: string; isPartial: boolean } {
  const executed = run.stage_results.filter((s) => s.status !== 'skipped');
  const total = run.stage_results.length;
  if (total === 0 || executed.length === total) {
    return { label: run.pipeline_name, isPartial: false };
  }
  if (executed.length === 1) {
    return { label: executed[0].stage_name, isPartial: true };
  }
  return { label: `${executed.length} of ${total} stages`, isPartial: true };
}

export function formatScriptType(scriptType: string): string {
  switch (scriptType) {
    // Node / JS
    case 'PackageJson':
      return 'npm';
    case 'Turborepo':
      return 'turbo';
    case 'Nx':
      return 'nx';
    case 'Deno':
      return 'deno';
    case 'Grunt':
      return 'grunt';
    case 'Gulp':
      return 'gulp';
    // Rust
    case 'CargoToml':
      return 'cargo';
    // Go
    case 'GoMod':
      return 'go';
    // Python
    case 'PythonProject':
      return 'python';
    case 'PythonRequirements':
      return 'pip';
    case 'Tox':
      return 'tox';
    case 'Pytest':
      return 'pytest';
    case 'PythonTestDir':
      return 'tests';
    // Ruby
    case 'Gemfile':
      return 'ruby';
    case 'Rakefile':
      return 'rake';
    // Java
    case 'Maven':
      return 'maven';
    case 'Gradle':
      return 'gradle';
    // .NET
    case 'DotNet':
      return '.net';
    // PHP
    case 'Composer':
      return 'php';
    // C / C++
    case 'CMake':
      return 'cmake';
    case 'Meson':
      return 'meson';
    // Make / tasks
    case 'Makefile':
      return 'make';
    case 'Justfile':
      return 'just';
    case 'Taskfile':
      return 'task';
    // Containers
    case 'Dockerfile':
      return 'docker';
    case 'DockerCompose':
      return 'compose';
    case 'Skaffold':
      return 'skaffold';
    case 'Vagrantfile':
      return 'vagrant';
    // Shell / env
    case 'ShellScript':
      return 'shell';
    case 'EnvFile':
      return 'env';
    case 'Procfile':
      return 'proc';
    // CI platforms
    case 'GithubActions':
      return 'github';
    case 'GitlabCi':
      return 'gitlab';
    case 'Jenkinsfile':
      return 'jenkins';
    case 'TravisCi':
      return 'travis';
    case 'DroneCi':
      return 'drone';
    case 'CircleCi':
      return 'circleci';
    case 'AzurePipelines':
      return 'azure';
    case 'BitbucketPipelines':
      return 'bitbucket';
    // Deploy / infra
    case 'Netlify':
      return 'netlify';
    case 'Vercel':
      return 'vercel';
    // Quality
    case 'PreCommit':
      return 'hooks';
    default:
      return scriptType.toLowerCase();
  }
}

export function formatPreflightError(err: { type: string; detail: unknown }): string {
  switch (err.type) {
    case 'MissingSecret': {
      const d = err.detail as { name: string; environment: string };
      return `Missing secret "${d.name}" for environment "${d.environment}"`;
    }
    case 'MissingSshHost': {
      const d = err.detail as { stage: string };
      return `Stage "${d.stage}" uses SSH but no host is configured`;
    }
    case 'MissingEnvironment': {
      const d = err.detail as { name: string };
      return `Environment "${d.name}" not found`;
    }
    case 'SshConnectivityFailed': {
      const d = err.detail as { host: string; error: string };
      return `SSH to ${d.host} failed: ${d.error}`;
    }
    case 'SshNotAvailable':
      return 'SSH client not found on PATH';
    default:
      return JSON.stringify(err);
  }
}
