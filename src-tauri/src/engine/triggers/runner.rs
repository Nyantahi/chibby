//! The trigger runtime: schedule ticks and file watchers.
//!
//! Hosted by the desktop app (spawned at startup) and by the CLI
//! (`chibby schedule`, `chibby watch`). Nothing here listens on a socket —
//! every trigger is driven by this machine's clock or its filesystem.

use super::schedule::{self, Decision};
use super::watch::{self, Debounce, DebounceDecision};
use super::{ScheduleTrigger, TriggersConfig, WatchTrigger};
use crate::engine::models::{RunKind, RunStatus};
use crate::engine::run_support::{execute_run, ExecuteRunRequest};
use crate::engine::{locks, persistence, trigger_state};
use crate::state::SharedPipelineState;
use anyhow::Result;
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How often the scheduler re-reads config and re-evaluates cron expressions.
pub const TICK_INTERVAL: Duration = Duration::from_secs(30);

/// Longest a watch loop sleeps with nothing pending.
const WATCH_IDLE_POLL: Duration = Duration::from_secs(1);

/// What happened to one trigger during a tick.
#[derive(Debug, Clone, PartialEq)]
pub enum TriggerAction {
    Fired { run_id: String, status: RunStatus },
    Skipped { reason: String },
    Failed { error: String },
}

/// One line of a tick report.
#[derive(Debug, Clone)]
pub struct TriggerOutcome {
    pub repo_path: String,
    pub trigger_id: String,
    pub action: TriggerAction,
}

/// Owns the schedule ticks and the watchers, reconciling against config each tick.
pub struct TriggerRunner {
    /// Repos to drive; empty means every tracked project.
    repos: Vec<PathBuf>,
    /// The GUI's in-process run state, when hosted by the app.
    pipeline_state: Option<SharedPipelineState>,
    tick: Duration,
}

impl Default for TriggerRunner {
    fn default() -> Self {
        Self {
            repos: Vec::new(),
            pipeline_state: None,
            tick: TICK_INTERVAL,
        }
    }
}

impl TriggerRunner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_repos(mut self, repos: Vec<PathBuf>) -> Self {
        self.repos = repos;
        self
    }

    pub fn with_pipeline_state(mut self, state: SharedPipelineState) -> Self {
        self.pipeline_state = Some(state);
        self
    }

    /// Evaluate every schedule once and fire what is due. Used by
    /// `chibby schedule --once` (launchd/systemd) and by each loop tick.
    pub async fn tick_once(&self) -> Vec<TriggerOutcome> {
        let mut outcomes = Vec::new();
        for repo in self.target_repos() {
            let Ok(config) = load_active_config(&repo) else {
                continue;
            };
            for trig in &config.schedules {
                if let Some(outcome) = self.evaluate_schedule(&repo, trig).await {
                    outcomes.push(outcome);
                }
            }
        }
        outcomes
    }

    /// Tick forever. Also starts one watch loop per repo that has watches.
    pub async fn run_forever(self) {
        for repo in self.target_repos() {
            let Ok(config) = load_active_config(&repo) else {
                continue;
            };
            if config.watches.iter().any(|w| w.enabled) {
                let state = self.pipeline_state.clone();
                tokio::spawn(async move {
                    if let Err(e) = watch_loop(repo.clone(), state).await {
                        log::warn!("[triggers] watch loop for {} ended: {e}", repo.display());
                    }
                });
            }
        }

        loop {
            for outcome in self.tick_once().await {
                log::info!(
                    "[triggers] {} / {}: {:?}",
                    outcome.repo_path,
                    outcome.trigger_id,
                    outcome.action
                );
            }
            tokio::time::sleep(self.tick).await;
        }
    }

    fn target_repos(&self) -> Vec<PathBuf> {
        if !self.repos.is_empty() {
            return self.repos.clone();
        }
        persistence::load_projects()
            .unwrap_or_default()
            .into_iter()
            .map(|p| PathBuf::from(p.path))
            .collect()
    }

    /// Decide and act on one schedule. `None` when it simply is not due.
    async fn evaluate_schedule(
        &self,
        repo: &Path,
        trig: &ScheduleTrigger,
    ) -> Option<TriggerOutcome> {
        let repo_path = repo.to_string_lossy().to_string();
        let now = Utc::now();
        let last_fired = trigger_state::get_trigger_state(&repo_path, &trig.id)
            .ok()
            .flatten()
            .and_then(|s| s.last_fired_at);

        let outcome = |action| {
            Some(TriggerOutcome {
                repo_path: repo_path.clone(),
                trigger_id: trig.id.clone(),
                action,
            })
        };

        match schedule::due_now(trig, last_fired, now) {
            Decision::NotDue => None,
            Decision::Skip { reason } => {
                let _ = trigger_state::record_skip(&repo_path, &trig.id, now, &reason);
                outcome(TriggerAction::Skipped { reason })
            }
            Decision::Fire { scheduled_for } => {
                // Skip, never queue: piling a nightly up behind a stuck run is
                // how a laptop ends up running four deploys at 3am.
                if let Some(reason) = busy_reason(&repo_path, self.pipeline_state.as_ref()).await {
                    let _ = trigger_state::record_skip(&repo_path, &trig.id, now, &reason);
                    return outcome(TriggerAction::Skipped { reason });
                }

                // Persist the fire time *before* running: a crash mid-run must
                // not re-fire the same occurrence on restart.
                let _ = trigger_state::record_fired(&repo_path, &trig.id, scheduled_for);

                let request = ExecuteRunRequest {
                    repo_path: repo.to_path_buf(),
                    pipeline_file: trig.pipeline_file.clone(),
                    environment: trig.environment.clone(),
                    stages: stage_filter(&trig.stages),
                    run_kind: RunKind::Scheduled,
                    trigger_id: Some(format!("scheduled:{}", trig.id)),
                    ..Default::default()
                };
                outcome(fire(&repo_path, &trig.id, request).await)
            }
        }
    }
}

/// Load a repo's triggers, or an empty config when triggers are switched off.
fn load_active_config(repo: &Path) -> Result<TriggersConfig> {
    let config = super::load_triggers_layered(repo)?;
    if !config.enabled {
        return Ok(TriggersConfig::default());
    }
    Ok(config)
}

/// Empty stage lists mean "the whole pipeline".
fn stage_filter(stages: &[String]) -> Option<Vec<String>> {
    (!stages.is_empty()).then(|| stages.to_vec())
}

/// Why this repo cannot start a run right now, if it cannot.
async fn busy_reason(
    repo_path: &str,
    pipeline_state: Option<&SharedPipelineState>,
) -> Option<String> {
    if let Some(state) = pipeline_state {
        if state.read().await.is_running(repo_path) {
            return Some("a run is already in progress in this process".to_string());
        }
    }
    let holder = locks::current_holder(repo_path).ok().flatten()?;
    Some(format!("a run is already in progress (pid {})", holder.pid))
}

/// Execute a triggered run and record the resulting run id.
async fn fire(repo_path: &str, trigger_id: &str, request: ExecuteRunRequest) -> TriggerAction {
    match execute_run(request, None, None, None).await {
        Ok(run) => {
            let _ = trigger_state::record_run_id(repo_path, trigger_id, &run.id);
            TriggerAction::Fired {
                run_id: run.id,
                status: run.status,
            }
        }
        Err(e) => TriggerAction::Failed {
            error: e.to_string(),
        },
    }
}

/// Run a configured trigger right now, ignoring its schedule.
///
/// Powers the UI's "run now" button. Still honours the run lock: firing by
/// hand must not double-run a repo either.
pub async fn fire_trigger_now(
    repo: &Path,
    trigger_id: &str,
) -> Result<crate::engine::models::PipelineRun> {
    let repo_path = repo.to_string_lossy().to_string();
    let config = super::load_triggers_layered(repo)?;

    let request = if let Some(trig) = config.schedules.iter().find(|s| s.id == trigger_id) {
        ExecuteRunRequest {
            repo_path: repo.to_path_buf(),
            pipeline_file: trig.pipeline_file.clone(),
            environment: trig.environment.clone(),
            stages: stage_filter(&trig.stages),
            run_kind: RunKind::Scheduled,
            trigger_id: Some(format!("scheduled:{trigger_id}")),
            ..Default::default()
        }
    } else if let Some(trig) = config.watches.iter().find(|w| w.id == trigger_id) {
        ExecuteRunRequest {
            repo_path: repo.to_path_buf(),
            pipeline_file: trig.pipeline_file.clone(),
            environment: trig.environment.clone(),
            stages: stage_filter(&trig.stages),
            run_kind: RunKind::Watch,
            trigger_id: Some(format!("watch:{trigger_id}")),
            ..Default::default()
        }
    } else {
        anyhow::bail!("No trigger '{trigger_id}' in {}", repo.display());
    };

    trigger_state::record_fired(&repo_path, trigger_id, Utc::now())?;
    let run = execute_run(request, None, None, None).await?;
    let _ = trigger_state::record_run_id(&repo_path, trigger_id, &run.id);
    Ok(run)
}

// ---------------------------------------------------------------------------
// File watching
// ---------------------------------------------------------------------------

/// Notified after each watch-triggered run, so a foreground `chibby watch`
/// can print something. The GUI passes `None`.
pub type RunReporter = std::sync::Arc<dyn Fn(&str, &TriggerAction) + Send + Sync>;

/// Watch a repo and run its watch triggers. Runs until the watcher dies.
pub async fn watch_loop(repo: PathBuf, pipeline_state: Option<SharedPipelineState>) -> Result<()> {
    let config = load_active_config(&repo)?;
    let triggers: Vec<WatchTrigger> = config.watches.into_iter().filter(|w| w.enabled).collect();
    if triggers.is_empty() {
        anyhow::bail!("No enabled watch triggers in {}", repo.display());
    }
    watch_with(repo, triggers, pipeline_state, None).await
}

/// Watch a repo against an explicit trigger list — also powers the ad-hoc
/// `chibby watch`, which never touches triggers.toml.
pub async fn watch_with(
    repo: PathBuf,
    triggers: Vec<WatchTrigger>,
    pipeline_state: Option<SharedPipelineState>,
    on_run: Option<RunReporter>,
) -> Result<()> {
    let repo_path = repo.to_string_lossy().to_string();
    // Held for the life of the loop: dropping the watcher stops the watch.
    let (_watcher, mut rx) = watch::watch_repo(&repo)?;
    let started = Instant::now();
    let mut debouncers: Vec<Debounce> = triggers.iter().map(Debounce::from_trigger).collect();

    log::info!(
        "[triggers] watching {} for {} trigger(s)",
        repo.display(),
        triggers.len()
    );

    loop {
        let wait = debouncers
            .iter()
            .filter_map(|d| match d.peek(elapsed_ms(started)) {
                DebounceDecision::Wait(ms) => Some(Duration::from_millis(ms)),
                _ => None,
            })
            .min()
            .unwrap_or(WATCH_IDLE_POLL);

        tokio::select! {
            event = rx.recv() => {
                let Some(path) = event else { return Ok(()) };
                let Some(rel) = watch::relative_to_repo(&repo, &path) else { continue };
                let now = elapsed_ms(started);
                for (trig, debounce) in triggers.iter().zip(debouncers.iter_mut()) {
                    if watch::matches_watch(trig, &rel) {
                        debounce.on_event(now);
                    }
                }
            }
            _ = tokio::time::sleep(wait) => {}
        }

        let now = elapsed_ms(started);
        for (trig, debounce) in triggers.iter().zip(debouncers.iter_mut()) {
            if debounce.poll(now) != DebounceDecision::Fire {
                continue;
            }
            if let Some(reason) = busy_reason(&repo_path, pipeline_state.as_ref()).await {
                log::info!("[triggers] watch '{}' skipped: {reason}", trig.id);
                let _ = trigger_state::record_skip(&repo_path, &trig.id, Utc::now(), &reason);
                continue;
            }
            let _ = trigger_state::record_fired(&repo_path, &trig.id, Utc::now());

            let request = ExecuteRunRequest {
                repo_path: repo.clone(),
                pipeline_file: trig.pipeline_file.clone(),
                environment: trig.environment.clone(),
                stages: stage_filter(&trig.stages),
                run_kind: RunKind::Watch,
                trigger_id: Some(format!("watch:{}", trig.id)),
                ..Default::default()
            };
            let action = fire(&repo_path, &trig.id, request).await;
            log::info!("[triggers] watch '{}': {action:?}", trig.id);
            if let Some(ref report) = on_run {
                report(&trig.id, &action);
            }
        }
    }
}

/// Debounce timestamps are monotonic milliseconds since the loop started.
fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::persistence::scoped_test_data_dir;

    use tempfile::TempDir;

    fn write_triggers(repo: &Path, body: &str) {
        std::fs::create_dir_all(repo.join(".chibby")).unwrap();
        std::fs::write(repo.join(".chibby").join("triggers.toml"), body).unwrap();
    }

    #[test]
    fn test_disabled_config_yields_no_triggers() {
        let temp = TempDir::new().unwrap();
        write_triggers(
            temp.path(),
            "enabled = false\n[[schedules]]\nid = \"nightly\"\ncron = \"0 3 * * *\"\n",
        );

        let config = load_active_config(temp.path()).unwrap();

        assert!(config.schedules.is_empty(), "master switch ignored");
    }

    #[test]
    fn test_stage_filter_treats_empty_as_all_stages() {
        assert_eq!(stage_filter(&[]), None);
        assert_eq!(
            stage_filter(&["build".to_string()]),
            Some(vec!["build".to_string()])
        );
    }

    /// A trigger that has never fired is armed, not fired, on the first tick.
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_first_tick_arms_a_schedule_without_running_it() {
        let (_dir, _guard) = scoped_test_data_dir();
        let temp = TempDir::new().unwrap();
        write_triggers(
            temp.path(),
            "enabled = true\n[[schedules]]\nid = \"nightly\"\ncron = \"0 3 * * *\"\n",
        );

        let runner = TriggerRunner::new().with_repos(vec![temp.path().to_path_buf()]);
        let outcomes = runner.tick_once().await;

        assert_eq!(outcomes.len(), 1);
        assert!(matches!(outcomes[0].action, TriggerAction::Skipped { .. }));
        let state = trigger_state::get_trigger_state(&temp.path().to_string_lossy(), "nightly")
            .unwrap()
            .unwrap();
        assert!(state.last_fired_at.is_some(), "baseline not persisted");
    }

    /// Cron semantics: a busy repo skips the occurrence rather than queueing it.
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_a_held_run_lock_skips_instead_of_queueing() {
        let (_dir, _guard) = scoped_test_data_dir();
        let temp = TempDir::new().unwrap();
        let repo_path = temp.path().to_string_lossy().to_string();
        write_triggers(
            temp.path(),
            "enabled = true\n[[schedules]]\nid = \"minutely\"\ncron = \"* * * * *\"\nmissed = \"run_once\"\n",
        );
        // Pretend the trigger fired an hour ago, so it is now overdue.
        trigger_state::record_fired(
            &repo_path,
            "minutely",
            Utc::now() - chrono::Duration::hours(1),
        )
        .unwrap();
        let _held = locks::acquire_run_lock(&repo_path).unwrap().unwrap();

        let runner = TriggerRunner::new().with_repos(vec![temp.path().to_path_buf()]);
        let outcomes = runner.tick_once().await;

        match &outcomes[0].action {
            TriggerAction::Skipped { reason } => {
                assert!(reason.contains("already in progress"), "{reason}")
            }
            other => panic!("expected Skipped, got {other:?}"),
        }
    }
}
