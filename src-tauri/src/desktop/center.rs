//! Watches the inbox and tells the window what changed. Entries come from this
//! process and from the CLI alike, so the directory is the one source for both.

use crate::engine::models::{AlertPolicy, NotificationEntry, NotificationSeverity};
use crate::engine::triggers::watch;
use crate::engine::{app_settings, inbox, notify, redact};
use serde::Serialize;
use std::collections::HashSet;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

/// Fallback for a watcher that died or missed an event.
const RESCAN_INTERVAL: Duration = Duration::from_secs(30);
/// Lets a burst of file events settle into one scan.
const SETTLE: Duration = Duration::from_millis(100);
/// More new entries than this in one scan become a single summary alert.
const MAX_SEPARATE_ALERTS: usize = 3;

#[derive(Debug, Clone, Serialize)]
struct NewNotification {
    entry: NotificationEntry,
    /// Whether the window was in front when this arrived.
    foreground: bool,
}

/// What one scan found compared with the previous one.
#[derive(Debug, Default, PartialEq)]
struct ScanResult {
    new_ids: Vec<String>,
    changed: bool,
}

/// Remembers the inbox between scans. Scanning is idempotent, so a watcher
/// event and the periodic rescan can both fire safely.
#[derive(Default)]
struct Center {
    seen: HashSet<String>,
    read: HashSet<String>,
}

impl Center {
    fn seeded(entries: &[NotificationEntry]) -> Self {
        let mut center = Self::default();
        center.scan(entries);
        center
    }

    fn scan(&mut self, entries: &[NotificationEntry]) -> ScanResult {
        let seen: HashSet<String> = entries.iter().map(|e| e.id.clone()).collect();
        let read: HashSet<String> = entries
            .iter()
            .filter(|e| e.read_at.is_some())
            .map(|e| e.id.clone())
            .collect();

        // Oldest first, so toasts appear in the order things happened.
        let new_ids = entries
            .iter()
            .rev()
            .filter(|e| !self.seen.contains(&e.id))
            .map(|e| e.id.clone())
            .collect();
        let changed = seen != self.seen || read != self.read;

        self.seen = seen;
        self.read = read;
        ScanResult { new_ids, changed }
    }
}

/// Whether the user is somewhere other than the Chibby window, read live.
fn is_away(app: &AppHandle) -> bool {
    app.get_webview_window("main").map_or(true, |window| {
        !window.is_focused().unwrap_or(false)
            || !window.is_visible().unwrap_or(false)
            || window.is_minimized().unwrap_or(false)
    })
}

fn should_alert(entry: &NotificationEntry, away: bool, when_focused: bool) -> bool {
    if entry.alert_delivered {
        return false;
    }
    match entry.alert {
        AlertPolicy::None => false,
        AlertPolicy::WhenAway => away || when_focused,
        AlertPolicy::Always => true,
    }
}

/// The (title, body) pairs to show for one scan's new entries.
fn plan_alerts(
    new: &[&NotificationEntry],
    away: bool,
    when_focused: bool,
) -> Vec<(String, String)> {
    let due: Vec<&&NotificationEntry> = new
        .iter()
        .filter(|entry| should_alert(entry, away, when_focused))
        .collect();

    if due.len() > MAX_SEPARATE_ALERTS {
        let failed = due
            .iter()
            .filter(|entry| entry.severity == NotificationSeverity::Failure)
            .count();
        return vec![(
            format!("Chibby: {} notifications", due.len()),
            format!("{failed} failed. Open Chibby to see them."),
        )];
    }

    due.iter()
        .map(|entry| (entry.title.clone(), redact::redact(&entry.body)))
        .collect()
}

/// Show an OS alert. The plugin reports no delivery failure on desktop, which
/// is why the inbox, not this, is the reliable record.
pub fn show_alert(app: &AppHandle, title: &str, body: &str) {
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        log::warn!("Desktop notification failed: {e}");
    }
}

fn scan_and_emit(app: &AppHandle, center: &mut Center) {
    let entries = match inbox::list() {
        Ok(entries) => entries,
        Err(e) => {
            log::warn!("Failed to read notifications: {e}");
            return;
        }
    };

    let result = center.scan(&entries);
    let away = is_away(app);
    let new: Vec<&NotificationEntry> = result
        .new_ids
        .iter()
        .filter_map(|id| entries.iter().find(|e| &e.id == id))
        .collect();

    for entry in &new {
        let _ = app.emit(
            "notification:new",
            NewNotification {
                entry: (*entry).clone(),
                foreground: !away,
            },
        );
    }
    if result.changed {
        let _ = app.emit("notification:changed", ());
    }

    if new.is_empty() {
        return;
    }
    let when_focused = app_settings::load_app_settings()
        .unwrap_or_default()
        .alert_when_focused;
    for (title, body) in plan_alerts(&new, away, when_focused) {
        show_alert(app, &title, &body);
    }
}

/// Start watching the inbox for the life of the app.
pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut center = Center::seeded(&inbox::list().unwrap_or_default());

        // Without a watcher the rescan still delivers, just later.
        let watcher = inbox::inbox_dir().and_then(|dir| watch::watch_repo(&dir));
        let (_watcher, mut events) = match watcher {
            Ok((watcher, events)) => (Some(watcher), Some(events)),
            Err(e) => {
                log::warn!("Notification watcher unavailable, using rescans only: {e}");
                (None, None)
            }
        };

        let mut rescan = tokio::time::interval(RESCAN_INTERVAL);
        loop {
            tokio::select! {
                _ = rescan.tick() => {
                    if let Err(e) = notify::touch_gui_heartbeat() {
                        log::warn!("{e}");
                    }
                }
                Some(_) = async {
                    match events.as_mut() {
                        Some(events) => events.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    tokio::time::sleep(SETTLE).await;
                    if let Some(events) = events.as_mut() {
                        while events.try_recv().is_ok() {}
                    }
                }
            }
            scan_and_emit(&app, &mut center);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::models::NotificationKind;
    use chrono::Utc;

    fn entry(id: &str) -> NotificationEntry {
        NotificationEntry {
            id: id.to_string(),
            ..NotificationEntry::new(
                NotificationKind::RunFinished,
                NotificationSeverity::Success,
                "title".to_string(),
                "body".to_string(),
            )
        }
    }

    fn alerting(alert: AlertPolicy, severity: NotificationSeverity) -> NotificationEntry {
        NotificationEntry {
            alert,
            severity,
            ..entry("a")
        }
    }

    /// (policy, away, alert_when_focused, alerts)
    #[test]
    fn test_should_alert_truth_table() {
        let cases = [
            (AlertPolicy::None, true, true, false),
            (AlertPolicy::WhenAway, true, false, true),
            (AlertPolicy::WhenAway, false, false, false),
            (AlertPolicy::WhenAway, false, true, true),
            (AlertPolicy::Always, false, false, true),
            (AlertPolicy::Always, true, false, true),
        ];

        for (policy, away, when_focused, expected) in cases {
            let due = alerting(policy, NotificationSeverity::Failure);
            assert_eq!(
                should_alert(&due, away, when_focused),
                expected,
                "{policy:?} away={away} when_focused={when_focused}"
            );
        }
    }

    #[test]
    fn test_entry_another_process_alerted_for_stays_quiet() {
        let mut delivered = alerting(AlertPolicy::Always, NotificationSeverity::Failure);
        delivered.alert_delivered = true;

        assert!(!should_alert(&delivered, true, true));
    }

    #[test]
    fn test_up_to_three_alerts_stay_separate() {
        let due = alerting(AlertPolicy::WhenAway, NotificationSeverity::Success);
        let new = [&due, &due, &due];

        assert_eq!(plan_alerts(&new, true, false).len(), 3);
    }

    #[test]
    fn test_a_burst_collapses_into_one_summary_counting_failures() {
        let success = alerting(AlertPolicy::WhenAway, NotificationSeverity::Success);
        let failure = alerting(AlertPolicy::Always, NotificationSeverity::Failure);
        let quiet = alerting(AlertPolicy::None, NotificationSeverity::Failure);
        let new = [&success, &success, &failure, &failure, &quiet];

        let alerts = plan_alerts(&new, true, false);

        assert_eq!(
            alerts,
            vec![(
                "Chibby: 4 notifications".to_string(),
                "2 failed. Open Chibby to see them.".to_string()
            )]
        );
    }

    #[test]
    fn test_alert_body_is_redacted() {
        let mut leaky = alerting(AlertPolicy::Always, NotificationSeverity::Failure);
        leaky.body = "curl -H 'Authorization: Bearer abcdef1234567890abcdef'".to_string();

        let alerts = plan_alerts(&[&leaky], true, false);

        assert!(
            !alerts[0].1.contains("abcdef1234567890abcdef"),
            "{:?}",
            alerts[0].1
        );
    }

    #[test]
    fn test_seeding_never_reannounces_existing_entries() {
        let entries = [entry("b"), entry("a")];
        let mut center = Center::seeded(&entries);

        assert_eq!(center.scan(&entries), ScanResult::default());
    }

    #[test]
    fn test_scan_reports_new_entries_oldest_first_and_only_once() {
        let mut center = Center::seeded(&[entry("a")]);
        let entries = [entry("c"), entry("b"), entry("a")];

        let first = center.scan(&entries);
        assert_eq!(first.new_ids, vec!["b".to_string(), "c".to_string()]);
        assert!(first.changed);

        assert_eq!(center.scan(&entries), ScanResult::default());
    }

    #[test]
    fn test_read_and_removal_are_changes_without_new_entries() {
        let mut center = Center::seeded(&[entry("b"), entry("a")]);

        let mut read = entry("a");
        read.read_at = Some(Utc::now());
        let result = center.scan(&[entry("b"), read.clone()]);
        assert!(result.changed);
        assert!(result.new_ids.is_empty());

        let result = center.scan(&[read]);
        assert!(result.changed);
        assert!(result.new_ids.is_empty());
    }
}
