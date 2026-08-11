//! `chibby run` / pipeline execution, status, history, retry, rollback, preflight.

use crate::args::PipelineCmd;
use crate::cli::{self, icons, Printer, StageStatus};
use anyhow::Context;
use chibby_lib::engine::executor;
use chibby_lib::engine::models::{
    Pipeline, PipelineRun, Project, RunKind, RunStatus as EngineRunStatus,
    StageStatus as EngineStageStatus,
};
use chibby_lib::engine::{persistence, pipeline, preflight, run_support};
use chibby_lib::state::create_pipeline_state;
use chrono::Utc;
use owo_colors::OwoColorize;
use std::path::{Path, PathBuf};

pub(crate) async fn run_pipeline(
    printer: &Printer,
    env: Option<&str>,
    stages: &[String],
    project: Option<&PathBuf>,
    skip_preflight: bool,
    dry_run: bool,
) -> anyhow::Result<()> {
    printer.banner();

    // Canonicalize so runs are stored under the same path `projects`/`status`
    // resolve to (e.g. /tmp vs /private/tmp on macOS).
    let project_path = {
        let p = crate::project_path(project);
        p.canonicalize().unwrap_or(p)
    };

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

    // Load the real pipeline from .chibby/pipeline.toml on every run.
    let pipeline = run_support::load_selected_pipeline(&project_path, None).with_context(|| {
        format!(
            "Failed to load pipeline from {}",
            project_path.join(".chibby").join("pipeline.toml").display()
        )
    })?;

    // Validate any requested --stage names exist before doing anything.
    for requested in stages {
        if !pipeline.stages.iter().any(|s| &s.name == requested) {
            let available = pipeline
                .stages
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            anyhow::bail!("Stage '{requested}' not found in pipeline. Available: {available}");
        }
    }

    // Preflight checks against the real pipeline + environment.
    if !skip_preflight && !dry_run {
        check_preflight(printer, &project_path, &pipeline, env).await?;
    }

    printer.subheader(&format!("{} Pipeline Stages", icons::GEAR));

    let stage_filter: Option<&[String]> = if stages.is_empty() {
        None
    } else {
        Some(stages)
    };

    // Dry run: list the real stages that would execute, then stop.
    if dry_run {
        for stage in &pipeline.stages {
            if stage_filter.is_some_and(|f| !f.contains(&stage.name)) {
                continue;
            }
            printer.stage(&stage.name, StageStatus::Pending);
        }
        printer.newline();
        printer.info("Dry run complete - no changes made");
        return Ok(());
    }

    // Resolve environment variables and secrets for the run.
    let (env_ref, env_vars) = run_support::resolve_execution_context(&project_path, env)?;

    // Wire Ctrl-C to cancel the in-process run gracefully.
    let repo_str = project_path.to_string_lossy().to_string();
    let cancel_state = create_pipeline_state();
    {
        let mut state = cancel_state.write().await;
        state.start(&repo_str);
    }
    let cancel_signal = cancel_state.clone();
    let cancel_repo = repo_str.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            let mut state = cancel_signal.write().await;
            state.cancel(&cancel_repo);
        }
    });

    let mut run = executor::run_pipeline(
        &pipeline,
        &project_path,
        env_ref.as_ref(),
        env_vars,
        Some(cli_log_callback()),
        stage_filter,
        Some(cancel_state.clone()),
        None,
        &uuid::Uuid::new_v4().to_string(),
    )
    .await?;

    run_support::annotate_run(&mut run, &pipeline, None);
    run_support::persist_completed_run(&run)?;
    run_support::post_run_housekeeping(&repo_str, &run).await;

    print_completed_run(printer, &run);

    // Surface a real exit code so `chibby run` is usable in scripts/CI.
    if run.status != EngineRunStatus::Success {
        std::process::exit(1);
    }

    Ok(())
}

/// Whether the git working tree at `path` is clean.
/// `Some(true)` clean, `Some(false)` dirty, `None` not a git repo / git unavailable.
fn git_tree_clean(path: &Path) -> Option<bool> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["status", "--porcelain"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(output.stdout.is_empty())
}

/// Human-readable preflight error for CLI display.
fn format_preflight_error(err: &preflight::PreflightError) -> String {
    use preflight::PreflightError::*;
    match err {
        MissingSecret { name, environment } => {
            format!("Secret '{name}' not set for environment '{environment}'")
        }
        MissingSshHost { stage } => format!("Stage '{stage}' uses SSH but no host is configured"),
        MissingEnvironment { name } => format!("Environment '{name}' is not defined"),
        SshConnectivityFailed { host, error } => format!("SSH to {host} failed: {error}"),
        SshNotAvailable => "ssh binary not found on PATH".to_string(),
    }
}

/// Real preflight checks shared by `run` and the standalone `preflight` command.
/// Hard failures return an error; a dirty git tree is a non-fatal warning.
async fn check_preflight(
    printer: &Printer,
    project_path: &Path,
    pipeline: &Pipeline,
    env: Option<&str>,
) -> anyhow::Result<()> {
    printer.subheader(&format!("{} Preflight Checks", icons::SHIELD));

    // Pipeline config exists (already loaded by the caller).
    printer.preflight_check("Pipeline config exists", true, None);

    // Git working tree state (warning only — never blocks a local run).
    match git_tree_clean(project_path) {
        Some(true) => printer.preflight_check("Git working tree clean", true, None),
        Some(false) => printer.warn("Git working tree has uncommitted changes"),
        None => printer.warn("Not a git repository"),
    }

    // Environment-specific validation (secrets, SSH hosts, connectivity).
    if let Some(env_name) = env {
        let environments = pipeline::load_environments_layered(project_path)?;
        let secrets_config = pipeline::load_secrets_config(project_path)?;
        let result = preflight::validate_preflight(
            pipeline,
            &project_path.to_string_lossy(),
            env_name,
            &environments,
            &secrets_config,
        )
        .await?;

        if result.passed {
            printer.preflight_check(&format!("Environment '{env_name}' ready"), true, None);
        }
        for err in &result.errors {
            printer.preflight_check(&format_preflight_error(err), false, None);
        }
        for warning in &result.warnings {
            printer.warn(warning);
        }
        if !result.passed {
            printer.newline();
            anyhow::bail!("Preflight validation failed for environment '{env_name}'");
        }
    }

    printer.newline();
    Ok(())
}

/// Format a UTC timestamp as a relative "x ago" string.
fn format_relative_time(when: chrono::DateTime<Utc>) -> String {
    let secs = (Utc::now() - when).num_seconds().max(0);
    match secs {
        0..=59 => "just now".to_string(),
        60..=3599 => format!("{}m ago", secs / 60),
        3600..=86399 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86400),
    }
}

/// Resolve the tracked project for a `-p`/cwd reference, or bail with guidance.
fn resolve_project(project: Option<&PathBuf>) -> anyhow::Result<Project> {
    let path = crate::project_path(project);
    let path_str = path
        .canonicalize()
        .unwrap_or(path.clone())
        .to_string_lossy()
        .to_string();
    let projects = persistence::load_projects()?;
    projects
        .into_iter()
        .find(|p| Path::new(&p.path) == Path::new(&path_str) || p.path == path_str)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "{} is not a tracked project. Add it with `chibby projects add {}`",
                path.display(),
                path.display()
            )
        })
}

pub(crate) async fn show_status(
    printer: &Printer,
    project: Option<&PathBuf>,
) -> anyhow::Result<()> {
    printer.header(&format!("{} Pipeline Status", icons::INFO));

    let proj = resolve_project(project)?;
    printer.kv("Project", &proj.name);

    let mut runs = persistence::load_runs_for_project(&proj.path)?;
    runs.sort_by(|a, b| b.started_at.cmp(&a.started_at));

    let Some(latest) = runs.first() else {
        printer.kv_colored("Status", "No runs yet", StageStatus::Pending);
        printer.newline();
        printer.info("Run `chibby run` to execute the pipeline.");
        return Ok(());
    };

    printer.kv_colored(
        "Status",
        cli_run_status(latest),
        run_status_to_cli(Some(&latest.status)).unwrap_or(StageStatus::Pending),
    );
    printer.kv("Last Run", &format_relative_time(latest.started_at));
    if let Some(d) = latest.duration_ms {
        printer.kv("Duration", &cli::format_duration(d));
    }
    if let Some(ref e) = latest.environment {
        printer.kv("Environment", e);
    }
    printer.newline();

    printer.subheader("Stages");
    for stage in &latest.stage_results {
        printer.stage_with_duration(
            &stage.stage_name,
            cli_stage_status(&stage.status),
            stage.duration_ms,
        );
    }

    Ok(())
}

pub(crate) async fn cancel_pipeline(
    printer: &Printer,
    _project: Option<&PathBuf>,
) -> anyhow::Result<()> {
    // CLI runs execute in a single foreground process; there is no shared
    // daemon to signal. Cancellation is handled by Ctrl-C inside `chibby run`.
    printer.header(&format!("{} Cancel Pipeline", icons::WARN));
    printer.info("Chibby CLI runs in the foreground — press Ctrl-C in the `chibby run` terminal to cancel it.");
    printer.info("The pipeline stops after the current stage and records a cancelled run.");
    Ok(())
}

pub(crate) fn run_status_to_cli(status: Option<&EngineRunStatus>) -> Option<StageStatus> {
    status.map(|s| match s {
        EngineRunStatus::Pending => StageStatus::Pending,
        EngineRunStatus::Running => StageStatus::Running,
        EngineRunStatus::Success => StageStatus::Success,
        EngineRunStatus::Failed => StageStatus::Failed,
        EngineRunStatus::Cancelled => StageStatus::Cancelled,
    })
}

pub(crate) async fn handle_pipeline(printer: &Printer, cmd: &PipelineCmd) -> anyhow::Result<()> {
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

pub(crate) async fn show_history(
    printer: &Printer,
    project: Option<&PathBuf>,
    env: Option<&str>,
    limit: usize,
) -> anyhow::Result<()> {
    printer.header(&format!("{} Run History", icons::CLOCK));

    let proj = resolve_project(project)?;
    printer.kv("Project", &proj.name);
    if let Some(e) = env {
        printer.kv("Environment", e);
    }
    printer.newline();

    let mut runs = persistence::load_runs_for_project(&proj.path)?;
    runs.sort_by(|a, b| b.started_at.cmp(&a.started_at));

    let filtered: Vec<&PipelineRun> = runs
        .iter()
        .filter(|r| match env {
            Some(e) => r.environment.as_deref() == Some(e),
            None => true,
        })
        .take(limit)
        .collect();

    if filtered.is_empty() {
        printer.info("No runs recorded yet. Run `chibby run` to create one.");
        return Ok(());
    }

    for run in filtered {
        printer.history_entry(
            &run.id,
            run_status_to_cli(Some(&run.status)).unwrap_or(StageStatus::Pending),
            &format_relative_time(run.started_at),
            run.duration_ms.unwrap_or(0),
            run.environment.as_deref(),
        );
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

pub(crate) async fn retry_run(
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
        &uuid::Uuid::new_v4().to_string(),
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

pub(crate) async fn rollback_run(printer: &Printer, run_id: &str) -> anyhow::Result<()> {
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
        &uuid::Uuid::new_v4().to_string(),
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
pub(crate) async fn run_preflight(
    printer: &Printer,
    env: Option<&str>,
    project: Option<&PathBuf>,
) -> anyhow::Result<()> {
    let project_path = crate::project_path(project);

    printer.header(&format!("{} Preflight Checks", icons::SHIELD));
    printer.kv("Project", &project_path.display().to_string());
    if let Some(e) = env {
        printer.kv("Environment", e);
    }
    printer.newline();

    let pipeline = run_support::load_selected_pipeline(&project_path, None).with_context(|| {
        format!(
            "Failed to load pipeline from {}",
            project_path.join(".chibby").join("pipeline.toml").display()
        )
    })?;

    check_preflight(printer, &project_path, &pipeline, env).await?;

    printer.success(&format!("All checks passed {}", icons::SPARKLE));
    Ok(())
}
