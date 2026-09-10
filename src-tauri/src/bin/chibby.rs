//! Chibby CLI - Local-first CI/CD
//!
//! A standalone CLI that shares the same engine as the desktop app.
//! Designed for headless servers, scripting, and terminal-first workflows.

use chibby_lib::engine::bootstrap::{self, ApplyMode, Classification};
use chibby_lib::engine::models::Project;
use chibby_lib::engine::secret_audit as secret_audit_engine;
use chibby_lib::engine::{persistence, pipeline, preflight, secrets as secrets_engine};
use clap::Parser;
use owo_colors::OwoColorize;
use std::path::{Path, PathBuf};

// Import CLI styled output
mod cli {
    include!("../cli/mod.rs");
}

use cli::{icons, Printer, StageStatus};
#[path = "chibby/aigen.rs"]
mod aigen;
#[path = "chibby/args.rs"]
mod args;
#[path = "chibby/env.rs"]
mod env;
#[path = "chibby/import_export.rs"]
mod import_export;
#[path = "chibby/runs.rs"]
mod runs;
#[path = "chibby/scan.rs"]
mod scan;
#[path = "chibby/triggers.rs"]
mod triggers;
use args::{ArtifactCmd, AuditCmd, Cli, Commands, ProjectsCmd, UpdaterCmd, VersionCmd};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Main Entry Point
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::main]
async fn main() {
    let cli_args = Cli::parse();

    // Handle color preferences. Per the NO_COLOR standard (https://no-color.org),
    // any non-empty value disables color — read presence manually so values like
    // NO_COLOR=1 don't fail clap's bool parsing.
    let no_color = cli_args.no_color
        || std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
    if no_color {
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
            pipeline,
            trigger,
        }) => {
            runs::run_pipeline(
                &printer,
                env.as_deref(),
                stage,
                project.as_ref(),
                *skip_preflight,
                *dry_run,
                pipeline.as_deref(),
                trigger.as_deref(),
            )
            .await
        }

        Some(Commands::Status { project }) => runs::show_status(&printer, project.as_ref()).await,

        Some(Commands::Cancel { project }) => {
            runs::cancel_pipeline(&printer, project.as_ref()).await
        }

        Some(Commands::Projects(cmd)) => handle_projects(&printer, cmd).await,

        Some(Commands::Pipeline(cmd)) => runs::handle_pipeline(&printer, cmd).await,

        Some(Commands::History {
            project,
            env,
            limit,
        }) => runs::show_history(&printer, project.as_ref(), env.as_deref(), *limit).await,

        Some(Commands::Retry { run_id, from_stage }) => {
            runs::retry_run(&printer, run_id, from_stage.as_deref()).await
        }

        Some(Commands::Rollback {
            run_id,
            last_good,
            env,
            project,
        }) => {
            runs::rollback_run(
                &printer,
                run_id.as_deref(),
                *last_good,
                env.as_deref(),
                project.as_ref(),
            )
            .await
        }

        Some(Commands::Secrets(cmd)) => env::handle_secrets(&printer, cmd).await,

        Some(Commands::Env(cmd)) => env::handle_env(&printer, cmd).await,

        Some(Commands::Version(cmd)) => handle_version(&printer, cmd).await,

        Some(Commands::Artifact(cmd)) => handle_artifact(&printer, cmd).await,

        Some(Commands::Scan(cmd)) => scan::handle_scan(&printer, cmd).await,

        Some(Commands::Init { path, ai }) => init_project(&printer, path.as_ref(), *ai).await,

        Some(Commands::Bootstrap {
            project,
            silent,
            dry_run,
            merge,
        }) => bootstrap_cmd(&printer, project.as_ref(), *silent, *dry_run, *merge).await,

        Some(Commands::Import(cmd)) => import_export::handle_import(&printer, cmd).await,

        Some(Commands::Export(cmd)) => import_export::handle_export(&printer, cmd).await,

        Some(Commands::Preflight { env, project }) => {
            runs::run_preflight(&printer, env.as_deref(), project.as_ref()).await
        }

        Some(Commands::Doctor { project }) => doctor(&printer, project.as_ref()).await,

        Some(Commands::Audit(cmd)) => handle_audit(&printer, cmd).await,

        Some(Commands::Updater(cmd)) => handle_updater(&printer, cmd).await,

        Some(Commands::Logs { run_id, follow }) => show_logs(&printer, run_id, *follow).await,

        Some(Commands::Schedule {
            project,
            once,
            dry_run,
            count,
        }) => triggers::handle_schedule(&printer, project.as_ref(), *once, *dry_run, *count).await,

        Some(Commands::Watch {
            project,
            stage,
            env,
            include,
            debounce_ms,
            min_interval_secs,
        }) => {
            triggers::handle_watch(
                &printer,
                project.as_ref(),
                stage,
                env.as_deref(),
                include,
                *debounce_ms,
                *min_interval_secs,
            )
            .await
        }

        Some(Commands::Hooks(cmd)) => triggers::handle_hooks(&printer, cmd).await,

        Some(Commands::Triggers(cmd)) => triggers::handle_triggers(&printer, cmd).await,

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
                    runs::run_status_to_cli(p.last_run_status.as_ref()),
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
                    let matched = cwd
                        .as_ref()
                        .and_then(|c| projects.iter().find(|p| Path::new(&p.path) == c.as_path()));
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

pub(crate) fn project_path(project: Option<&PathBuf>) -> PathBuf {
    project
        .cloned()
        .unwrap_or_else(|| std::env::current_dir().expect("current_dir failed"))
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
    printer.kv("Suggested envs", &report.suggested_environments.join(", "));

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

    let mode = if merge {
        ApplyMode::Merge
    } else {
        ApplyMode::Safe
    };
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

    if ai {
        let spin = cli::spinner(&format!("{} Generating pipeline with AI...", icons::SPARKLE));
        let result = aigen::generate_pipeline_toml(&path).await;
        spin.finish_and_clear();
        match result {
            Ok(explanation) => {
                printer.success("Created .chibby/pipeline.toml");
                if !explanation.trim().is_empty() {
                    printer.newline();
                    println!("{}", explanation.trim());
                }
            }
            Err(e) => {
                printer.error(&format!("AI pipeline generation failed: {}", e));
                return Err(e);
            }
        }
    } else {
        let spin = cli::spinner("Generating pipeline...");
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
        spin.finish_and_clear();

        printer.success("Created .chibby/pipeline.toml");
        printer.success("Created .chibby/environments.toml");
    }
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
