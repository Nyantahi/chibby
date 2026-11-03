//! Watches the inbox and tells the window what changed. Entries come from this
//! process and from the CLI alike, so the directory is the one source for both.

use crate::engine::inbox;
use crate::engine::models::NotificationEntry;
use crate::engine::triggers::watch;
use serde::Serialize;
use std::collections::HashSet;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// Fallback for a watcher that died or missed an event.
const RESCAN_INTERVAL: Duration = Duration::from_secs(30);
/// Lets a burst of file events settle into one scan.
const SETTLE: Duration = Duration::from_millis(100);

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

fn is_foreground(app: &AppHandle) -> bool {
    app.get_webview_window("main").is_some_and(|window| {
        window.is_focused().unwrap_or(false) && window.is_visible().unwrap_or(false)
    })
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
    let foreground = is_foreground(app);
    for id in &result.new_ids {
        if let Some(entry) = entries.iter().find(|e| &e.id == id) {
            let _ = app.emit(
                "notification:new",
                NewNotification {
                    entry: entry.clone(),
                    foreground,
                },
            );
        }
    }
    if result.changed {
        let _ = app.emit("notification:changed", ());
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
                _ = rescan.tick() => {}
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
    use crate::engine::models::{NotificationKind, NotificationSeverity};
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
