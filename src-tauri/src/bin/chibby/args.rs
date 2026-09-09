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

    /// Disable colors and emoji
    #[arg(long, global = true, env = "NO_COLOR")]
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
        /// Run ID to rollback to
        run_id: String,
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

    /// Open the desktop app
    App,
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
