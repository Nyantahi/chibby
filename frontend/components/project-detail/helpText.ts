/** Brief per-section help copy shown in HelpTip popovers across the project tabs. */
export const HELP = {
  bootstrapImport:
    'Scan your repo for env/secret references, or pull config from Vercel, Railway, Fly, or a .env file — a fast way to seed environments and secrets.',
  environments:
    'Named deploy targets (e.g. staging, production) with their SSH host/port. The pipeline uses these to know where and how to deploy.',
  secrets:
    'References to sensitive values (API keys, tokens). Values live in your OS keychain — never in config files — and are injected at run time per environment.',
  version: "Read and bump your project's version, and manage tags and changelog for a release.",
  artifactsSigning:
    'Configure the build artifacts a release produces and optionally sign them so users can verify authenticity.',
  updater: 'Set up auto-update feeds so shipped apps can download and install new versions.',
  notifications:
    'Send release notifications (e.g. Slack or webhook) when a pipeline or deploy completes.',
  gates:
    'Security and quality gates that must pass before shipping — secret scanning, dependency, SAST, and container scans. Block or warn on findings.',
  cleanup:
    'Prune old build artifacts, caches, and deploy leftovers. Run a dry run first to preview what would be removed.',
  triggers:
    'Local triggers that run this pipeline without a click: cron schedules, file watches, and git hooks. Config lives in .chibby/triggers.toml, with a gitignored triggers.local.toml for per-machine overrides.',
  deploymentHistory:
    'A log of past deployments per environment — what shipped, when, and its result.',
} as const;
