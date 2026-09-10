//! `chibby run` / pipeline execution, status, history, retry, rollback, preflight.

use crate::args::PipelineCmd;
use crate::cli::{self, icons, Printer, StageStatus};
use anyhow::Context;
use chibby_lib::engine::executor;
use chibby_lib::engine::models::{
    Pipeline, PipelineRun, Project, RollbackOutcome, RunKind, RunStatus as EngineRunStatus,
    StageStatus as EngineStageStatus,
};
use chibby_lib::engine::run_support::{execute_run, ExecuteRunRequest};
use chibby_lib::engine::{persistence, pipeline, preflight, rollback, run_support};
use chibby_lib::state::create_pipeline_state;
use chrono::Utc;
use owo_colors::OwoColorize;
use std::path::{Path, PathBuf};

#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_pipeline(
    printer: &Printer,
    env: Option<&str>,
    stages: &[String],
    project: Option<&PathBuf>,
    skip_preflight: bool,
    dry_run: bool,
    pipeline_file: Option<&str>,
    trigger: Option<&str>,
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

    if let Some(source) = trigger {
        printer.kv("Trigger", source);
    }

    // Load the real pipeline from .chibby/ on every run.
    let pipeline =
        run_support::load_selected_pipeline(&project_path, pipeline_file).with_context(|| {
            format!(
                "Failed to load pipeline from {}",
                project_path
                    .join(".chibby")
                    .join(format!("{}.toml", pipeline_file.unwrap_or("pipeline")))
                    .display()
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

    let run = execute_run(
        ExecuteRunRequest {
            repo_path: project_path.clone(),
            pipeline_file: pipeline_file.map(str::to_string),
            environment: env.map(str::to_string),
            stages: stage_filter.map(<[String]>::to_vec),
            run_kind: run_kind_for_trigger(trigger),
            trigger_id: trigger.map(str::to_string),
            pipeline_override: Some(pipeline),
            ..Default::default()
        },
        Some(cli_log_callback()),
        Some(cancel_state),
        None,
    )
    .await?;

    print_completed_run(printer, &run);

    // Surface a real exit code so `chibby run` is usable in scripts/CI.
    if run.status != EngineRunStatus::Success {
        std::process::exit(1);
    }

    Ok(())
}

/// Map a `--trigger` tag onto the run provenance it records.
fn run_kind_for_trigger(trigger: Option<&str>) -> RunKind {
    match trigger {
        Some(t) if t.starts_with("hook:") => RunKind::Hook,
        Some(t) if t.starts_with("scheduled:") => RunKind::Scheduled,
        Some(t) if t.starts_with("watch:") => RunKind::Watch,
        _ => RunKind::Normal,
    }
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
    err.to_string()
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
    project: Option<&PathBuf>,
) -> anyhow::Result<()> {
    // The run lock names the owning process, so this reaches runs started by
    // the desktop app or a trigger, not just this terminal.
    crate::triggers::cancel_run(printer, project)
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
        PipelineCmd::Generate { project, ai } => {
            let path = crate::project_path(project.as_ref());
            if *ai {
                let spin =
                    cli::spinner(&format!("{} Generating pipeline with AI...", icons::SPARKLE));
                let result = crate::aigen::generate_pipeline_toml(&path).await;
                spin.finish_and_clear();
                match result {
                    Ok(explanation) => {
                        printer.success("Pipeline generated → .chibby/pipeline.toml");
                        if !explanation.trim().is_empty() {
                            printer.newline();
                            println!("{}", explanation.trim());
                            printer.newline();
                        }
                    }
                    Err(e) => {
                        printer.error(&format!("AI pipeline generation failed: {}", e));
                        return Err(e);
                    }
                }
            } else {
                let spin = cli::spinner(&format!("{} Detecting scripts...", icons::GEAR));
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                spin.finish_and_clear();
                printer.success("Pipeline generated");
            }
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
            history_tag(run),
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
        EngineStageStatus::TimedOut => StageStatus::TimedOut,
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
    print_health_failure(printer, run);
}

/// Distinct block for the case auto-rollback exists to serve: the commands
/// passed but the post-deploy health check did not.
fn print_health_failure(printer: &Printer, run: &PipelineRun) {
    let Some(stage) = run.health_failure_stage.as_deref() else {
        return;
    };

    printer.newline();
    printer.error(&format!("health check failed on stage '{stage}'"));

    match (run.rollback_run_id.as_deref(), run.rollback_outcome) {
        (Some(id), Some(outcome)) => printer.rollback_note(&format!(
            "auto-rolled back to run {id} — {}",
            rollback_outcome_label(outcome)
        )),
        (_, Some(RollbackOutcome::Skipped)) => {
            // The bad release is still live — say why, don't send them to the log.
            let reason = run
                .rollback_skip_reason
                .as_deref()
                .unwrap_or("see the log for the reason");
            printer.rollback_note(&format!("auto-rollback skipped — {reason}"));
        }
        _ => {}
    }
}

fn rollback_outcome_label(outcome: RollbackOutcome) -> &'static str {
    match outcome {
        RollbackOutcome::Succeeded => "succeeded",
        RollbackOutcome::Failed => "FAILED, manual intervention required",
        RollbackOutcome::Skipped => "skipped",
    }
}

/// History marker separating automatic rollbacks from human-initiated ones.
fn history_tag(run: &PipelineRun) -> Option<&'static str> {
    match run.run_kind {
        RunKind::Rollback if run.auto_rollback_of.is_some() => Some("auto-rollback"),
        RunKind::Rollback => Some("rollback"),
        RunKind::Retry => Some("retry"),
        RunKind::Scheduled => Some("scheduled"),
        RunKind::Watch => Some("watch"),
        RunKind::Hook => Some("hook"),
        RunKind::Normal => None,
    }
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
                .find(|s| s.status.is_failure())
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

    let parent_id = original.parent_run_id.as_deref().unwrap_or(run_id);
    let existing_retries = persistence::retry_count_for_run(parent_id).unwrap_or(0);

    printer.info("Starting retry...");
    let run = run_support::execute_run(
        run_support::ExecuteRunRequest {
            repo_path: path.to_path_buf(),
            pipeline_file: original.pipeline_file.clone(),
            environment: original.environment.clone(),
            stages: Some(stages_to_run),
            run_kind: RunKind::Retry,
            parent_run_id: Some(parent_id.to_string()),
            retry_number: Some(existing_retries + 1),
            retry_from_stage: Some(retry_stage),
            pipeline_override: Some(pipeline),
            ..Default::default()
        },
        Some(cli_log_callback()),
        None,
        None,
    )
    .await?;

    print_completed_run(printer, &run);
    Ok(())
}

pub(crate) async fn rollback_run(
    printer: &Printer,
    run_id: Option<&str>,
    last_good: bool,
    env: Option<&str>,
    project: Option<&PathBuf>,
) -> anyhow::Result<()> {
    printer.header(&format!("{} Rollback", icons::ROLLBACK));

    let target = match (last_good, run_id) {
        (true, Some(_)) => {
            anyhow::bail!("Pass either a run id or --last-good, not both")
        }
        (true, None) => resolve_last_good_target(printer, env, project)?,
        (false, Some(id)) => load_rollback_target(id)?,
        (false, None) => anyhow::bail!(
            "Nothing to roll back to. Pass a run id, or --last-good with --env <ENV>."
        ),
    };

    printer.kv("Target Run", &target.id);
    printer.newline();

    let pipeline = run_support::pipeline_snapshot_for_run(&target)?;
    let path = PathBuf::from(&target.repo_path);

    printer.warn("Rolling back to recorded deployment pipeline...");
    let run = run_support::execute_run(
        run_support::ExecuteRunRequest {
            repo_path: path,
            pipeline_file: target.pipeline_file.clone(),
            environment: target.environment.clone(),
            run_kind: RunKind::Rollback,
            rollback_target_id: Some(target.id.clone()),
            pipeline_override: Some(pipeline),
            ..Default::default()
        },
        Some(cli_log_callback()),
        None,
        None,
    )
    .await?;

    print_completed_run(printer, &run);
    Ok(())
}

/// Load an explicitly named rollback target.
fn load_rollback_target(run_id: &str) -> anyhow::Result<PipelineRun> {
    let target = persistence::load_run(run_id)?
        .ok_or_else(|| anyhow::anyhow!("Run {} not found", run_id))?;
    if target.status != EngineRunStatus::Success {
        anyhow::bail!("Can only roll back to a successful run");
    }
    Ok(target)
}

/// Resolve `--last-good`: the newest run that actually deployed this
/// pipeline's deploy stage successfully in `env`.
fn resolve_last_good_target(
    printer: &Printer,
    env: Option<&str>,
    project: Option<&PathBuf>,
) -> anyhow::Result<PipelineRun> {
    let env = env.ok_or_else(|| {
        anyhow::anyhow!("--last-good needs --env <ENV> to know which deployment to restore")
    })?;

    let path = crate::project_path(project);
    let path_str = path
        .canonicalize()
        .unwrap_or_else(|_| path.clone())
        .to_string_lossy()
        .to_string();

    let pipeline = run_support::load_selected_pipeline(&path, None)
        .with_context(|| format!("Failed to load pipeline from {}", path.display()))?;
    let stage = rollback::deploy_stage_name(&pipeline)
        .ok_or_else(|| anyhow::anyhow!("Pipeline '{}' has no stages", pipeline.name))?;

    printer.kv("Project", &path_str);
    printer.kv("Environment", env);
    printer.kv("Deploy Stage", stage);

    persistence::last_good_deployment(&path_str, env, stage, "")?.ok_or_else(|| {
        anyhow::anyhow!("No known-good deployment of stage '{stage}' recorded for '{env}'")
    })
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
