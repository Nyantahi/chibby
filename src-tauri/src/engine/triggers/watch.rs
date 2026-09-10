//! Filesystem watch triggers: path filtering plus a hand-rolled debounce.
//!
//! `fsnotify` is the external crate; `crate::engine::notify` is Chibby's own
//! outbound notification module. The alias keeps the two apart.

use super::WatchTrigger;
use anyhow::{Context, Result};
use fsnotify::{RecommendedWatcher, RecursiveMode, Watcher};
use glob::Pattern;
use notify as fsnotify;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

/// Paths Chibby never reacts to, applied before the user's own excludes.
///
/// `.chibby/**` is mandatory, not cosmetic: a run writes into `.chibby/`, so
/// watching it is an infinite loop.
pub const BUILTIN_EXCLUDES: &[&str] = &[
    ".git/**",
    ".chibby/**",
    "node_modules/**",
    "target/**",
    "dist/**",
    "build/**",
    "*.log",
    ".DS_Store",
];

/// Whether a repo-relative path should trigger a run for `trig`.
pub fn matches_watch(trig: &WatchTrigger, rel_path: &str) -> bool {
    let path = normalise(rel_path);

    if any_pattern_matches(BUILTIN_EXCLUDES.iter().copied(), &path) {
        return false;
    }
    if any_pattern_matches(trig.exclude.iter().map(String::as_str), &path) {
        return false;
    }
    // No includes means "everything that survived the excludes".
    if trig.include.is_empty() {
        return true;
    }
    any_pattern_matches(trig.include.iter().map(String::as_str), &path)
}

/// Repo-relative, forward-slashed form of a path, with any `./` prefix removed.
fn normalise(rel_path: &str) -> String {
    rel_path
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

fn any_pattern_matches<'a>(patterns: impl Iterator<Item = &'a str>, path: &str) -> bool {
    patterns
        .filter_map(|p| Pattern::new(p).ok())
        .any(|pattern| {
            if pattern.matches(path) {
                return true;
            }
            // A bare pattern like `*.log` or `.DS_Store` is meant to apply at any
            // depth, so also test it against the file name alone.
            !pattern.as_str().contains('/')
                && path
                    .rsplit('/')
                    .next()
                    .is_some_and(|name| pattern.matches(name))
        })
}

/// Make a watched path repo-relative, or `None` when it escapes the repo.
pub fn relative_to_repo(repo: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(repo).ok()?;
    Some(normalise(&rel.to_string_lossy()))
}

// ---------------------------------------------------------------------------
// Debounce
// ---------------------------------------------------------------------------

/// What the watch loop should do at a given moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebounceDecision {
    /// Fire a run now.
    Fire,
    /// Nothing is pending.
    Idle,
    /// Something is pending; ask again in this many milliseconds.
    Wait(u64),
}

/// Coalesces a burst of filesystem events into a single run.
///
/// Pure and clock-injected — every method takes a monotonic millisecond
/// timestamp — so coalescing and the minimum-interval floor are testable
/// without any real waiting.
#[derive(Debug)]
pub struct Debounce {
    debounce_ms: u64,
    min_interval_ms: u64,
    /// Most recent qualifying event, while a burst is in flight.
    last_event_ms: Option<u64>,
    last_fired_ms: Option<u64>,
}

impl Debounce {
    pub fn new(debounce_ms: u64, min_interval_secs: u64) -> Self {
        Self {
            debounce_ms,
            min_interval_ms: min_interval_secs.saturating_mul(1000),
            last_event_ms: None,
            last_fired_ms: None,
        }
    }

    pub fn from_trigger(trig: &WatchTrigger) -> Self {
        Self::new(trig.debounce_ms, trig.min_interval_secs)
    }

    /// Record a qualifying event, resetting the quiet period.
    pub fn on_event(&mut self, now_ms: u64) {
        self.last_event_ms = Some(now_ms);
    }

    /// Decide what to do at `now_ms` without changing anything — used to work
    /// out how long the watch loop may sleep.
    pub fn peek(&self, now_ms: u64) -> DebounceDecision {
        let Some(last_event) = self.last_event_ms else {
            return DebounceDecision::Idle;
        };

        let quiet = now_ms.saturating_sub(last_event);
        if quiet < self.debounce_ms {
            return DebounceDecision::Wait(self.debounce_ms - quiet);
        }

        // Floor between runs, so a build writing into the repo cannot hot-loop.
        if let Some(fired) = self.last_fired_ms {
            let since = now_ms.saturating_sub(fired);
            if since < self.min_interval_ms {
                return DebounceDecision::Wait(self.min_interval_ms - since);
            }
        }

        DebounceDecision::Fire
    }

    /// Decide what to do at `now_ms`, consuming the pending burst on `Fire`.
    pub fn poll(&mut self, now_ms: u64) -> DebounceDecision {
        let decision = self.peek(now_ms);
        if decision == DebounceDecision::Fire {
            self.last_event_ms = None;
            self.last_fired_ms = Some(now_ms);
        }
        decision
    }
}

// ---------------------------------------------------------------------------
// Watcher
// ---------------------------------------------------------------------------

/// Start one recursive watcher for `repo`, streaming changed paths.
///
/// The returned watcher must be kept alive: dropping it stops the watch.
pub fn watch_repo(repo: &Path) -> Result<(RecommendedWatcher, UnboundedReceiver<PathBuf>)> {
    let (tx, rx) = unbounded_channel();

    let mut watcher =
        fsnotify::recommended_watcher(move |res: fsnotify::Result<fsnotify::Event>| {
            let Ok(event) = res else { return };
            if !matches!(
                event.kind,
                fsnotify::EventKind::Create(_)
                    | fsnotify::EventKind::Modify(_)
                    | fsnotify::EventKind::Remove(_)
            ) {
                return;
            }
            for path in event.paths {
                // Send failures mean the receiver is gone — the watch is over.
                let _ = tx.send(path);
            }
        })
        .context("Failed to create filesystem watcher")?;

    watcher
        .watch(repo, RecursiveMode::Recursive)
        .with_context(|| format!("Failed to watch {}", repo.display()))?;

    Ok((watcher, rx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc::unbounded_channel;

    fn trigger(include: &[&str], exclude: &[&str]) -> WatchTrigger {
        WatchTrigger {
            id: "tests".to_string(),
            enabled: true,
            include: include.iter().map(|s| s.to_string()).collect(),
            exclude: exclude.iter().map(|s| s.to_string()).collect(),
            debounce_ms: 750,
            min_interval_secs: 10,
            pipeline_file: None,
            environment: None,
            stages: Vec::new(),
        }
    }

    #[test]
    fn test_empty_include_matches_everything_not_excluded() {
        let trig = trigger(&[], &[]);

        assert!(matches_watch(&trig, "src/main.rs"));
        assert!(matches_watch(&trig, "README.md"));
    }

    #[test]
    fn test_include_globs_restrict_matches() {
        let trig = trigger(&["src/**/*.rs"], &[]);

        assert!(matches_watch(&trig, "src/engine/mod.rs"));
        assert!(!matches_watch(&trig, "docs/readme.md"));
    }

    #[test]
    fn test_user_excludes_win_over_includes() {
        let trig = trigger(&["src/**"], &["src/generated/**"]);

        assert!(matches_watch(&trig, "src/main.rs"));
        assert!(!matches_watch(&trig, "src/generated/api.rs"));
    }

    /// The run itself writes into `.chibby/`, so matching it would loop forever.
    #[test]
    fn test_chibby_directory_is_never_watchable() {
        let trig = trigger(&["**/*.toml", ".chibby/**"], &[]);

        assert!(!matches_watch(&trig, ".chibby/pipeline.toml"));
        assert!(!matches_watch(&trig, ".chibby/triggers.local.toml"));
    }

    #[test]
    fn test_builtin_denylist_covers_git_and_build_output() {
        let trig = trigger(&[], &[]);

        for path in [
            ".git/HEAD",
            "node_modules/left-pad/index.js",
            "target/debug/chibby",
            "dist/app.js",
            "build/out.o",
            "logs/build.log",
            ".DS_Store",
            "src/.DS_Store",
        ] {
            assert!(!matches_watch(&trig, path), "should be excluded: {path}");
        }
    }

    #[test]
    fn test_relative_to_repo_rejects_paths_outside_the_repo() {
        let repo = Path::new("/repo");

        assert_eq!(
            relative_to_repo(repo, Path::new("/repo/src/main.rs")).as_deref(),
            Some("src/main.rs")
        );
        assert!(relative_to_repo(repo, Path::new("/elsewhere/main.rs")).is_none());
    }

    #[test]
    fn test_debounce_coalesces_a_burst_into_one_fire() {
        let mut debounce = Debounce::new(750, 0);

        // A burst arriving over 300ms, each event resetting the quiet period.
        for now in [0, 100, 200, 300] {
            debounce.on_event(now);
            assert!(matches!(debounce.poll(now), DebounceDecision::Wait(_)));
        }

        assert_eq!(debounce.poll(1_050), DebounceDecision::Fire);
        assert_eq!(debounce.poll(1_051), DebounceDecision::Idle);
    }

    #[test]
    fn test_debounce_waits_out_the_remaining_quiet_period() {
        let mut debounce = Debounce::new(750, 0);
        debounce.on_event(0);

        assert_eq!(debounce.poll(500), DebounceDecision::Wait(250));
        // Peeking must never consume the pending burst.
        assert_eq!(debounce.peek(1_000), DebounceDecision::Fire);
        assert_eq!(debounce.poll(1_000), DebounceDecision::Fire);
    }

    #[test]
    fn test_min_interval_floors_a_second_fire() {
        let mut debounce = Debounce::new(0, 10);

        debounce.on_event(0);
        assert_eq!(debounce.poll(0), DebounceDecision::Fire);

        // A build writing back into the repo immediately: held off, not fired.
        debounce.on_event(1_000);
        assert_eq!(debounce.poll(1_000), DebounceDecision::Wait(9_000));
        assert_eq!(debounce.poll(10_000), DebounceDecision::Fire);
    }

    /// Drives the filter + debounce from a hand-fed channel — the same shape
    /// the real watcher uses, with no filesystem and no waiting.
    #[tokio::test]
    async fn test_channel_burst_produces_a_single_run() {
        let trig = trigger(&[], &[]);
        let (tx, mut rx) = unbounded_channel::<(String, u64)>();

        for (i, path) in [
            "src/a.rs",
            ".chibby/pipeline.toml",
            "src/b.rs",
            ".git/index",
        ]
        .iter()
        .enumerate()
        {
            tx.send((path.to_string(), i as u64 * 50)).unwrap();
        }
        drop(tx);

        let mut debounce = Debounce::from_trigger(&trig);
        let mut fires = 0;
        while let Some((path, at)) = rx.recv().await {
            if matches_watch(&trig, &path) {
                debounce.on_event(at);
            }
            if debounce.poll(at) == DebounceDecision::Fire {
                fires += 1;
            }
        }
        // Everything is still inside the quiet window.
        assert_eq!(fires, 0);

        assert_eq!(debounce.poll(2_000), DebounceDecision::Fire);
    }
}
