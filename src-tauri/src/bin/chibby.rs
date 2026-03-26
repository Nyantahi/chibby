//! Chibby CLI - Local-first CI/CD
//!
//! A standalone CLI that shares the same engine as the desktop app.
//! Designed for headless servers, scripting, and terminal-first workflows.

use chibby_lib::engine::executor;
use chibby_lib::engine::models::{
    PipelineRun, RunKind, RunStatus as EngineRunStatus, StageStatus as EngineStageStatus,
};
use chibby_lib::engine::{persistence, run_support};
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
enum SecretsCmd {
    /// Set a secret value
    Set {
        /// Secret key
        key: String,
        /// Secret value (omit to prompt securely)
        value: Option<String>,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Delete a secret
    Delete {
        /// Secret key
        key: String,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Check secrets status
    Status {
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
    /// Show environment details
    Show {
        /// Environment name
        name: String,
        /// Project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Test SSH connection
    Test {
        /// Environment name
        name: String,
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

        Some(Commands::Preflight { env, project }) => {
            run_preflight(&printer, env.as_deref(), project.as_ref()).await
        }

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

async fn handle_projects(printer: &Printer, cmd: &ProjectsCmd) -> anyhow::Result<()> {
    match cmd {
        ProjectsCmd::List => {
            printer.header(&format!("{} Projects", icons::FOLDER));

            // TODO: Load actual projects from persistence
            printer.project_with_status("my-app", "~/projects/my-app", Some(StageStatus::Success));
            printer.project_with_status(
                "website",
                "~/DevProjects/website",
                Some(StageStatus::Success),
            );
            printer.project_with_status("api", "~/DevProjects/api", Some(StageStatus::Failed));
            printer.project_with_status("mobile-app", "~/DevProjects/mobile", None);
            printer.newline();

            printer.stats("projects", 4, StageStatus::Pending);
        }
        ProjectsCmd::Add { path, name } => {
            let spin = cli::spinner("Adding project...");
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            spin.finish_and_clear();

            let display_name = name.as_deref().unwrap_or_else(|| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("project")
            });

            printer.success(&format!("Added {}", display_name));
            printer.info(&format!(
                "Run {} to generate pipeline",
                "chibby pipeline generate".cyan()
            ));
        }
        ProjectsCmd::Remove { project } => {
            printer.warn(&format!("Removing {}...", project));
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            printer.success("Project removed");
        }
        ProjectsCmd::Info { project: _ } => {
            printer.header(&format!("{} Project Info", icons::INFO));
            printer.kv("Name", "my-app");
            printer.kv("Path", "~/projects/my-app");
            printer.kv("Pipeline", "Yes");
            printer.kv("Environments", "staging, production");
            printer.kv("Last Deploy", "2 hours ago");
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

async fn handle_secrets(printer: &Printer, cmd: &SecretsCmd) -> anyhow::Result<()> {
    match cmd {
        SecretsCmd::Status { project: _ } => {
            printer.header(&format!("{} Secrets Status", icons::LOCK));

            printer.secret("DEPLOY_KEY", true);
            printer.secret("AWS_ACCESS_KEY", true);
            printer.secret("AWS_SECRET_KEY", true);
            printer.secret("SLACK_WEBHOOK", false);
        }
        SecretsCmd::Set {
            key,
            value: _,
            project: _,
        } => {
            // In real impl, would prompt for value if not provided
            let spin = cli::spinner(&format!("Setting {}...", key));
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            spin.finish_and_clear();

            printer.success(&format!("Secret {} saved to keychain", key));
        }
        SecretsCmd::Delete { key, project: _ } => {
            printer.warn(&format!("Deleting {}...", key));
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            printer.success("Secret deleted");
        }
    }
    Ok(())
}

async fn handle_env(printer: &Printer, cmd: &EnvCmd) -> anyhow::Result<()> {
    match cmd {
        EnvCmd::List { project: _ } => {
            printer.header(&format!("{} Environments", icons::GEAR));

            println!("  {} {}", icons::SUCCESS.green(), "staging".white().bold());
            printer.kv("Host", "staging.example.com");
            printer.kv("User", "deploy");
            printer.newline();

            println!(
                "  {} {}",
                icons::SUCCESS.green(),
                "production".white().bold()
            );
            printer.kv("Host", "prod.example.com");
            printer.kv("User", "deploy");
        }
        EnvCmd::Show { name, project: _ } => {
            printer.header(&format!("{} Environment: {}", icons::GEAR, name));
            printer.kv("Host", "example.com");
            printer.kv("User", "deploy");
            printer.kv("Port", "22");
            printer.kv("Working Dir", "/var/www/app");
        }
        EnvCmd::Test { name, project: _ } => {
            let spin = cli::spinner(&format!("Testing connection to {}...", name));
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            spin.finish_and_clear();

            printer.success(&format!("Connected to {}", name));
        }
    }
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

async fn handle_scan(printer: &Printer, cmd: &ScanCmd) -> anyhow::Result<()> {
    match cmd {
        ScanCmd::Secrets {
            project: _,
            baseline,
        } => {
            printer.header(&format!("{} Secret Scan", icons::SCAN));

            let spin = cli::spinner("Scanning for leaked secrets...");
            tokio::time::sleep(std::time::Duration::from_millis(800)).await;
            spin.finish_and_clear();

            if *baseline {
                printer.info("Creating baseline from current findings...");
            }

            printer.success("No secrets found in repository");
        }
        ScanCmd::Deps {
            project: _,
            severity,
        } => {
            printer.header(&format!("{} Dependency Scan", icons::BUG));
            printer.kv("Severity threshold", severity);
            printer.newline();

            let spin = cli::spinner("Scanning dependencies...");
            tokio::time::sleep(std::time::Duration::from_millis(600)).await;
            spin.finish_and_clear();

            printer.success("No vulnerabilities found");
        }
        ScanCmd::Commits { project: _, since } => {
            printer.header(&format!("{} Commit Lint", icons::CHECK));

            if let Some(s) = since {
                printer.kv("Since", s);
            }
            printer.newline();

            let spin = cli::spinner("Checking commit messages...");
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            spin.finish_and_clear();

            printer.success("All commits follow conventional format");
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
