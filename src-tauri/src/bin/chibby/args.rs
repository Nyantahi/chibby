//! CLI argument definitions (clap).
//!
//! Extracted from the CLI entrypoint to keep the binary root thin.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "chibby",
    about = "Local-first CI/CD for solo developers",
    version,
    after_help = "Run 'chibby <command> --help' for more information on a command.",
    styles = get_styles()
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Disable colors and emoji (also honors any non-empty NO_COLOR env var)
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Output as JSON (for scripting)
    #[arg(long, global = true)]
    pub json: bool,
}

fn get_styles() -> clap::builder::Styles {
    clap::builder::Styles::styled()
        .usage(clap::builder::styling::AnsiColor::Cyan.on_default().bold())
        .header(clap::builder::styling::AnsiColor::Cyan.on_default().bold())
        .literal(clap::builder::styling::AnsiColor::Green.on_default())
        .placeholder(clap::builder::styling::AnsiColor::BrightBlack.on_default())
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run the pipeline for the current project
    Run {
        /// Environment to deploy to (e.g., staging, production)
        #[arg(short, long)]
        env: Option<String>,

        /// Only run specific stages
        #[arg(short, long)]
        stage: Vec<String>,

        /// Project path (defaults to current directory)
        #[arg(short, long)]
        project: Option<PathBuf>,

        /// Skip preflight checks
        #[arg(long)]
        skip_preflight: bool,

        /// Dry run - show what would be executed without running
        #[arg(long)]
        dry_run: bool,

        /// Pipeline file stem to run (defaults to `pipeline`)
        #[arg(long)]
        pipeline: Option<String>,

        /// Tags the run with the trigger that started it
        /// (`hook:pre-push`, `scheduled:<id>`, `watch:<id>`).
        #[arg(long, hide = true)]
        trigger: Option<String>,
    },

    /// Show status of the current or last run
    Status {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },

    /// Cancel a running pipeline
    Cancel {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },

    /// Manage projects
    #[command(subcommand)]
    Projects(ProjectsCmd),

    /// Manage pipelines
    #[command(subcommand)]
    Pipeline(PipelineCmd),

    /// View run history
    History {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,

        /// Filter by environment
        #[arg(short, long)]
        env: Option<String>,

        /// Number of runs to show
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },

    /// Retry a failed run
    Retry {
        /// Run ID to retry
        run_id: String,

        /// Start from a specific stage
        #[arg(long)]
        from_stage: Option<String>,
    },

    /// Rollback to a previous successful run
    Rollback {
        /// Run ID to rollback to (optional with --last-good)
        run_id: Option<String>,

        /// Roll back to the last known-good deployment instead of a run ID
        #[arg(long)]
        last_good: bool,

        /// Environment to look up the last known-good deployment in
        #[arg(short, long)]
        env: Option<String>,

        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },

    /// Manage environment variables and secrets
    #[command(subcommand)]
    Secrets(SecretsCmd),

    /// Manage environments
    #[command(subcommand)]
    Env(EnvCmd),

    /// Version management
    #[command(subcommand)]
    Version(VersionCmd),

    /// Artifact management
    #[command(subcommand)]
    Artifact(ArtifactCmd),

    /// Security and quality scans
    #[command(subcommand)]
    Scan(ScanCmd),

    /// Run preflight checks
    Preflight {
        /// Environment to check
        #[arg(short, long)]
        env: Option<String>,

        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },

    /// Diagnose env/secret/SSH/CLI-tool health for a project
    Doctor {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },

    /// Inspect per-secret audit history (last set/delete + provenance)
    #[command(subcommand)]
    Audit(AuditCmd),

    /// Tauri updater commands
    #[command(subcommand)]
    Updater(UpdaterCmd),

    /// Initialize a new project
    Init {
        /// Path to initialize
        path: Option<PathBuf>,

        /// Use AI to generate smarter pipeline
        #[arg(long)]
        ai: bool,
    },

    /// Scan a project for env/secret references and bootstrap configs
    Bootstrap {
        /// Project path (defaults to current directory)
        #[arg(short, long)]
        project: Option<PathBuf>,

        /// Apply without printing the review table
        #[arg(long)]
        silent: bool,

        /// Show what would be written without touching the filesystem
        #[arg(long)]
        dry_run: bool,

        /// Merge with existing configs (default refuses if either file exists)
        #[arg(long)]
        merge: bool,
    },

    /// Import env/secrets from .env, Vercel, Railway, or Fly.io
    #[command(subcommand)]
    Import(ImportCmd),

    /// Export environment + secret values to a .env file
    #[command(subcommand)]
    Export(ExportCmd),

    /// Stream logs from a run
    Logs {
        /// Run ID
        run_id: String,

        /// Follow log output
        #[arg(short, long)]
        follow: bool,
    },

    /// Run scheduled triggers (cron) for a project
    Schedule {
        /// Project path (defaults to current directory)
        #[arg(short, long)]
        project: Option<PathBuf>,

        /// Fire whatever is due and exit — wire this into launchd or systemd
        #[arg(long)]
        once: bool,

        /// Print the next fire times without running anything
        #[arg(long)]
        dry_run: bool,

        /// How many fire times --dry-run prints per schedule
        #[arg(long, default_value = "5")]
        count: usize,
    },

    /// Watch files and run the pipeline when they change
    Watch {
        /// Project path (defaults to current directory)
        #[arg(short, long)]
        project: Option<PathBuf>,

        /// Only run specific stages
        #[arg(short, long)]
        stage: Vec<String>,

        /// Environment to run in
        #[arg(short, long)]
        env: Option<String>,

        /// Glob of files to watch (repeatable; omit to watch everything)
        #[arg(long)]
        include: Vec<String>,

        /// Quiet period before a burst of changes fires a run
        #[arg(long, default_value = "750")]
        debounce_ms: u64,

        /// Minimum seconds between two runs of this watch
        #[arg(long, default_value = "10")]
        min_interval_secs: u64,
    },

    /// Manage Chibby-managed git hooks
    #[command(subcommand)]
    Hooks(HooksCmd),

    /// Inspect and toggle local triggers
    #[command(subcommand)]
    Triggers(TriggersCmd),

    /// Open the desktop app
    App,
}

/// Which git hook to act on.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum HookKindArg {
    PrePush,
    PreCommit,
}

#[derive(Subcommand)]
pub enum HooksCmd {
    /// Install a Chibby-managed git hook
    Install {
        /// Which hook to install
        #[arg(value_enum, default_value = "pre-push")]
        kind: HookKindArg,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
        /// Stages the hook runs (omit for the whole pipeline)
        #[arg(short, long)]
        stage: Vec<String>,
        /// Environment the hook runs in
        #[arg(short, long)]
        env: Option<String>,
        /// Back up an existing foreign hook and replace it
        #[arg(long)]
        force: bool,
        /// Insert Chibby's block into an existing hook, keeping the rest
        #[arg(long)]
        append: bool,
        /// Report failures without blocking the git operation
        #[arg(long)]
        non_blocking: bool,
    },
    /// Remove Chibby's block from a git hook
    Uninstall {
        #[arg(value_enum, default_value = "pre-push")]
        kind: HookKindArg,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Show what is installed at each hook path
    Status {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum TriggersCmd {
    /// List configured triggers and their last run
    List {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Enable a trigger (written to triggers.local.toml)
    Enable {
        /// Trigger id
        id: String,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Disable a trigger (written to triggers.local.toml)
    Disable {
        /// Trigger id
        id: String,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Show the next fire times for every schedule
    Next {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
        /// How many fire times to print per schedule
        #[arg(long, default_value = "5")]
        count: usize,
    },
}

#[derive(Subcommand)]
pub enum ProjectsCmd {
    /// List all projects
    List,
    /// Add a project
    Add {
        /// Path to the project
        path: PathBuf,
        /// Project name (defaults to directory name)
        #[arg(short, long)]
        name: Option<String>,
    },
    /// Remove a project
    Remove {
        /// Project ID or path
        project: String,
    },
    /// Show project info
    Info {
        /// Project ID or path
        project: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum PipelineCmd {
    /// Generate a pipeline from detected scripts
    Generate {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
        /// Use AI to generate smarter pipeline
        #[arg(long)]
        ai: bool,
    },
    /// Validate pipeline configuration
    Validate {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Show pipeline stages
    Show {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Edit pipeline in $EDITOR
    Edit {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum AuditCmd {
    /// List every secret with its set/delete history
    List {
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Show the audit snapshot for a single secret
    Show {
        /// Secret name
        name: String,
        /// Environment
        #[arg(short, long)]
        env: String,
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum ImportCmd {
    /// Import names (and optionally values) from a .env file
    Dotenv {
        /// Path to the .env file
        path: PathBuf,
        /// Target environment to merge into
        #[arg(short, long, default_value = "production")]
        env: String,
        /// Also pull values (variables -> environments.toml, secrets -> keychain)
        #[arg(long)]
        with_values: bool,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Import env vars from Vercel via `vercel env`
    Vercel {
        /// Target environment ("production" maps to Vercel's `production`)
        #[arg(short, long, default_value = "production")]
        env: String,
        /// Pull values via `vercel env pull` (otherwise names only)
        #[arg(long)]
        with_values: bool,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Import env vars from Railway via `railway variables`
    Railway {
        #[arg(short, long, default_value = "production")]
        env: String,
        #[arg(long)]
        with_values: bool,
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Import secret names from Fly.io via `flyctl secrets list` (names only — Fly is write-only)
    Fly {
        #[arg(short, long, default_value = "production")]
        env: String,
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum ExportCmd {
    /// Export resolved variables + secret values to a .env file
    Dotenv {
        /// Source environment
        #[arg(short, long, default_value = "production")]
        env: String,
        /// Output path (parent dirs created if needed)
        #[arg(short, long, default_value = ".env.chibby")]
        out: PathBuf,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum SecretsCmd {
    /// List declared secret references (from .chibby/secrets.toml)
    List {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Add a new secret reference to .chibby/secrets.toml
    Add {
        /// Secret name
        name: String,
        /// Environment(s) this secret applies to (repeatable; omit = all)
        #[arg(short, long)]
        env: Vec<String>,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Remove a secret reference from .chibby/secrets.toml
    Remove {
        /// Secret name
        name: String,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Set a secret value in the OS keychain (prompts if --value omitted)
    Set {
        /// Secret name
        name: String,
        /// Environment name (required — secrets are scoped per-env)
        #[arg(short, long)]
        env: String,
        /// Secret value (omit to prompt securely)
        #[arg(long)]
        value: Option<String>,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Overwrite an existing secret value (alias for `set`)
    Rotate {
        /// Secret name
        name: String,
        /// Environment name
        #[arg(short, long)]
        env: String,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Delete a secret value from the OS keychain
    Delete {
        /// Secret name
        name: String,
        /// Environment name (required)
        #[arg(short, long)]
        env: String,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Show which declared secrets are set in the keychain
    Status {
        /// Environment to check (omit = all declared envs)
        #[arg(short, long)]
        env: Option<String>,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum EnvCmd {
    /// List environments
    List {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Show environment details (resolved with environments.local.toml overrides)
    Show {
        /// Environment name
        name: String,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Add a new environment to .chibby/environments.toml
    Add {
        /// Environment name
        name: String,
        /// SSH host (user@host) for ssh-backed stages
        #[arg(long)]
        ssh_host: Option<String>,
        /// SSH port (default 22)
        #[arg(long)]
        ssh_port: Option<u16>,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Remove an environment from .chibby/environments.toml
    Remove {
        /// Environment name
        name: String,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Open .chibby/environments.toml in $EDITOR
    Edit {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Duplicate an environment under a new name
    Copy {
        /// Source environment
        from: String,
        /// Destination environment
        to: String,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Test SSH connectivity for an environment
    Test {
        /// Environment name
        name: String,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Manage environment variables (non-secret config) per environment
    #[command(subcommand)]
    Vars(EnvVarsCmd),
    /// Compare two environments side by side
    Diff {
        /// Source environment
        from: String,
        /// Destination environment
        to: String,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Scan environments.toml for variable values that look like real credentials
    ScanLeaks {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum EnvVarsCmd {
    /// List variables for an environment (merged with environments.local.toml)
    List {
        /// Environment name
        env: String,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Set a variable on an environment
    Set {
        /// Environment name
        env: String,
        /// Variable name (must match [A-Za-z_][A-Za-z0-9_]*)
        key: String,
        /// Variable value
        value: String,
        /// Write to environments.local.toml (per-dev override) instead of environments.toml
        #[arg(long)]
        local: bool,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Get a single variable value
    Get {
        /// Environment name
        env: String,
        /// Variable name
        key: String,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Delete a variable from an environment
    Delete {
        /// Environment name
        env: String,
        /// Variable name
        key: String,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum VersionCmd {
    /// Show current version
    Show {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Bump version
    Bump {
        /// Bump level: patch, minor, major, or explicit version
        level: String,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
        /// Create git tag
        #[arg(long)]
        tag: bool,
        /// Generate changelog
        #[arg(long)]
        changelog: bool,
    },
}

#[derive(Subcommand)]
pub enum ArtifactCmd {
    /// List artifacts
    List {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Collect artifacts from last build
    Collect {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Clean old artifacts
    Clean {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
        /// Preview without deleting
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
pub enum ScanCmd {
    /// Scan for leaked secrets
    Secrets {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
        /// Create baseline from current findings
        #[arg(long)]
        baseline: bool,
    },
    /// Scan dependencies for vulnerabilities
    Deps {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
        /// Minimum severity: low, medium, high, critical
        #[arg(long, default_value = "high")]
        severity: String,
    },
    /// Lint commit messages
    Commits {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
        /// Check commits since this tag/ref
        #[arg(long)]
        since: Option<String>,
    },
    /// Static analysis (semgrep)
    Sast {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Container image scan (trivy image)
    Container {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Infrastructure-as-Code scan (trivy config)
    Iac {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// License compliance check
    License {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum UpdaterCmd {
    /// Generate Tauri update signing keys
    GenerateKeys {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Sign an update bundle
    Sign {
        /// Bundle path
        bundle: PathBuf,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Generate latest.json
    LatestJson {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Publish update
    Publish {
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
        /// Preview without publishing
        #[arg(long)]
        dry_run: bool,
    },
}
