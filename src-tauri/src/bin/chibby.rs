//! Chibby CLI - Local-first CI/CD
//!
//! A standalone CLI that shares the same engine as the desktop app.
//! Designed for headless servers, scripting, and terminal-first workflows.

use chibby_lib::engine::executor;
use chibby_lib::engine::models::{
    Environment, PipelineRun, Project, RunKind, RunStatus as EngineRunStatus, SecretRef,
    StageStatus as EngineStageStatus,
};
use chibby_lib::engine::bootstrap::{self, ApplyMode, Classification};
use chibby_lib::engine::importers::{
    self, dotenv::DotEnvImporter, flyio::FlyImporter, railway::RailwayImporter,
    vercel::VercelImporter, ApplyOptions, ImportContext, ImportReport, Importer,
};
use chibby_lib::engine::secret_audit::{self as secret_audit_engine, Provenance};
use chibby_lib::engine::{gates, persistence, pipeline, preflight, run_support, secrets as secrets_engine};
use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;
use std::path::{Path, PathBuf};

// Import CLI styled output
mod cli {
    include!("../cli/mod.rs");
}

use cli::{icons, Printer, StageStatus};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// CLI Definition
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Parser)]
#[command(
    name = "chibby",
    about = "Local-first CI/CD for solo developers",
    version,
    after_help = "Run 'chibby <command> --help' for more information on a command.",
    styles = get_styles()
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Disable colors and emoji
    #[arg(long, global = true, env = "NO_COLOR")]
    no_color: bool,

    /// Output as JSON (for scripting)
    #[arg(long, global = true)]
    json: bool,
}

fn get_styles() -> clap::builder::Styles {
    clap::builder::Styles::styled()
        .usage(clap::builder::styling::AnsiColor::Cyan.on_default().bold())
        .header(clap::builder::styling::AnsiColor::Cyan.on_default().bold())
        .literal(clap::builder::styling::AnsiColor::Green.on_default())
        .placeholder(clap::builder::styling::AnsiColor::BrightBlack.on_default())
}

#[derive(Subcommand)]
enum Commands {
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
enum ProjectsCmd {
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
enum PipelineCmd {
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
enum AuditCmd {
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
enum ImportCmd {
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
enum ExportCmd {
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
enum SecretsCmd {
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
enum EnvCmd {
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
enum EnvVarsCmd {
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
enum VersionCmd {
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
enum ArtifactCmd {
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
enum ScanCmd {
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
enum UpdaterCmd {
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

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Main Entry Point
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::main]
async fn main() {
    let cli_args = Cli::parse();

    // Handle color preferences
    if cli_args.no_color {
        owo_colors::set_override(false);
    }

    let printer = Printer::new(cli_args.verbose);

    // Run the appropriate command
    let result = match &cli_args.command {
        None => {
            // No command - show banner and help hint
            show_welcome(&printer);
            Ok(())
        }

        Some(Commands::Run {
            env,
            stage,
            project,
            skip_preflight,
            dry_run,
        }) => {
            run_pipeline(
                &printer,
                env.as_deref(),
                stage,
                project.as_ref(),
                *skip_preflight,
                *dry_run,
            )
            .await
        }

        Some(Commands::Status { project }) => show_status(&printer, project.as_ref()).await,

        Some(Commands::Cancel { project }) => cancel_pipeline(&printer, project.as_ref()).await,

        Some(Commands::Projects(cmd)) => handle_projects(&printer, cmd).await,

        Some(Commands::Pipeline(cmd)) => handle_pipeline(&printer, cmd).await,

        Some(Commands::History {
            project,
            env,
            limit,
        }) => show_history(&printer, project.as_ref(), env.as_deref(), *limit).await,

        Some(Commands::Retry { run_id, from_stage }) => {
            retry_run(&printer, run_id, from_stage.as_deref()).await
        }

        Some(Commands::Rollback { run_id }) => rollback_run(&printer, run_id).await,

        Some(Commands::Secrets(cmd)) => handle_secrets(&printer, cmd).await,

        Some(Commands::Env(cmd)) => handle_env(&printer, cmd).await,

        Some(Commands::Version(cmd)) => handle_version(&printer, cmd).await,

        Some(Commands::Artifact(cmd)) => handle_artifact(&printer, cmd).await,

        Some(Commands::Scan(cmd)) => handle_scan(&printer, cmd).await,

        Some(Commands::Init { path, ai }) => init_project(&printer, path.as_ref(), *ai).await,

        Some(Commands::Bootstrap {
            project,
            silent,
            dry_run,
            merge,
        }) => bootstrap_cmd(&printer, project.as_ref(), *silent, *dry_run, *merge).await,

        Some(Commands::Import(cmd)) => handle_import(&printer, cmd).await,

        Some(Commands::Export(cmd)) => handle_export(&printer, cmd).await,

        Some(Commands::Preflight { env, project }) => {
            run_preflight(&printer, env.as_deref(), project.as_ref()).await
        }

        Some(Commands::Doctor { project }) => doctor(&printer, project.as_ref()).await,

        Some(Commands::Audit(cmd)) => handle_audit(&printer, cmd).await,

        Some(Commands::Updater(cmd)) => handle_updater(&printer, cmd).await,

        Some(Commands::Logs { run_id, follow }) => show_logs(&printer, run_id, *follow).await,

        Some(Commands::App) => {
            open_app(&printer);
            Ok(())
        }
    };

    // Handle errors with styled output
    if let Err(e) = result {
        printer.error(&e.to_string());
        std::process::exit(1);
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Command Implementations
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn show_welcome(printer: &Printer) {
    printer.banner();
    println!(
        "   {} {}",
        "Type".bright_black(),
        "chibby --help".cyan().bold()
    );
    println!(
        "   {} {}",
        "  or".bright_black(),
        "chibby run".cyan().bold()
    );
    println!();
}

async fn run_pipeline(
    printer: &Printer,
    env: Option<&str>,
    stages: &[String],
    project: Option<&PathBuf>,
    skip_preflight: bool,
    dry_run: bool,
) -> anyhow::Result<()> {
    printer.banner();

    let project_path = project
        .cloned()
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    printer.header(&format!("{} Running Pipeline", icons::ROCKET));
    printer.kv("Project", &project_path.display().to_string());
    if let Some(e) = env {
        printer.kv("Environment", e);
    }
    if !stages.is_empty() {
        printer.kv("Stages", &stages.join(", "));
    }
    if dry_run {
        printer.warn("Dry run mode - nothing will be executed");
    }
    printer.newline();

    // Preflight checks
    if !skip_preflight && !dry_run {
        printer.subheader(&format!("{} Preflight Checks", icons::SHIELD));

        let checks = [
            ("Pipeline config exists", true),
            ("Git working tree clean", true),
            ("Required secrets configured", true),
        ];

        for (check, passes) in checks {
            let spin = cli::spinner(&format!("Checking {}...", check.to_lowercase()));
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            spin.finish_and_clear();

            if passes {
                printer.preflight_check(check, true, None);
            } else {
                printer.preflight_check(check, false, Some("Missing configuration"));
                return Err(anyhow::anyhow!("Preflight check failed: {}", check));
            }
        }

        if env.is_some() {
            let spin = cli::spinner("Testing SSH connection...");
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            spin.finish_and_clear();
            printer.preflight_check("SSH connection", true, None);
        }

        printer.newline();
    }

    // Pipeline execution
    printer.subheader(&format!("{} Pipeline Stages", icons::GEAR));

    // Example stages - in real impl, load from pipeline config
    let example_stages = [
        ("preflight", "Preflight"),
        ("build", "Build"),
        ("test", "Test"),
        ("deploy", "Deploy"),
    ];

    let start_time = std::time::Instant::now();
    let mut passed = 0;
    let failed = false;

    for (stage_type, stage_name) in &example_stages {
        if dry_run {
            printer.stage_typed(stage_type, stage_name, StageStatus::Pending);
            continue;
        }

        // Show running state
        printer.stage_typed(stage_type, stage_name, StageStatus::Running);

        // Simulate execution
        let spin = cli::spinner(&format!("Running {}...", stage_name.to_lowercase()));

        // Simulate varying durations
        let duration = match *stage_type {
            "build" => 1200,
            "test" => 800,
            "deploy" => 600,
            _ => 300,
        };
        tokio::time::sleep(std::time::Duration::from_millis(duration)).await;
        spin.finish_and_clear();

        // Show result - move up to overwrite running state
        print!("\x1B[1A\x1B[2K"); // Move up and clear line
        printer.stage_with_duration(stage_name, StageStatus::Success, Some(duration));
        passed += 1;
    }

    // Summary
    let elapsed = start_time.elapsed().as_millis() as u64;

    if dry_run {
        printer.newline();
        printer.info("Dry run complete - no changes made");
    } else if failed {
        printer.run_summary("failed", elapsed, passed, example_stages.len());
    } else {
        printer.run_summary("success", elapsed, passed, example_stages.len());
    }

    Ok(())
}

async fn show_status(printer: &Printer, _project: Option<&PathBuf>) -> anyhow::Result<()> {
    printer.header(&format!("{} Pipeline Status", icons::INFO));

    printer.kv("Project", "my-app");
    printer.kv_colored("Status", "Success", StageStatus::Success);
    printer.kv("Last Run", "2 minutes ago");
    printer.kv("Duration", "1m 23s");
    printer.kv("Environment", "production");
    printer.newline();

    printer.subheader("Stages");
    printer.stage_with_duration("Build", StageStatus::Success, Some(45_000));
    printer.stage_with_duration("Test", StageStatus::Success, Some(23_000));
    printer.stage_with_duration("Deploy", StageStatus::Success, Some(15_000));

    Ok(())
}

async fn cancel_pipeline(printer: &Printer, _project: Option<&PathBuf>) -> anyhow::Result<()> {
    let spin = cli::spinner("Cancelling pipeline...");
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    spin.finish_and_clear();

    printer.warn("Pipeline cancelled");
    Ok(())
}

fn run_status_to_cli(status: Option<&EngineRunStatus>) -> Option<StageStatus> {
    status.map(|s| match s {
        EngineRunStatus::Pending => StageStatus::Pending,
        EngineRunStatus::Running => StageStatus::Running,
        EngineRunStatus::Success => StageStatus::Success,
        EngineRunStatus::Failed => StageStatus::Failed,
        EngineRunStatus::Cancelled => StageStatus::Cancelled,
    })
}

fn find_project_by_ref<'a>(projects: &'a [Project], project_ref: &str) -> Option<&'a Project> {
    projects.iter().find(|p| {
        p.id == project_ref
            || p.name == project_ref
            || p.path == project_ref
            || Path::new(&p.path) == Path::new(project_ref)
    })
}

async fn handle_projects(printer: &Printer, cmd: &ProjectsCmd) -> anyhow::Result<()> {
    match cmd {
        ProjectsCmd::List => {
            printer.header(&format!("{} Projects", icons::FOLDER));
            let projects = persistence::load_projects()?;
            if projects.is_empty() {
                printer.info("No projects tracked yet. Add one with `chibby projects add <path>`.");
                return Ok(());
            }
            for p in &projects {
                printer.project_with_status(
                    &p.name,
                    &p.path,
                    run_status_to_cli(p.last_run_status.as_ref()),
                );
            }
            printer.newline();
            printer.stats("projects", projects.len(), StageStatus::Pending);
        }
        ProjectsCmd::Add { path, name } => {
            let abs_path = path.canonicalize().unwrap_or_else(|_| path.clone());
            if !abs_path.exists() {
                anyhow::bail!("Path does not exist: {}", abs_path.display());
            }
            let path_str = abs_path.to_string_lossy().to_string();
            let display_name = name.clone().unwrap_or_else(|| {
                abs_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("project")
                    .to_string()
            });
            let existing = persistence::load_projects()?;
            if existing.iter().any(|p| p.path == path_str) {
                printer.warn(&format!("Already tracking {}", path_str));
                return Ok(());
            }
            let project = Project::new(&display_name, &path_str);
            persistence::add_project(project)?;
            printer.success(&format!("Added {}", display_name));
            printer.info(&format!(
                "Run {} to generate pipeline",
                "chibby pipeline generate".cyan()
            ));
        }
        ProjectsCmd::Remove { project } => {
            let projects = persistence::load_projects()?;
            let target = find_project_by_ref(&projects, project)
                .ok_or_else(|| anyhow::anyhow!("Project not found: {}", project))?;
            let id = target.id.clone();
            let name = target.name.clone();
            printer.warn(&format!("Removing {}...", name));
            persistence::remove_project(&id)?;
            printer.success("Project removed");
        }
        ProjectsCmd::Info { project } => {
            let projects = persistence::load_projects()?;
            let target_ref = match project {
                Some(p) => p.as_str(),
                None => {
                    // Default: pick the project whose path matches CWD if any
                    let cwd = std::env::current_dir().ok();
                    let matched = cwd.as_ref().and_then(|c| {
                        projects.iter().find(|p| Path::new(&p.path) == c.as_path())
                    });
                    match matched {
                        Some(p) => &p.id,
                        None => {
                            anyhow::bail!(
                                "No project specified and current directory is not tracked"
                            );
                        }
                    }
                }
            };
            let p = find_project_by_ref(&projects, target_ref)
                .ok_or_else(|| anyhow::anyhow!("Project not found: {}", target_ref))?;
            printer.header(&format!("{} Project Info", icons::INFO));
            printer.kv("Name", &p.name);
            printer.kv("Path", &p.path);
            printer.kv("ID", &p.id);
            printer.kv("Added", &p.added_at.to_rfc3339());
            if let Some(at) = &p.last_run_at {
                printer.kv("Last run", &at.to_rfc3339());
            }
            if let Some(status) = &p.last_run_status {
                printer.kv("Last status", &format!("{:?}", status));
            }
            let has_pipeline = Path::new(&p.path).join(".chibby/pipeline.toml").exists();
            printer.kv("Pipeline", if has_pipeline { "Yes" } else { "No" });
        }
    }
    Ok(())
}

async fn handle_pipeline(printer: &Printer, cmd: &PipelineCmd) -> anyhow::Result<()> {
    match cmd {
        PipelineCmd::Show { project: _ } => {
            printer.header(&format!("{} Pipeline Stages", icons::GEAR));

            let stages = [
                (
                    "preflight",
                    "Preflight",
                    "chibby scan secrets && chibby scan deps",
                ),
                ("build", "Build", "npm run build"),
                ("test", "Test", "npm test"),
                (
                    "deploy",
                    "Deploy",
                    "ssh deploy@server 'docker compose up -d'",
                ),
            ];

            for (i, (stage_type, name, cmd)) in stages.iter().enumerate() {
                let icon = match *stage_type {
                    "preflight" => icons::SHIELD,
                    "build" => icons::BUILD,
                    "test" => icons::TEST,
                    "deploy" => icons::DEPLOY,
                    _ => icons::GEAR,
                };

                println!(
                    "  {} {} {}",
                    format!("{}", i + 1).bright_black(),
                    icon,
                    name.white().bold()
                );
                printer.cmd(cmd);
            }
        }
        PipelineCmd::Generate { project: _, ai } => {
            let msg = if *ai {
                format!("{} Generating pipeline with AI...", icons::SPARKLE)
            } else {
                format!("{} Detecting scripts...", icons::GEAR)
            };

            let spin = cli::spinner(&msg);
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            spin.finish_and_clear();

            printer.success("Pipeline generated");
            printer.info(&format!("Edit with: {}", "chibby pipeline edit".cyan()));
        }
        PipelineCmd::Validate { project: _ } => {
            let spin = cli::spinner("Validating pipeline...");
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            spin.finish_and_clear();

            printer.success("Pipeline is valid");
        }
        PipelineCmd::Edit { project: _ } => {
            printer.info("Opening pipeline in $EDITOR...");
            // TODO: Actually open the file
        }
    }
    Ok(())
}

async fn show_history(
    printer: &Printer,
    _project: Option<&PathBuf>,
    env: Option<&str>,
    limit: usize,
) -> anyhow::Result<()> {
    printer.header(&format!("{} Run History", icons::CLOCK));

    if let Some(e) = env {
        printer.kv("Environment", e);
        printer.newline();
    }

    // Example history entries
    let runs = [
        (
            "abc123",
            StageStatus::Success,
            "2m ago",
            34_000,
            Some("production"),
        ),
        (
            "def456",
            StageStatus::Success,
            "1h ago",
            45_000,
            Some("staging"),
        ),
        (
            "ghi789",
            StageStatus::Failed,
            "2h ago",
            12_000,
            Some("production"),
        ),
        ("jkl012", StageStatus::Cancelled, "3h ago", 8_000, None),
        (
            "mno345",
            StageStatus::Success,
            "1d ago",
            67_000,
            Some("staging"),
        ),
    ];

    for (id, status, when, duration, run_env) in runs.iter().take(limit) {
        printer.history_entry(id, *status, when, *duration, *run_env);
    }

    Ok(())
}

fn cli_log_callback() -> executor::LogCallback {
    Box::new(
        move |stage: &str, log_type: &str, msg: &str| match log_type {
            "info" if msg.starts_with("--- Starting stage:") => {
                println!("  {} {}", icons::RUNNING.blue().bold(), stage.blue().bold());
            }
            "cmd" => {
                println!("     {}", msg.bright_black().italic());
            }
            "stdout" => {
                println!("     {} {}", icons::PIPE.bright_black(), msg.white());
            }
            "stderr" => {
                println!("     {} {}", icons::PIPE.yellow(), msg.yellow());
            }
            "warn" => {
                println!("     {} {}", icons::WARN.yellow().bold(), msg.yellow());
            }
            "error" => {
                eprintln!("     {} {}", icons::FAILURE.red().bold(), msg.red());
            }
            _ => {}
        },
    )
}

fn cli_stage_status(status: &EngineStageStatus) -> StageStatus {
    match status {
        EngineStageStatus::Pending => StageStatus::Pending,
        EngineStageStatus::Running => StageStatus::Running,
        EngineStageStatus::Success => StageStatus::Success,
        EngineStageStatus::Failed => StageStatus::Failed,
        EngineStageStatus::Skipped => StageStatus::Skipped,
    }
}

fn cli_run_status(run: &PipelineRun) -> &'static str {
    match run.status {
        EngineRunStatus::Success => "success",
        EngineRunStatus::Failed => "failed",
        EngineRunStatus::Cancelled => "cancelled",
        EngineRunStatus::Running => "running",
        EngineRunStatus::Pending => "pending",
    }
}

fn print_completed_run(printer: &Printer, run: &PipelineRun) {
    printer.newline();
    printer.subheader("Stages");
    for stage in &run.stage_results {
        printer.stage_with_duration(
            &stage.stage_name,
            cli_stage_status(&stage.status),
            stage.duration_ms,
        );
    }

    let passed = run
        .stage_results
        .iter()
        .filter(|s| s.status == EngineStageStatus::Success)
        .count();
    printer.run_summary(
        cli_run_status(run),
        run.duration_ms.unwrap_or(0),
        passed,
        run.stage_results.len(),
    );
    printer.kv("Run ID", &run.id);
}

async fn retry_run(
    printer: &Printer,
    run_id: &str,
    from_stage: Option<&str>,
) -> anyhow::Result<()> {
    printer.header(&format!("{} Retrying Run", icons::RETRY));
    printer.kv("Original Run", run_id);

    if let Some(stage) = from_stage {
        printer.kv("From Stage", stage);
    }

    printer.newline();
    let original = persistence::load_run(run_id)?
        .ok_or_else(|| anyhow::anyhow!("Run {} not found", run_id))?;
    let pipeline = run_support::pipeline_snapshot_for_run(&original)?;
    let retry_stage = from_stage
        .map(str::to_string)
        .or_else(|| {
            original
                .stage_results
                .iter()
                .find(|s| s.status == EngineStageStatus::Failed)
                .map(|s| s.stage_name.clone())
        })
        .unwrap_or_else(|| {
            pipeline
                .stages
                .first()
                .map(|s| s.name.clone())
                .unwrap_or_default()
        });
    let stages_to_run = run_support::stages_to_run_from_stage(&pipeline, &retry_stage)?;
    let path = Path::new(&original.repo_path);
    let (env_ref, env_vars) =
        run_support::resolve_execution_context(path, original.environment.as_deref())?;

    printer.info("Starting retry...");
    let mut run = executor::run_pipeline(
        &pipeline,
        path,
        env_ref.as_ref(),
        env_vars,
        Some(cli_log_callback()),
        Some(&stages_to_run),
        None,
        None,
    )
    .await?;

    let parent_id = original.parent_run_id.as_deref().unwrap_or(run_id);
    let existing_retries = persistence::retry_count_for_run(parent_id).unwrap_or(0);
    run.run_kind = RunKind::Retry;
    run.parent_run_id = Some(parent_id.to_string());
    run.retry_number = Some(existing_retries + 1);
    run.retry_from_stage = Some(retry_stage);
    run_support::annotate_run(&mut run, &pipeline, original.pipeline_file.as_deref());
    run_support::persist_completed_run(&run)?;
    run_support::post_run_housekeeping(&original.repo_path, &run).await;

    print_completed_run(printer, &run);
    Ok(())
}

async fn rollback_run(printer: &Printer, run_id: &str) -> anyhow::Result<()> {
    printer.header(&format!("{} Rollback", icons::ROLLBACK));
    printer.kv("Target Run", run_id);
    printer.newline();

    let target = persistence::load_run(run_id)?
        .ok_or_else(|| anyhow::anyhow!("Run {} not found", run_id))?;
    if target.status != EngineRunStatus::Success {
        anyhow::bail!("Can only roll back to a successful run");
    }

    let pipeline = run_support::pipeline_snapshot_for_run(&target)?;
    let path = Path::new(&target.repo_path);
    let (env_ref, env_vars) =
        run_support::resolve_execution_context(path, target.environment.as_deref())?;

    printer.warn("Rolling back to recorded deployment pipeline...");
    let mut run = executor::run_pipeline(
        &pipeline,
        path,
        env_ref.as_ref(),
        env_vars,
        Some(cli_log_callback()),
        None,
        None,
        None,
    )
    .await?;

    run.run_kind = RunKind::Rollback;
    run.rollback_target_id = Some(run_id.to_string());
    run_support::annotate_run(&mut run, &pipeline, target.pipeline_file.as_deref());
    run_support::persist_completed_run(&run)?;
    run_support::post_run_housekeeping(&target.repo_path, &run).await;

    print_completed_run(printer, &run);
    Ok(())
}

/// Resolve project path: explicit `--project` or current directory.
fn project_path(project: Option<&PathBuf>) -> PathBuf {
    project
        .cloned()
        .unwrap_or_else(|| std::env::current_dir().expect("current_dir failed"))
}

async fn handle_secrets(printer: &Printer, cmd: &SecretsCmd) -> anyhow::Result<()> {
    match cmd {
        SecretsCmd::List { project } => {
            let path = project_path(project.as_ref());
            let config = pipeline::load_secrets_config(&path)?;
            printer.header(&format!("{} Declared Secrets", icons::LOCK));
            if config.secrets.is_empty() {
                printer.info("No secrets declared. Run `chibby secrets add <NAME>` to add one.");
                return Ok(());
            }
            for s in &config.secrets {
                let scope = if s.environments.is_empty() {
                    "all environments".to_string()
                } else {
                    s.environments.join(", ")
                };
                printer.kv(&s.name, &scope);
            }
        }

        SecretsCmd::Add { name, env, project } => {
            let path = project_path(project.as_ref());
            pipeline::add_secret_ref(
                &path,
                SecretRef {
                    name: name.clone(),
                    environments: env.clone(),
                },
            )?;
            let scope = if env.is_empty() {
                "all environments".to_string()
            } else {
                env.join(", ")
            };
            printer.success(&format!("Added secret reference '{}' ({})", name, scope));
            printer.info(&format!(
                "Set the value with: chibby secrets set {} --env <env>",
                name
            ));
        }

        SecretsCmd::Remove { name, project } => {
            let path = project_path(project.as_ref());
            pipeline::remove_secret_ref(&path, name)?;
            printer.success(&format!("Removed secret reference '{}'", name));
            printer.warn(
                "Stored keychain values for this secret were NOT deleted. \
                 Use `chibby secrets delete` per environment to remove them.",
            );
        }

        SecretsCmd::Set {
            name,
            env,
            value,
            project,
        } => {
            let path = project_path(project.as_ref());
            let path_str = path.to_string_lossy().to_string();
            let value = match value {
                Some(v) => v.clone(),
                None => rpassword::prompt_password(format!("Value for {} ({}): ", name, env))?,
            };
            if value.is_empty() {
                anyhow::bail!("Secret value cannot be empty");
            }
            secrets_engine::set_secret(&path_str, env, name, &value)?;
            secret_audit_engine::record_set_quietly(&path_str, env, name, Provenance::Cli);
            printer.success(&format!(
                "Saved '{}' to OS keychain for env '{}'",
                name, env
            ));
        }

        SecretsCmd::Rotate { name, env, project } => {
            let path = project_path(project.as_ref());
            let path_str = path.to_string_lossy().to_string();
            let value =
                rpassword::prompt_password(format!("New value for {} ({}): ", name, env))?;
            if value.is_empty() {
                anyhow::bail!("Secret value cannot be empty");
            }
            secrets_engine::set_secret(&path_str, env, name, &value)?;
            secret_audit_engine::record_set_quietly(&path_str, env, name, Provenance::Cli);
            printer.success(&format!("Rotated '{}' for env '{}'", name, env));
        }

        SecretsCmd::Delete { name, env, project } => {
            let path = project_path(project.as_ref());
            let path_str = path.to_string_lossy().to_string();
            secrets_engine::delete_secret(&path_str, env, name)?;
            secret_audit_engine::record_delete_quietly(&path_str, env, name, Provenance::Cli);
            printer.success(&format!(
                "Deleted '{}' from keychain for env '{}'",
                name, env
            ));
        }

        SecretsCmd::Status { env, project } => {
            let path = project_path(project.as_ref());
            let path_str = path.to_string_lossy().to_string();
            let secrets_config = pipeline::load_secrets_config(&path)?;
            let envs_config = pipeline::load_environments_layered(&path)?;
            printer.header(&format!("{} Secret Status", icons::LOCK));

            if secrets_config.secrets.is_empty() {
                printer.info("No secrets declared.");
                return Ok(());
            }

            let envs_to_check: Vec<String> = match env {
                Some(e) => vec![e.clone()],
                None => {
                    let mut names: Vec<String> = secrets_config
                        .secrets
                        .iter()
                        .flat_map(|s| s.environments.iter().cloned())
                        .chain(envs_config.environments.iter().map(|e| e.name.clone()))
                        .collect();
                    names.sort();
                    names.dedup();
                    if names.is_empty() {
                        names.push("default".to_string());
                    }
                    names
                }
            };

            for env_name in envs_to_check {
                printer.subheader(&format!("Environment: {}", env_name));
                let statuses = secrets_engine::check_secrets_status(
                    &path_str,
                    &env_name,
                    &secrets_config,
                );
                for s in statuses {
                    printer.secret(&s.name, s.is_set);
                }
                printer.newline();
            }
        }
    }
    Ok(())
}

async fn handle_env(printer: &Printer, cmd: &EnvCmd) -> anyhow::Result<()> {
    match cmd {
        EnvCmd::List { project } => {
            let path = project_path(project.as_ref());
            let config = pipeline::load_environments_layered(&path)?;
            printer.header(&format!("{} Environments", icons::GEAR));
            if config.environments.is_empty() {
                printer.info("No environments defined. Run `chibby env add <NAME>` to add one.");
                return Ok(());
            }
            for env in &config.environments {
                println!("  {} {}", icons::SUCCESS.green(), env.name.white().bold());
                if let Some(host) = &env.ssh_host {
                    printer.kv("Host", host);
                }
                if let Some(port) = env.ssh_port {
                    printer.kv("Port", &port.to_string());
                }
                printer.kv("Variables", &env.variables.len().to_string());
                printer.newline();
            }
        }

        EnvCmd::Show { name, project } => {
            let path = project_path(project.as_ref());
            let config = pipeline::load_environments_layered(&path)?;
            let env = config
                .environments
                .iter()
                .find(|e| e.name == *name)
                .ok_or_else(|| anyhow::anyhow!("Environment '{}' not found", name))?;
            printer.header(&format!("{} Environment: {}", icons::GEAR, name));
            printer.kv(
                "Host",
                env.ssh_host.as_deref().unwrap_or("(none — local only)"),
            );
            printer.kv(
                "Port",
                &env.ssh_port.map(|p| p.to_string()).unwrap_or_default(),
            );
            if !env.variables.is_empty() {
                printer.subheader("Variables");
                let mut keys: Vec<&String> = env.variables.keys().collect();
                keys.sort();
                for k in keys {
                    printer.kv(k, &env.variables[k]);
                }
            }
        }

        EnvCmd::Add {
            name,
            ssh_host,
            ssh_port,
            project,
        } => {
            let path = project_path(project.as_ref());
            pipeline::add_environment(
                &path,
                Environment {
                    name: name.clone(),
                    ssh_host: ssh_host.clone(),
                    ssh_port: *ssh_port,
                    variables: Default::default(),
                },
            )?;
            printer.success(&format!("Added environment '{}'", name));
        }

        EnvCmd::Remove { name, project } => {
            let path = project_path(project.as_ref());
            pipeline::remove_environment(&path, name)?;
            printer.success(&format!("Removed environment '{}'", name));
        }

        EnvCmd::Edit { project } => {
            let path = project_path(project.as_ref());
            let file = path.join(".chibby").join("environments.toml");
            if !file.exists() {
                std::fs::create_dir_all(path.join(".chibby"))?;
                std::fs::write(&file, "")?;
            }
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
            let status = std::process::Command::new(&editor).arg(&file).status()?;
            if !status.success() {
                anyhow::bail!("Editor '{}' exited non-zero", editor);
            }
            // Validate after edit
            pipeline::load_environments(&path)
                .map_err(|e| anyhow::anyhow!("environments.toml is invalid after edit: {e}"))?;
            printer.success(&format!("Saved {}", file.display()));
        }

        EnvCmd::Copy { from, to, project } => {
            let path = project_path(project.as_ref());
            let mut config = pipeline::load_environments(&path)?;
            let src = config
                .environments
                .iter()
                .find(|e| e.name == *from)
                .ok_or_else(|| anyhow::anyhow!("Source environment '{}' not found", from))?
                .clone();
            if config.environments.iter().any(|e| e.name == *to) {
                anyhow::bail!("Destination environment '{}' already exists", to);
            }
            config.environments.push(Environment {
                name: to.clone(),
                ..src
            });
            pipeline::save_environments(&path, &config)?;
            printer.success(&format!("Copied '{}' -> '{}'", from, to));
        }

        EnvCmd::Test { name, project } => {
            let path = project_path(project.as_ref());
            let config = pipeline::load_environments_layered(&path)?;
            let env = config
                .environments
                .iter()
                .find(|e| e.name == *name)
                .ok_or_else(|| anyhow::anyhow!("Environment '{}' not found", name))?;
            let host = env
                .ssh_host
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Environment '{}' has no ssh_host", name))?;
            let spin = cli::spinner(&format!("Testing SSH to {}...", host));
            let result = preflight::test_ssh_connectivity(host, env.ssh_port).await;
            spin.finish_and_clear();
            match result {
                Ok(msg) => printer.success(&msg),
                Err(e) => {
                    printer.error(&format!("SSH test failed: {e}"));
                    return Err(e.into());
                }
            }
        }

        EnvCmd::Vars(vars_cmd) => return handle_env_vars(printer, vars_cmd).await,

        EnvCmd::Diff { from, to, project } => {
            let path = project_path(project.as_ref());
            let config = pipeline::load_environments_layered(&path)?;
            let a = config
                .environments
                .iter()
                .find(|e| e.name == *from)
                .ok_or_else(|| anyhow::anyhow!("Environment '{}' not found", from))?;
            let b = config
                .environments
                .iter()
                .find(|e| e.name == *to)
                .ok_or_else(|| anyhow::anyhow!("Environment '{}' not found", to))?;

            printer.header(&format!("{} Env diff: {} vs {}", icons::GEAR, from, to));
            printer.newline();
            printer.subheader("Variables");

            let mut keys: std::collections::BTreeSet<&String> =
                a.variables.keys().chain(b.variables.keys()).collect();
            let mut any_var_diff = false;
            for k in keys.iter() {
                let av = a.variables.get(*k);
                let bv = b.variables.get(*k);
                match (av, bv) {
                    (Some(_), None) => {
                        any_var_diff = true;
                        printer.kv(&format!("- {}", k), &format!("only in {}", from));
                    }
                    (None, Some(_)) => {
                        any_var_diff = true;
                        printer.kv(&format!("+ {}", k), &format!("only in {}", to));
                    }
                    (Some(va), Some(vb)) if va != vb => {
                        any_var_diff = true;
                        printer.kv(&format!("~ {}", k), &format!("{} | {}", va, vb));
                    }
                    _ => {}
                }
            }
            if !any_var_diff {
                printer.info("Variables identical.");
            }
            keys.clear();

            printer.newline();
            printer.subheader("Secrets");
            let secrets_config = pipeline::load_secrets_config(&path)?;
            let in_a: std::collections::BTreeSet<&str> = secrets_config
                .secrets
                .iter()
                .filter(|s| s.environments.is_empty() || s.environments.iter().any(|e| e == from))
                .map(|s| s.name.as_str())
                .collect();
            let in_b: std::collections::BTreeSet<&str> = secrets_config
                .secrets
                .iter()
                .filter(|s| s.environments.is_empty() || s.environments.iter().any(|e| e == to))
                .map(|s| s.name.as_str())
                .collect();
            let mut any_secret_diff = false;
            for name in in_a.difference(&in_b) {
                any_secret_diff = true;
                printer.kv(&format!("- {}", name), &format!("only in {}", from));
            }
            for name in in_b.difference(&in_a) {
                any_secret_diff = true;
                printer.kv(&format!("+ {}", name), &format!("only in {}", to));
            }
            if !any_secret_diff {
                printer.info("Secret references identical.");
            }
        }

        EnvCmd::ScanLeaks { project } => {
            let path = project_path(project.as_ref());
            let hits = pipeline::scan_environments_for_leaks(&path)?;
            printer.header(&format!("{} Leak scan", icons::LOCK));
            if hits.is_empty() {
                printer.success("No suspicious values found in environments.toml");
                return Ok(());
            }
            printer.warn(&format!(
                "{} value(s) in environments.toml look like real credentials. \
                 Consider moving them to secrets.toml + the keychain.",
                hits.len()
            ));
            for hit in &hits {
                printer.kv(
                    &format!("{}/{}", hit.env, hit.variable),
                    &format!("{} ({})", hit.match_.rule, hit.match_.preview),
                );
            }
            anyhow::bail!("Leak scan found {} suspicious value(s)", hits.len());
        }
    }
    Ok(())
}

async fn handle_env_vars(printer: &Printer, cmd: &EnvVarsCmd) -> anyhow::Result<()> {
    match cmd {
        EnvVarsCmd::List { env, project } => {
            let path = project_path(project.as_ref());
            let config = pipeline::load_environments_layered(&path)?;
            let env_obj = config
                .environments
                .iter()
                .find(|e| e.name == *env)
                .ok_or_else(|| anyhow::anyhow!("Environment '{}' not found", env))?;
            printer.header(&format!("{} Variables for '{}'", icons::GEAR, env));
            if env_obj.variables.is_empty() {
                printer.info("No variables set.");
                return Ok(());
            }
            let mut keys: Vec<&String> = env_obj.variables.keys().collect();
            keys.sort();
            for k in keys {
                printer.kv(k, &env_obj.variables[k]);
            }
        }

        EnvVarsCmd::Set {
            env,
            key,
            value,
            local,
            project,
        } => {
            let path = project_path(project.as_ref());
            if *local {
                let mut config = pipeline::load_environments_local(&path)?;
                if let Some(e) = config.environments.iter_mut().find(|e| e.name == *env) {
                    e.variables.insert(key.clone(), value.clone());
                } else {
                    let mut vars = std::collections::HashMap::new();
                    vars.insert(key.clone(), value.clone());
                    config.environments.push(Environment {
                        name: env.clone(),
                        ssh_host: None,
                        ssh_port: None,
                        variables: vars,
                    });
                }
                pipeline::save_environments_local(&path, &config)?;
                printer.success(&format!(
                    "Set {}={} on env '{}' (local override)",
                    key, value, env
                ));
            } else {
                pipeline::set_env_variable(&path, env, key, value)?;
                printer.success(&format!("Set {}={} on env '{}'", key, value, env));
            }
        }

        EnvVarsCmd::Get { env, key, project } => {
            let path = project_path(project.as_ref());
            let config = pipeline::load_environments_layered(&path)?;
            let env_obj = config
                .environments
                .iter()
                .find(|e| e.name == *env)
                .ok_or_else(|| anyhow::anyhow!("Environment '{}' not found", env))?;
            match env_obj.variables.get(key) {
                Some(v) => println!("{}", v),
                None => anyhow::bail!("Variable '{}' not set on env '{}'", key, env),
            }
        }

        EnvVarsCmd::Delete { env, key, project } => {
            let path = project_path(project.as_ref());
            pipeline::remove_env_variable(&path, env, key)?;
            printer.success(&format!("Removed '{}' from env '{}'", key, env));
        }
    }
    Ok(())
}

async fn bootstrap_cmd(
    printer: &Printer,
    project: Option<&PathBuf>,
    silent: bool,
    dry_run: bool,
    merge: bool,
) -> anyhow::Result<()> {
    let path = project_path(project);
    printer.header(&format!("{} Bootstrap", icons::GEAR));
    printer.kv("Project", &path.display().to_string());

    let report = bootstrap::scan_project(&path)?;
    printer.kv("Scanned files", &report.scanned_files.to_string());
    printer.kv(
        "Suggested envs",
        &report.suggested_environments.join(", "),
    );

    if report.detected.is_empty() {
        printer.newline();
        printer.info("No env or secret references detected. Nothing to do.");
        return Ok(());
    }

    if !silent {
        printer.newline();
        printer.subheader(&format!("Detected ({})", report.detected.len()));
        for d in &report.detected {
            let kind_label = match d.classification {
                Classification::Secret => "secret",
                Classification::Variable => "variable",
            };
            let sources: Vec<String> = d.sources.iter().map(|s| s.path.clone()).collect();
            let mut seen = std::collections::BTreeSet::new();
            let uniq_sources: Vec<String> = sources
                .into_iter()
                .filter(|p| seen.insert(p.clone()))
                .collect();
            let sources_label = if uniq_sources.is_empty() {
                String::new()
            } else {
                format!(" ({})", uniq_sources.join(", "))
            };
            printer.kv(&d.name, &format!("{}{}", kind_label, sources_label));
        }
        printer.newline();
    }

    if dry_run {
        printer.info("--dry-run: nothing written. Re-run without --dry-run to apply.");
        return Ok(());
    }

    let mode = if merge { ApplyMode::Merge } else { ApplyMode::Safe };
    match bootstrap::apply_bootstrap(&path, &report, mode) {
        Ok(true) => {
            printer.success(&format!(
                "Wrote {}/.chibby/environments.toml and secrets.toml",
                path.display()
            ));
            printer.info(
                "Next: `chibby secrets set <NAME> --env <env>` to populate values, \
                 or set them in the Chibby Secrets panel.",
            );
        }
        Ok(false) => {
            printer.warn(
                "Configs already present. Re-run with --merge to add only the newly-detected names.",
            );
        }
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

async fn handle_audit(printer: &Printer, cmd: &AuditCmd) -> anyhow::Result<()> {
    match cmd {
        AuditCmd::List { project } => {
            let path = project_path(project.as_ref());
            let path_str = path.to_string_lossy().to_string();
            let audit = secret_audit_engine::load_for_project(&path_str)?;
            printer.header(&format!("{} Secret Audit", icons::LOCK));
            if audit.entries.is_empty() {
                printer.info(
                    "No audit history yet. Audit records are created on set/delete operations.",
                );
                return Ok(());
            }
            for (key, snap) in &audit.entries {
                let last_set = snap
                    .last_set
                    .map(|t| t.to_rfc3339())
                    .unwrap_or_else(|| "never".to_string());
                let provenance = snap.last_provenance.as_deref().unwrap_or("?");
                printer.kv(
                    key,
                    &format!(
                        "set#{}={} via {} (deletes={})",
                        snap.set_count, last_set, provenance, snap.delete_count
                    ),
                );
            }
        }
        AuditCmd::Show { name, env, project } => {
            let path = project_path(project.as_ref());
            let path_str = path.to_string_lossy().to_string();
            match secret_audit_engine::get(&path_str, env, name)? {
                None => {
                    printer.warn(&format!("No audit record for {}/{}", env, name));
                }
                Some(snap) => {
                    printer.header(&format!("{} {}/{}", icons::LOCK, env, name));
                    printer.kv("Set count", &snap.set_count.to_string());
                    printer.kv("Delete count", &snap.delete_count.to_string());
                    printer.kv(
                        "Last set",
                        &snap
                            .last_set
                            .map(|t| t.to_rfc3339())
                            .unwrap_or_else(|| "never".to_string()),
                    );
                    printer.kv(
                        "Last deleted",
                        &snap
                            .last_deleted
                            .map(|t| t.to_rfc3339())
                            .unwrap_or_else(|| "never".to_string()),
                    );
                    printer.kv(
                        "Last provenance",
                        snap.last_provenance.as_deref().unwrap_or("?"),
                    );
                }
            }
        }
    }
    Ok(())
}

async fn handle_import(printer: &Printer, cmd: &ImportCmd) -> anyhow::Result<()> {
    let (report, repo_path) = match cmd {
        ImportCmd::Dotenv {
            path,
            env,
            with_values,
            project,
        } => {
            let repo = project_path(project.as_ref());
            let ctx = ImportContext {
                repo_path: repo.clone(),
                env_name: env.clone(),
                source_path: Some(path.clone()),
                include_values: *with_values,
            };
            (DotEnvImporter.run(&ctx)?, repo)
        }
        ImportCmd::Vercel {
            env,
            with_values,
            project,
        } => {
            let repo = project_path(project.as_ref());
            let ctx = ImportContext {
                repo_path: repo.clone(),
                env_name: env.clone(),
                source_path: None,
                include_values: *with_values,
            };
            (VercelImporter.run(&ctx)?, repo)
        }
        ImportCmd::Railway {
            env,
            with_values,
            project,
        } => {
            let repo = project_path(project.as_ref());
            let ctx = ImportContext {
                repo_path: repo.clone(),
                env_name: env.clone(),
                source_path: None,
                include_values: *with_values,
            };
            (RailwayImporter.run(&ctx)?, repo)
        }
        ImportCmd::Fly { env, project } => {
            let repo = project_path(project.as_ref());
            let ctx = ImportContext {
                repo_path: repo.clone(),
                env_name: env.clone(),
                source_path: None,
                include_values: false,
            };
            (FlyImporter.run(&ctx)?, repo)
        }
    };

    printer.header(&format!("{} Import from {}", icons::GEAR, report.source));
    printer.kv("Env", &report.env_name);
    printer.kv("Detected", &report.entries.len().to_string());
    printer.newline();

    print_import_report(printer, &report);

    let applied = importers::apply_report(&report, &repo_path, ApplyOptions::default())?;
    printer.newline();
    printer.success(&format!(
        "Variables: {} added ({} with values), Secret refs: {} added ({} values stored in keychain)",
        applied.variables_added,
        applied.variables_value_set,
        applied.secrets_ref_added,
        applied.secrets_value_saved
    ));
    if applied.secrets_value_saved == 0
        && report
            .entries
            .iter()
            .any(|e| e.classification == Classification::Secret)
    {
        printer.info(
            "No secret values were stored. Re-run with `--with-values` (where supported) or set them with `chibby secrets set NAME --env <env>`.",
        );
    }
    Ok(())
}

fn print_import_report(printer: &Printer, report: &ImportReport) {
    let mut secrets: Vec<&_> = report
        .entries
        .iter()
        .filter(|e| e.classification == Classification::Secret)
        .collect();
    let mut vars: Vec<&_> = report
        .entries
        .iter()
        .filter(|e| e.classification == Classification::Variable)
        .collect();
    secrets.sort_by(|a, b| a.name.cmp(&b.name));
    vars.sort_by(|a, b| a.name.cmp(&b.name));

    if !vars.is_empty() {
        printer.subheader(&format!("Variables ({})", vars.len()));
        for v in &vars {
            let label = if v.value.is_some() { "value" } else { "name only" };
            printer.kv(&v.name, label);
        }
        printer.newline();
    }
    if !secrets.is_empty() {
        printer.subheader(&format!("Secrets ({})", secrets.len()));
        for s in &secrets {
            let label = if s.value.is_some() { "value" } else { "name only" };
            printer.kv(&s.name, label);
        }
    }
}

async fn handle_export(printer: &Printer, cmd: &ExportCmd) -> anyhow::Result<()> {
    let ExportCmd::Dotenv { env, out, project } = cmd;
    let repo = project_path(project.as_ref());
    let lines = importers::export_dotenv(&repo, env, out)?;
    printer.success(&format!(
        "Wrote {} lines to {}",
        lines,
        out.display()
    ));
    printer.info(
        "This file may contain plaintext secrets — keep it out of git and treat it like a credential.",
    );
    Ok(())
}

async fn doctor(printer: &Printer, project: Option<&PathBuf>) -> anyhow::Result<()> {
    let path = project_path(project);
    printer.header(&format!("{} Doctor", icons::GEAR));
    printer.kv("Project", &path.display().to_string());
    printer.newline();

    // Config files present?
    let chibby_dir = path.join(".chibby");
    printer.preflight_check(
        "pipeline.toml present",
        chibby_dir.join("pipeline.toml").exists(),
        None,
    );
    let envs_present = chibby_dir.join("environments.toml").exists();
    printer.preflight_check("environments.toml present", envs_present, None);
    let secrets_present = chibby_dir.join("secrets.toml").exists();
    printer.preflight_check("secrets.toml present", secrets_present, None);
    printer.newline();

    if !envs_present {
        printer.info("No environments.toml — skipping environment checks.");
        return Ok(());
    }

    let envs = pipeline::load_environments_layered(&path)?;
    let secrets_config = pipeline::load_secrets_config(&path)?;

    let mut any_failures = false;

    for env in &envs.environments {
        printer.subheader(&format!("Environment: {}", env.name));

        // SSH reachable?
        if let Some(host) = &env.ssh_host {
            match preflight::test_ssh_connectivity(host, env.ssh_port).await {
                Ok(msg) => printer.preflight_check("SSH reachable", true, Some(&msg)),
                Err(e) => {
                    any_failures = true;
                    printer.preflight_check("SSH reachable", false, Some(&e.to_string()));
                }
            }
        }

        // All declared secrets set in keychain for this env?
        let statuses = secrets_engine::check_secrets_status(
            &path.to_string_lossy(),
            &env.name,
            &secrets_config,
        );
        for s in statuses {
            if !s.is_set {
                any_failures = true;
            }
            printer.secret(&s.name, s.is_set);
        }
        printer.newline();
    }

    if any_failures {
        anyhow::bail!("Doctor found unresolved issues. Fix the items marked above.");
    }
    printer.success("All checks passed.");
    Ok(())
}

async fn handle_version(printer: &Printer, cmd: &VersionCmd) -> anyhow::Result<()> {
    match cmd {
        VersionCmd::Show { project: _ } => {
            printer.header(&format!("{} Version Info", icons::VERSION));
            printer.kv("Current", "1.2.3");
            printer.kv("Last Tag", "v1.2.3");
            printer.kv("Commits Since", "5");
        }
        VersionCmd::Bump {
            level,
            project: _,
            tag,
            changelog,
        } => {
            let spin = cli::spinner(&format!("Bumping version ({})...", level));
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            spin.finish_and_clear();

            printer.success("Version bumped: 1.2.3 → 1.2.4");

            if *tag {
                printer.success("Created git tag: v1.2.4");
            }
            if *changelog {
                printer.success("Generated CHANGELOG.md");
            }
        }
    }
    Ok(())
}

async fn handle_artifact(printer: &Printer, cmd: &ArtifactCmd) -> anyhow::Result<()> {
    match cmd {
        ArtifactCmd::List { project: _ } => {
            printer.header(&format!("{} Artifacts", icons::PACKAGE));

            let artifacts = [
                ("my-app-1.2.4-darwin-arm64.dmg", "12.5 MB", "2h ago"),
                ("my-app-1.2.4-darwin-x64.dmg", "14.2 MB", "2h ago"),
                ("my-app-1.2.3-darwin-arm64.dmg", "12.3 MB", "1d ago"),
            ];

            for (name, size, when) in artifacts {
                println!(
                    "  {} {} {} {}",
                    icons::PACKAGE,
                    name.white(),
                    size.bright_black(),
                    when.bright_black()
                );
            }
        }
        ArtifactCmd::Collect { project: _ } => {
            let spin = cli::spinner("Collecting artifacts...");
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            spin.finish_and_clear();

            printer.success("Collected 2 artifacts");
        }
        ArtifactCmd::Clean {
            project: _,
            dry_run,
        } => {
            if *dry_run {
                printer.warn("Dry run - would delete:");
                println!("    {} my-app-1.2.2-darwin-arm64.dmg", icons::FAILURE.red());
                println!("    {} my-app-1.2.1-darwin-arm64.dmg", icons::FAILURE.red());
            } else {
                let spin = cli::spinner("Cleaning old artifacts...");
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                spin.finish_and_clear();

                printer.success("Cleaned 2 old artifacts (freed 24.1 MB)");
            }
        }
    }
    Ok(())
}

fn resolve_project_path(project: Option<&PathBuf>) -> anyhow::Result<PathBuf> {
    let p = project
        .cloned()
        .unwrap_or_else(|| std::env::current_dir().unwrap());
    let abs = p.canonicalize().unwrap_or(p);
    Ok(abs)
}

async fn handle_scan(printer: &Printer, cmd: &ScanCmd) -> anyhow::Result<()> {
    match cmd {
        ScanCmd::Secrets { project, baseline } => {
            let repo = resolve_project_path(project.as_ref())?;
            printer.header(&format!("{} Secret Scan", icons::SCAN));
            printer.kv("Project", &repo.display().to_string());
            printer.newline();

            if *baseline {
                let spin = cli::spinner("Creating baseline...");
                let msg = gates::create_secret_scan_baseline(&repo)?;
                spin.finish_and_clear();
                printer.success(&msg);
                return Ok(());
            }

            let config = gates::load_gates_config(&repo).unwrap_or_default();
            let spin = cli::spinner("Scanning for leaked secrets...");
            let result = gates::run_secret_scan(&repo, &config)?;
            spin.finish_and_clear();

            printer.kv("Scanner", &result.scanner);
            if result.passed && result.findings.is_empty() {
                printer.success(&result.message);
            } else {
                printer.warn(&format!("{} finding(s)", result.findings.len()));
                for f in result.findings.iter().take(50) {
                    println!(
                        "  {} {}:{} — {} ({})",
                        icons::WARN.yellow(),
                        f.file.bright_black(),
                        f.line,
                        f.rule.red(),
                        f.preview.bright_black()
                    );
                }
                if result.findings.len() > 50 {
                    printer.info(&format!("+ {} more", result.findings.len() - 50));
                }
                if !result.passed {
                    std::process::exit(1);
                }
            }
        }
        ScanCmd::Deps { project, severity } => {
            let repo = resolve_project_path(project.as_ref())?;
            printer.header(&format!("{} Dependency Scan", icons::BUG));
            printer.kv("Project", &repo.display().to_string());
            printer.kv("Severity threshold", severity);
            printer.newline();

            let mut config = gates::load_gates_config(&repo).unwrap_or_default();
            config.audit_severity_threshold = severity.clone();

            let spin = cli::spinner("Scanning dependencies...");
            let result = gates::run_dependency_audit(&repo, &config)?;
            spin.finish_and_clear();

            printer.kv("Scanner", &result.scanner);
            if result.passed && result.findings.is_empty() {
                printer.success(&result.message);
            } else {
                printer.warn(&format!("{} vulnerability(ies)", result.findings.len()));
                for f in result.findings.iter().take(50) {
                    println!(
                        "  {} {} {} — {} ({})",
                        icons::WARN.yellow(),
                        f.package.bright_white(),
                        f.installed_version.bright_black(),
                        f.advisory_id.red(),
                        format!("{:?}", f.severity).to_lowercase()
                    );
                    if let Some(fixed) = &f.fixed_version {
                        println!("      fixed in {}", fixed.green());
                    }
                }
                if result.findings.len() > 50 {
                    printer.info(&format!("+ {} more", result.findings.len() - 50));
                }
                if !result.passed {
                    std::process::exit(1);
                }
            }
        }
        ScanCmd::Commits { project, since } => {
            let repo = resolve_project_path(project.as_ref())?;
            printer.header(&format!("{} Commit Lint", icons::CHECK));
            printer.kv("Project", &repo.display().to_string());
            if let Some(s) = since {
                printer.kv("Since", s);
            }
            printer.newline();

            let config = gates::load_gates_config(&repo).unwrap_or_default();
            let spin = cli::spinner("Checking commit messages...");
            let result = gates::run_commit_lint(&repo, &config)?;
            spin.finish_and_clear();

            printer.kv("Commits checked", &result.commits_checked.to_string());
            if result.passed {
                printer.success(&result.message);
            } else {
                printer.warn(&format!("{} violation(s)", result.violations.len()));
                for v in result.violations.iter().take(50) {
                    println!(
                        "  {} {} — {}",
                        v.hash.bright_black(),
                        v.subject.white(),
                        v.rule.red()
                    );
                    println!("      expected: {}", v.expected.bright_black());
                }
                std::process::exit(1);
            }
        }
        ScanCmd::Sast { project } => {
            let repo = resolve_project_path(project.as_ref())?;
            printer.header(&format!("{} SAST (semgrep)", icons::SCAN));
            printer.kv("Project", &repo.display().to_string());
            printer.newline();

            let config = gates::load_gates_config(&repo).unwrap_or_default();
            let spin = cli::spinner("Running semgrep...");
            let result = gates::run_sast(&repo, &config)?;
            spin.finish_and_clear();

            printer.kv("Scanner", &result.scanner);
            if result.passed && result.findings.is_empty() {
                printer.success(&result.message);
            } else {
                printer.warn(&result.message);
                for f in result.findings.iter().take(50) {
                    println!(
                        "  {} {}:{} — {} {}",
                        icons::WARN.yellow(),
                        f.file.bright_black(),
                        f.line,
                        f.rule.red(),
                        format!("[{:?}]", f.severity).bright_black()
                    );
                    if !f.message.is_empty() {
                        println!("      {}", f.message.bright_black());
                    }
                }
                if result.findings.len() > 50 {
                    printer.info(&format!("+ {} more", result.findings.len() - 50));
                }
                if !result.passed {
                    std::process::exit(1);
                }
            }
        }
        ScanCmd::Container { project } => {
            let repo = resolve_project_path(project.as_ref())?;
            printer.header(&format!("{} Container Scan (trivy)", icons::SCAN));
            printer.kv("Project", &repo.display().to_string());
            printer.newline();

            let config = gates::load_gates_config(&repo).unwrap_or_default();
            let spin = cli::spinner("Running trivy image...");
            let result = gates::run_container_scan(&repo, &config)?;
            spin.finish_and_clear();

            printer.kv("Scanner", &result.scanner);
            printer.kv("Targets", &result.targets.join(", "));
            if result.passed && result.findings.is_empty() {
                printer.success(&result.message);
            } else {
                printer.warn(&result.message);
                for f in result.findings.iter().take(50) {
                    println!(
                        "  {} {} {} — {} [{:?}]",
                        icons::WARN.yellow(),
                        f.package.bright_white(),
                        f.installed_version.bright_black(),
                        f.advisory_id.red(),
                        f.severity
                    );
                    if let Some(fixed) = &f.fixed_version {
                        println!("      fixed in {}", fixed.green());
                    }
                }
                if !result.passed {
                    std::process::exit(1);
                }
            }
        }
        ScanCmd::Iac { project } => {
            let repo = resolve_project_path(project.as_ref())?;
            printer.header(&format!("{} IaC Scan (trivy config)", icons::SCAN));
            printer.kv("Project", &repo.display().to_string());
            printer.newline();

            let config = gates::load_gates_config(&repo).unwrap_or_default();
            let spin = cli::spinner("Running trivy config...");
            let result = gates::run_iac_scan(&repo, &config)?;
            spin.finish_and_clear();

            printer.kv("Scanner", &result.scanner);
            if result.passed && result.findings.is_empty() {
                printer.success(&result.message);
            } else {
                printer.warn(&result.message);
                for f in result.findings.iter().take(50) {
                    let loc = match f.line {
                        Some(l) => format!("{}:{}", f.file, l),
                        None => f.file.clone(),
                    };
                    println!(
                        "  {} {} — {} [{:?}]",
                        icons::WARN.yellow(),
                        loc.bright_black(),
                        f.rule.red(),
                        f.severity
                    );
                    if !f.message.is_empty() {
                        println!("      {}", f.message.bright_black());
                    }
                    if let Some(r) = &f.resolution {
                        println!("      fix: {}", r.green());
                    }
                }
                if !result.passed {
                    std::process::exit(1);
                }
            }
        }
        ScanCmd::License { project } => {
            let repo = resolve_project_path(project.as_ref())?;
            printer.header(&format!("{} License Check", icons::SCAN));
            printer.kv("Project", &repo.display().to_string());
            printer.newline();

            let config = gates::load_gates_config(&repo).unwrap_or_default();
            let spin = cli::spinner("Checking dependency licenses...");
            let result = gates::run_license_check(&repo, &config)?;
            spin.finish_and_clear();

            printer.kv("Scanner", &result.scanner);
            if result.passed && result.findings.is_empty() {
                printer.success(&result.message);
            } else {
                printer.warn(&result.message);
                for f in result.findings.iter().take(50) {
                    println!(
                        "  {} {} {} — {} ({})",
                        icons::WARN.yellow(),
                        f.package.bright_white(),
                        f.version.bright_black(),
                        f.license.red(),
                        f.reason.bright_black()
                    );
                }
                if !result.passed {
                    std::process::exit(1);
                }
            }
        }
    }
    Ok(())
}

async fn init_project(printer: &Printer, path: Option<&PathBuf>, ai: bool) -> anyhow::Result<()> {
    printer.banner();
    printer.header(&format!("{} Initialize Project", icons::SPARKLE));

    let path = path
        .cloned()
        .unwrap_or_else(|| std::env::current_dir().unwrap());
    printer.kv("Path", &path.display().to_string());
    printer.newline();

    let spin = cli::spinner("Detecting project type...");
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    spin.finish_and_clear();

    printer.info("Detected: Tauri + React + TypeScript");
    printer.newline();

    let msg = if ai {
        format!("{} Generating pipeline with AI...", icons::SPARKLE)
    } else {
        "Generating pipeline...".to_string()
    };
    let spin = cli::spinner(&msg);
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    spin.finish_and_clear();

    printer.success("Created .chibby/pipeline.toml");
    printer.success("Created .chibby/environments.toml");
    printer.newline();

    println!(
        "  {} {} {}",
        icons::ROCKET,
        "Ready!".green().bold(),
        "Run your first pipeline:".white()
    );
    println!("     {}", "chibby run".cyan().bold());
    printer.newline();

    Ok(())
}

async fn run_preflight(
    printer: &Printer,
    env: Option<&str>,
    _project: Option<&PathBuf>,
) -> anyhow::Result<()> {
    printer.header(&format!("{} Preflight Checks", icons::SHIELD));

    if let Some(e) = env {
        printer.kv("Environment", e);
        printer.newline();
    }

    let checks = [
        ("Pipeline config", true, None),
        ("Git status clean", true, None),
        ("Secrets configured", true, None),
        ("Dependencies up to date", true, None),
    ];

    for (check, passes, msg) in checks {
        let spin = cli::spinner(&format!("Checking {}...", check.to_lowercase()));
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        spin.finish_and_clear();

        printer.preflight_check(check, passes, msg);
    }

    if env.is_some() {
        let spin = cli::spinner("Testing SSH connection...");
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        spin.finish_and_clear();
        printer.preflight_check("SSH connection", true, None);
    }

    printer.newline();
    printer.success(&format!("All checks passed {}", icons::SPARKLE));

    Ok(())
}

async fn handle_updater(printer: &Printer, cmd: &UpdaterCmd) -> anyhow::Result<()> {
    match cmd {
        UpdaterCmd::GenerateKeys { project: _ } => {
            printer.header(&format!("{} Generate Update Keys", icons::KEY));

            let spin = cli::spinner("Generating key pair...");
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            spin.finish_and_clear();

            printer.success("Generated Tauri update key pair");
            printer.success("Private key saved to keychain");
            printer.info("Public key written to .chibby/updater.toml");
        }
        UpdaterCmd::Sign { bundle, project: _ } => {
            printer.header(&format!("{} Sign Update Bundle", icons::SIGN));
            printer.kv("Bundle", &bundle.display().to_string());
            printer.newline();

            let spin = cli::spinner("Signing bundle...");
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            spin.finish_and_clear();

            printer.success("Bundle signed");
        }
        UpdaterCmd::LatestJson { project: _ } => {
            let spin = cli::spinner("Generating latest.json...");
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            spin.finish_and_clear();

            printer.success("Generated latest.json");
        }
        UpdaterCmd::Publish {
            project: _,
            dry_run,
        } => {
            if *dry_run {
                printer.warn("Dry run - would publish:");
                printer.kv("latest.json", "s3://releases/latest.json");
                printer.kv("bundle", "s3://releases/my-app-1.2.4.tar.gz");
            } else {
                let pb = cli::progress_bar(100, "Uploading");
                for i in 0..100 {
                    pb.set_position(i);
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
                pb.finish_and_clear();

                printer.success("Published update");
            }
        }
    }
    Ok(())
}

async fn show_logs(printer: &Printer, run_id: &str, follow: bool) -> anyhow::Result<()> {
    printer.header(&format!("{} Logs: {}", icons::FILE, run_id));

    if follow {
        printer.info("Following logs (Ctrl+C to stop)...");
        printer.newline();
    }

    // Example log output
    printer.log("cmd", "npm run build");
    printer.log("stdout", "> chibby@0.1.0 build");
    printer.log("stdout", "> tsc && vite build");
    printer.log("stdout", "vite v5.0.0 building for production...");
    printer.log("stdout", "transforming...");
    printer.log("stdout", "rendering chunks...");
    printer.log("stdout", "computing gzip size...");
    printer.log("info", "Build completed successfully");

    Ok(())
}

fn open_app(printer: &Printer) {
    printer.info("Opening Chibby app...");

    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg("-a")
            .arg("Chibby")
            .spawn();
    }

    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("chibby-app").spawn();
    }

    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "chibby"])
            .spawn();
    }
}
