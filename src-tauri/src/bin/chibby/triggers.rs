//! `chibby schedule` / `watch` / `hooks` / `triggers`.
//!
//! All of it runs on this machine. There is no daemon and nothing listens on a
//! socket: `--once` is meant to be driven by launchd or systemd, and the
//! foreground loop only lives as long as the terminal does.

use crate::args::{HookKindArg, HooksCmd, TriggersCmd};
use crate::cli::{icons, Printer};
use chibby_lib::engine::locks;
use chibby_lib::engine::trigger_state;
use chibby_lib::engine::triggers::hooks::{self, HookKind, HookState, InstallMode};
use chibby_lib::engine::triggers::runner::{TriggerAction, TriggerRunner};
use chibby_lib::engine::triggers::{
    self, runner, schedule, HookSpec, ScheduleTrigger, TriggersConfig, WatchTrigger,
};
use chrono::Utc;
use owo_colors::OwoColorize;
use std::path::PathBuf;

impl From<HookKindArg> for HookKind {
    fn from(arg: HookKindArg) -> Self {
        match arg {
            HookKindArg::PrePush => HookKind::PrePush,
            HookKindArg::PreCommit => HookKind::PreCommit,
        }
    }
}

// ---------------------------------------------------------------------------
// chibby schedule
// ---------------------------------------------------------------------------

pub(crate) async fn handle_schedule(
    printer: &Printer,
    project: Option<&PathBuf>,
    once: bool,
    dry_run: bool,
    count: usize,
) -> anyhow::Result<()> {
    let path = crate::project_path(project);
    printer.header(&format!("{} Scheduled Triggers", icons::CLOCK));
    printer.kv("Project", &path.display().to_string());

    if dry_run {
        return print_next_runs(printer, &path, count);
    }

    let runner = TriggerRunner::new().with_repos(vec![path.clone()]);

    if once {
        let outcomes = runner.tick_once().await;
        if outcomes.is_empty() {
            printer.info("Nothing due.");
            return Ok(());
        }
        for outcome in &outcomes {
            print_outcome(printer, &outcome.trigger_id, &outcome.action);
        }
        return Ok(());
    }

    printer.info(&format!(
        "Ticking every {}s. Ctrl-C to stop.",
        runner::TICK_INTERVAL.as_secs()
    ));
    runner.run_forever().await;
    Ok(())
}

fn print_outcome(printer: &Printer, trigger_id: &str, action: &TriggerAction) {
    match action {
        TriggerAction::Fired { run_id, status } => {
            printer.kv(trigger_id, &format!("{status:?} — run {run_id}"))
        }
        TriggerAction::Skipped { reason } => printer.kv(trigger_id, &format!("skipped — {reason}")),
        TriggerAction::Failed { error } => printer.error(&format!("{trigger_id}: {error}")),
    }
}

fn print_next_runs(printer: &Printer, path: &std::path::Path, count: usize) -> anyhow::Result<()> {
    let config = triggers::load_triggers_layered(path)?;
    if config.schedules.is_empty() {
        printer.info("No schedules configured in .chibby/triggers.toml.");
        return Ok(());
    }
    if !config.enabled {
        printer.warn("Triggers are disabled for this project (enabled = false).");
    }

    printer.newline();
    for trig in &config.schedules {
        printer.subheader(&format!("{} ({})", trig.id, trig.cron));
        match schedule::next_run_times(&trig.cron, Utc::now(), count) {
            Ok(times) => {
                for time in times {
                    println!("     {}", time.to_rfc3339().bright_black());
                }
            }
            Err(e) => printer.error(&e.to_string()),
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// chibby watch
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_watch(
    printer: &Printer,
    project: Option<&PathBuf>,
    stages: &[String],
    env: Option<&str>,
    include: &[String],
    debounce_ms: u64,
    min_interval_secs: u64,
) -> anyhow::Result<()> {
    let path = crate::project_path(project);
    let path = path.canonicalize().unwrap_or(path);

    printer.header(&format!("{} Watching", icons::GEAR));
    printer.kv("Project", &path.display().to_string());
    if !include.is_empty() {
        printer.kv("Include", &include.join(", "));
    }
    if !stages.is_empty() {
        printer.kv("Stages", &stages.join(", "));
    }
    printer.info("Ctrl-C to stop.");
    printer.newline();

    // Ad-hoc watch: built from flags, never written to triggers.toml.
    let trigger = WatchTrigger {
        id: "cli".to_string(),
        enabled: true,
        include: include.to_vec(),
        exclude: Vec::new(),
        debounce_ms,
        min_interval_secs,
        pipeline_file: None,
        environment: env.map(str::to_string),
        stages: stages.to_vec(),
    };

    // The watcher runs headless; without this the terminal would sit silent
    // while runs come and go.
    let reporter: runner::RunReporter = std::sync::Arc::new(|id: &str, action: &TriggerAction| {
        println!(
            "  {} {}",
            icons::RUNNING.blue().bold(),
            describe(id, action)
        );
    });

    runner::watch_with(path, vec![trigger], None, Some(reporter)).await
}

/// One-line summary of a triggered run.
fn describe(trigger_id: &str, action: &TriggerAction) -> String {
    match action {
        TriggerAction::Fired { run_id, status } => {
            format!("{trigger_id}: {status:?} — run {run_id}")
        }
        TriggerAction::Skipped { reason } => format!("{trigger_id}: skipped — {reason}"),
        TriggerAction::Failed { error } => format!("{trigger_id}: failed — {error}"),
    }
}

// ---------------------------------------------------------------------------
// chibby hooks
// ---------------------------------------------------------------------------

pub(crate) async fn handle_hooks(printer: &Printer, cmd: &HooksCmd) -> anyhow::Result<()> {
    match cmd {
        HooksCmd::Install {
            kind,
            project,
            stage,
            env,
            force,
            append,
            non_blocking,
        } => {
            let path = crate::project_path(project.as_ref());
            let path = path.canonicalize().unwrap_or(path);
            let mode = match (force, append) {
                (true, _) => InstallMode::Force,
                (_, true) => InstallMode::Append,
                _ => InstallMode::Safe,
            };
            let spec = HookSpec {
                stages: stage.clone(),
                pipeline_file: None,
                environment: env.clone(),
                blocking: !non_blocking,
            };

            let report = hooks::install(&path, (*kind).into(), &spec, mode)?;

            if !report.installed {
                printer.warn(&report.message);
                printer.newline();
                println!("{}", report.snippet);
                return Ok(());
            }
            printer.success(&format!("{} → {}", report.message, report.path.display()));
            if let Some(backup) = report.backup_path {
                printer.info(&format!("Previous hook saved to {}", backup.display()));
            }
        }
        HooksCmd::Uninstall { kind, project } => {
            let path = crate::project_path(project.as_ref());
            hooks::uninstall(&path, (*kind).into())?;
            printer.success(&format!("Removed Chibby's {} block", kind_label(*kind)));
        }
        HooksCmd::Status { project } => {
            let path = crate::project_path(project.as_ref());
            printer.header(&format!("{} Git Hooks", icons::SHIELD));
            printer.kv("Project", &path.display().to_string());
            printer.newline();
            for kind in [HookKind::PrePush, HookKind::PreCommit] {
                printer.kv(kind.file_name(), state_label(hooks::status(&path, kind)?));
            }
        }
    }
    Ok(())
}

fn kind_label(kind: HookKindArg) -> &'static str {
    HookKind::from(kind).file_name()
}

fn state_label(state: HookState) -> &'static str {
    match state {
        HookState::NotInstalled => "not installed",
        HookState::ChibbyManaged => "managed by Chibby",
        HookState::Foreign => "present, not Chibby's (use --append or --force)",
        HookState::ForeignWithChibbyBlock => "present, with a Chibby block",
    }
}

// ---------------------------------------------------------------------------
// chibby triggers
// ---------------------------------------------------------------------------

pub(crate) async fn handle_triggers(printer: &Printer, cmd: &TriggersCmd) -> anyhow::Result<()> {
    match cmd {
        TriggersCmd::List { project } => {
            let path = crate::project_path(project.as_ref());
            let config = triggers::load_triggers_layered(&path)?;
            printer.header(&format!("{} Triggers", icons::CLOCK));
            printer.kv("Project", &path.display().to_string());
            printer.kv("Enabled", if config.enabled { "yes" } else { "no" });
            printer.newline();
            list_triggers(printer, &path.to_string_lossy(), &config)?;
        }
        TriggersCmd::Enable { id, project } => {
            set_enabled(printer, project.as_ref(), id, true)?;
        }
        TriggersCmd::Disable { id, project } => {
            set_enabled(printer, project.as_ref(), id, false)?;
        }
        TriggersCmd::Next { project, count } => {
            let path = crate::project_path(project.as_ref());
            printer.header(&format!("{} Next Runs", icons::CLOCK));
            print_next_runs(printer, &path, *count)?;
        }
    }
    Ok(())
}

fn list_triggers(
    printer: &Printer,
    repo_path: &str,
    config: &TriggersConfig,
) -> anyhow::Result<()> {
    let state = trigger_state::trigger_state_for_repo(repo_path)?;

    if config.schedules.is_empty() && config.watches.is_empty() {
        printer.info("No triggers configured. Add them to .chibby/triggers.toml.");
        return Ok(());
    }

    for trig in &config.schedules {
        printer.subheader(&format!("schedule: {}", trig.id));
        printer.kv("Cron", &trig.cron);
        printer.kv("Enabled", if trig.enabled { "yes" } else { "no" });
        print_last(printer, state.get(&trig.id));
    }
    for trig in &config.watches {
        printer.subheader(&format!("watch: {}", trig.id));
        printer.kv(
            "Include",
            &if trig.include.is_empty() {
                "everything".to_string()
            } else {
                trig.include.join(", ")
            },
        );
        printer.kv("Enabled", if trig.enabled { "yes" } else { "no" });
        print_last(printer, state.get(&trig.id));
    }
    Ok(())
}

fn print_last(printer: &Printer, entry: Option<&trigger_state::TriggerStateEntry>) {
    let Some(entry) = entry else {
        printer.kv("Last fired", "never");
        return;
    };
    printer.kv(
        "Last fired",
        &entry
            .last_fired_at
            .map(|t| t.to_rfc3339())
            .unwrap_or_else(|| "never".to_string()),
    );
    if let Some(run_id) = &entry.last_run_id {
        printer.kv("Last run", run_id);
    }
    if let Some(reason) = &entry.last_skip_reason {
        printer.kv("Last skip", reason);
    }
}

/// Flip a trigger's `enabled` flag.
///
/// Written to `triggers.local.toml`: whether a trigger runs is per-machine
/// policy, and the committed file belongs to the team.
fn set_enabled(
    printer: &Printer,
    project: Option<&PathBuf>,
    id: &str,
    enabled: bool,
) -> anyhow::Result<()> {
    let path = crate::project_path(project);
    let merged = triggers::load_triggers_layered(&path)?;
    let mut local = triggers::load_triggers_local(&path)?;

    if let Some(trig) = merged.schedules.iter().find(|s| s.id == id) {
        let mut updated: ScheduleTrigger = trig.clone();
        updated.enabled = enabled;
        upsert(&mut local.schedules, updated, |s| &s.id);
    } else if let Some(trig) = merged.watches.iter().find(|w| w.id == id) {
        let mut updated: WatchTrigger = trig.clone();
        updated.enabled = enabled;
        upsert(&mut local.watches, updated, |w| &w.id);
    } else {
        anyhow::bail!("No trigger '{id}' in {}", path.display());
    }

    local.enabled = merged.enabled || enabled;
    triggers::save_triggers_local(&path, &local)?;

    printer.success(&format!(
        "{} '{id}' in .chibby/triggers.local.toml",
        if enabled { "Enabled" } else { "Disabled" }
    ));
    Ok(())
}

/// Replace an entry with the same id, or append it.
fn upsert<T, F>(items: &mut Vec<T>, value: T, id_of: F)
where
    F: Fn(&T) -> &String,
{
    match items.iter().position(|item| id_of(item) == id_of(&value)) {
        Some(index) => items[index] = value,
        None => items.push(value),
    }
}

// ---------------------------------------------------------------------------
// chibby cancel
// ---------------------------------------------------------------------------

/// Cancel whichever process is running this repo.
///
/// The run lock records the owning pid, so the cancel flag reaches a run
/// started by the desktop app or a scheduled trigger — not just this terminal.
pub(crate) fn cancel_run(printer: &Printer, project: Option<&PathBuf>) -> anyhow::Result<()> {
    let path = crate::project_path(project);
    let path = path.canonicalize().unwrap_or(path);
    let repo_path = path.to_string_lossy().to_string();

    printer.header(&format!("{} Cancel Pipeline", icons::WARN));
    printer.kv("Project", &repo_path);

    if !locks::request_cancel(&repo_path)? {
        printer.info("No run is in progress for this project.");
        return Ok(());
    }

    let pid = locks::current_holder(&repo_path)?
        .map(|h| h.pid.to_string())
        .unwrap_or_else(|| "?".to_string());
    printer.success(&format!("Cancellation requested (pid {pid})"));
    printer.info("The run stops after the current command and records a cancelled run.");
    Ok(())
}
