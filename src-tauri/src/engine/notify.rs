use crate::engine::app_settings;
use crate::engine::models::{
    AlertPolicy, NotifyChannel, NotifyConfig, NotifyOn, NotifyPayload, NotifyTarget,
    RollbackOutcome, RunStatus,
};
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

// ---------------------------------------------------------------------------
// Notification config persistence (.chibby/notify.toml)
// ---------------------------------------------------------------------------

/// Save notification config to .chibby/notify.toml.
pub fn save_notify_config(repo_path: &Path, config: &NotifyConfig) -> Result<()> {
    let chibby_dir = repo_path.join(".chibby");
    std::fs::create_dir_all(&chibby_dir)?;

    let toml_str =
        toml::to_string_pretty(config).context("Failed to serialize notification config")?;

    let file_path = chibby_dir.join("notify.toml");
    std::fs::write(&file_path, &toml_str)?;

    log::info!("Saved notification config to {}", file_path.display());
    Ok(())
}

/// Load notification config from .chibby/notify.toml.
pub fn load_notify_config(repo_path: &Path) -> Result<NotifyConfig> {
    let file_path = repo_path.join(".chibby").join("notify.toml");
    if !file_path.exists() {
        return Ok(NotifyConfig::default());
    }
    let content = std::fs::read_to_string(&file_path)
        .with_context(|| format!("Failed to read {}", file_path.display()))?;

    let config: NotifyConfig = toml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", file_path.display()))?;

    Ok(config)
}

/// Whether the repo has its own .chibby/notify.toml, rather than app defaults.
pub fn has_repo_notify_config(repo_path: &Path) -> bool {
    repo_path.join(".chibby").join("notify.toml").exists()
}

/// Resolve notification config for a repo, falling back to app-level defaults.
///
/// If a per-repo `.chibby/notify.toml` exists, it is used as-is.
/// Otherwise, the app-level settings (`default_notify_on_success` /
/// `default_notify_on_failure`) are used to build a desktop notification
/// config automatically.
pub fn resolve_notify_config(repo_path: &Path) -> Result<NotifyConfig> {
    if has_repo_notify_config(repo_path) {
        return load_notify_config(repo_path);
    }

    // No per-repo config — build from app-level defaults.
    let app = app_settings::load_app_settings().unwrap_or_default();

    let notify_success = app.default_notify_on_success;
    let notify_failure = app.default_notify_on_failure;

    if !notify_success && !notify_failure {
        return Ok(NotifyConfig::default()); // both off → disabled
    }

    let mut targets = Vec::new();

    if notify_success && notify_failure {
        targets.push(NotifyTarget {
            channel: NotifyChannel::Desktop,
            url: None,
            on: NotifyOn::Always,
        });
    } else if notify_success {
        targets.push(NotifyTarget {
            channel: NotifyChannel::Desktop,
            url: None,
            on: NotifyOn::Success,
        });
    } else {
        targets.push(NotifyTarget {
            channel: NotifyChannel::Desktop,
            url: None,
            on: NotifyOn::Failure,
        });
    }

    Ok(NotifyConfig {
        enabled: true,
        targets,
        ..Default::default()
    })
}

// ---------------------------------------------------------------------------
// Notification dispatch
// ---------------------------------------------------------------------------

/// Whether this payload must be announced regardless of the normal config.
///
/// Scheduled and file-watch runs have no human watching the screen, so a
/// failure that nobody is told about is a failure that never happened.
fn escalates_unattended(config: &NotifyConfig, payload: &NotifyPayload) -> bool {
    if !payload.run_kind.is_some_and(|k| k.is_unattended()) {
        return false;
    }
    match payload.status {
        RunStatus::Failed => config.unattended.always_on_failure,
        RunStatus::Success => config.unattended.on_success,
        _ => false,
    }
}

/// Distinct title for an escalated unattended failure, when that is what this is.
fn unattended_label(payload: &NotifyPayload) -> Option<String> {
    if !payload.run_kind.is_some_and(|k| k.is_unattended()) || payload.status != RunStatus::Failed {
        return None;
    }
    let trigger = payload.trigger_id.as_deref().unwrap_or("trigger");
    Some(format!(
        "Unattended failure: {} / {trigger}",
        payload.project
    ))
}

/// What one payload should do, given the config.
#[derive(Debug, PartialEq)]
pub struct DeliveryPlan {
    pub alert: AlertPolicy,
    pub webhooks: Vec<String>,
}

/// Decide delivery for a payload. Pure, so the policy is testable without
/// sending anything.
pub fn plan_delivery(config: &NotifyConfig, payload: &NotifyPayload) -> DeliveryPlan {
    let escalate = escalates_unattended(config, payload);

    if !config.enabled && !escalate {
        return DeliveryPlan {
            alert: AlertPolicy::None,
            webhooks: Vec::new(),
        };
    }

    let fires = |target: &&NotifyTarget| {
        escalate
            || match target.on {
                NotifyOn::Always => true,
                NotifyOn::Success => payload.status == RunStatus::Success,
                NotifyOn::Failure => payload.status == RunStatus::Failed,
            }
    };

    // An escalation has to reach the user even with only webhook targets.
    let alert = if escalate {
        AlertPolicy::Always
    } else if config
        .targets
        .iter()
        .filter(fires)
        .any(|t| t.channel == NotifyChannel::Desktop)
    {
        AlertPolicy::WhenAway
    } else {
        AlertPolicy::None
    };

    let webhooks = config
        .targets
        .iter()
        .filter(fires)
        .filter(|t| t.channel == NotifyChannel::Webhook)
        .filter_map(|t| {
            if t.url.is_none() {
                log::warn!("Webhook target has no URL configured");
            }
            t.url.clone()
        })
        .collect();

    DeliveryPlan { alert, webhooks }
}

/// Send notifications based on config and run status.
/// Failures are logged but never returned as errors — notifications must not block pipelines.
pub async fn send_notifications(config: &NotifyConfig, payload: &NotifyPayload) {
    let plan = plan_delivery(config, payload);

    if plan.alert != AlertPolicy::None {
        if let Err(e) = send_desktop_notification(payload).await {
            log::warn!("Desktop notification failed: {e}");
        }
    }
    send_webhooks(&plan.webhooks, payload).await;
}

/// As [`send_notifications`], webhooks only: for a payload whose desktop alert
/// is carried by another one, such as a rollback reported by its parent run.
pub async fn send_webhook_notifications(config: &NotifyConfig, payload: &NotifyPayload) {
    send_webhooks(&plan_delivery(config, payload).webhooks, payload).await;
}

/// Resolve the repo's config and send. A config that will not parse still
/// alerts: a typo in notify.toml must not silence a failure.
pub async fn announce(repo_path: &Path, payload: &NotifyPayload) {
    match resolve_notify_config(repo_path) {
        Ok(config) => send_notifications(&config, payload).await,
        Err(e) => {
            log::warn!(
                "Failed to load notify config for {}: {e}",
                repo_path.display()
            );
            if let Err(e) = send_desktop_notification(payload).await {
                log::warn!("Desktop notification failed: {e}");
            }
        }
    }
}

async fn send_webhooks(urls: &[String], payload: &NotifyPayload) {
    for url in urls {
        if let Err(e) = send_webhook(url, payload).await {
            log::warn!("Webhook notification to {url} failed: {e}");
        }
    }
}

/// Human-readable suffix describing an automatic rollback, if one ran.
fn rollback_suffix(payload: &NotifyPayload) -> String {
    match payload.rollback {
        Some(RollbackOutcome::Succeeded) => " — auto-rolled back, healthy again".to_string(),
        Some(RollbackOutcome::Failed) => {
            " — AUTO-ROLLBACK FAILED, manual intervention required".to_string()
        }
        Some(RollbackOutcome::Skipped) => " — auto-rollback skipped".to_string(),
        None => String::new(),
    }
}

/// Title and body of the desktop notification for a payload.
fn desktop_text(payload: &NotifyPayload) -> (String, String) {
    let status_text = match payload.status {
        RunStatus::Success => "succeeded",
        RunStatus::Failed => "failed",
        RunStatus::Cancelled => "cancelled",
        _ => "completed",
    };

    let title = match unattended_label(payload) {
        Some(label) => format!("Chibby: {label}"),
        None => format!("Chibby: {} {}", payload.project, status_text),
    };

    let mut body = format!("{}{}", payload.message, rollback_suffix(payload));
    if let Some(ref version) = payload.version {
        body = format!("v{version} — {body}");
    }
    if let Some(ms) = payload.duration_ms {
        let secs = ms / 1000;
        body.push_str(&format!(" ({secs}s)"));
    }

    (title, body)
}

/// The OS command that shows a notification. Title and body come from
/// repo-controlled names, so they travel as data and never as script text.
#[cfg(target_os = "macos")]
fn desktop_command(title: &str, body: &str) -> Command {
    let mut command = Command::new("osascript");
    command
        .args(["-e", "on run argv"])
        .args([
            "-e",
            "display notification (item 2 of argv) with title (item 1 of argv)",
        ])
        .args(["-e", "end run", "--", title, body]);
    command
}

#[cfg(target_os = "linux")]
fn desktop_command(title: &str, body: &str) -> Command {
    let mut command = Command::new("notify-send");
    command.args(["--", title, body]);
    command
}

#[cfg(target_os = "windows")]
fn desktop_command(title: &str, body: &str) -> Command {
    let script = "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null; \
         $template = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent(0); \
         $text = $template.GetElementsByTagName('text'); \
         $text[0].AppendChild($template.CreateTextNode($env:CHIBBY_NOTIFY_TITLE)) | Out-Null; \
         $text[1].AppendChild($template.CreateTextNode($env:CHIBBY_NOTIFY_BODY)) | Out-Null; \
         $notifier = [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('Chibby'); \
         $notifier.Show([Windows.UI.Notifications.ToastNotification]::new($template))";
    let mut command = Command::new("powershell");
    command
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("CHIBBY_NOTIFY_TITLE", title)
        .env("CHIBBY_NOTIFY_BODY", body);
    command
}

// Desktop alerts this test thread would have shown.
#[cfg(test)]
thread_local! {
    pub(crate) static TEST_ALERTS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Send a desktop notification using the OS notification system.
async fn send_desktop_notification(payload: &NotifyPayload) -> Result<()> {
    let (title, body) = desktop_text(payload);

    // Tests drive real runs through this path; they must not pop up alerts.
    if cfg!(test) {
        #[cfg(test)]
        TEST_ALERTS.with(|count| count.set(count.get() + 1));
        return Ok(());
    }

    let output = tokio::process::Command::from(desktop_command(&title, &body))
        .output()
        .await
        .context("Failed to start the OS notification command")?;

    if !output.status.success() {
        bail!(
            "OS notification command exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    log::info!("Sent desktop notification: {title}");
    Ok(())
}

/// Send a webhook notification (Slack/Discord compatible JSON payload).
async fn send_webhook(url: &str, payload: &NotifyPayload) -> Result<()> {
    let status_emoji = match payload.status {
        RunStatus::Success => "✅",
        RunStatus::Failed => "❌",
        RunStatus::Cancelled => "⚠️",
        _ => "🔄",
    };

    let mut text = match unattended_label(payload) {
        Some(label) => format!("{status_emoji} *{label}* — {}", payload.message),
        None => format!("{status_emoji} *{}* {}", payload.project, payload.message),
    };
    if let (None, Some(version)) = (unattended_label(payload), payload.version.as_ref()) {
        text = format!(
            "{status_emoji} *{}* v{version} — {}",
            payload.project, payload.message
        );
    }
    if let Some(ref env) = payload.environment {
        text.push_str(&format!(" (env: {env})"));
    }
    text.push_str(&rollback_suffix(payload));
    if let Some(ms) = payload.duration_ms {
        let secs = ms / 1000;
        text.push_str(&format!(" [{secs}s]"));
    }

    // Slack/Discord compatible payload
    let body = serde_json::json!({
        "text": text,
        "username": "Chibby",
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(url)
        .json(&body)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .with_context(|| format!("Failed to POST to {url}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();
        log::warn!("Webhook returned {status}: {body_text}");
    } else {
        log::info!("Sent webhook notification to {url}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::models::{RunKind, UnattendedNotify};
    use std::ffi::OsStr;

    fn payload(status: RunStatus, run_kind: Option<RunKind>) -> NotifyPayload {
        NotifyPayload {
            project: "shop".to_string(),
            version: None,
            environment: Some("prod".to_string()),
            status,
            duration_ms: Some(1000),
            message: "Pipeline 'shop' failed".to_string(),
            rollback: None,
            run_kind,
            trigger_id: Some("scheduled:nightly".to_string()),
        }
    }

    /// The whole point: a nightly failure is announced even with notifications off.
    #[test]
    fn test_unattended_failure_escalates_past_disabled_config() {
        let config = NotifyConfig::default();
        assert!(!config.enabled);

        assert!(escalates_unattended(
            &config,
            &payload(RunStatus::Failed, Some(RunKind::Scheduled))
        ));
    }

    #[test]
    fn test_attended_runs_never_escalate() {
        let config = NotifyConfig::default();

        for kind in [
            RunKind::Normal,
            RunKind::Retry,
            RunKind::Rollback,
            RunKind::Hook,
        ] {
            assert!(
                !escalates_unattended(&config, &payload(RunStatus::Failed, Some(kind))),
                "{kind:?} should not escalate"
            );
        }
        assert!(!escalates_unattended(
            &config,
            &payload(RunStatus::Failed, None)
        ));
    }

    #[test]
    fn test_escalation_can_be_switched_off_and_success_switched_on() {
        let config = NotifyConfig {
            unattended: UnattendedNotify {
                always_on_failure: false,
                on_success: true,
            },
            ..Default::default()
        };

        assert!(!escalates_unattended(
            &config,
            &payload(RunStatus::Failed, Some(RunKind::Watch))
        ));
        assert!(escalates_unattended(
            &config,
            &payload(RunStatus::Success, Some(RunKind::Watch))
        ));
    }

    #[test]
    fn test_unattended_failure_gets_a_distinct_title() {
        let label = unattended_label(&payload(RunStatus::Failed, Some(RunKind::Scheduled)));

        assert_eq!(
            label.as_deref(),
            Some("Unattended failure: shop / scheduled:nightly")
        );
        assert!(unattended_label(&payload(RunStatus::Success, Some(RunKind::Scheduled))).is_none());
    }

    const HOOK: &str = "https://hooks.example/a";

    fn target(channel: NotifyChannel, on: NotifyOn) -> NotifyTarget {
        let url = (channel == NotifyChannel::Webhook).then(|| HOOK.to_string());
        NotifyTarget { channel, url, on }
    }

    fn config(enabled: bool, targets: Vec<NotifyTarget>) -> NotifyConfig {
        NotifyConfig {
            enabled,
            targets,
            ..Default::default()
        }
    }

    /// (enabled, target `on`, status, kind, webhook fires), matching the
    /// behaviour of the per-target loop this replaced.
    #[test]
    fn test_webhook_selection_is_unchanged() {
        let scheduled = Some(RunKind::Scheduled);
        let cases = [
            (false, NotifyOn::Always, RunStatus::Success, None, false),
            (false, NotifyOn::Always, RunStatus::Failed, None, false),
            (true, NotifyOn::Always, RunStatus::Success, None, true),
            (true, NotifyOn::Always, RunStatus::Cancelled, None, true),
            (true, NotifyOn::Success, RunStatus::Success, None, true),
            (true, NotifyOn::Success, RunStatus::Failed, None, false),
            (true, NotifyOn::Failure, RunStatus::Failed, None, true),
            (true, NotifyOn::Failure, RunStatus::Success, None, false),
            (true, NotifyOn::Failure, RunStatus::Cancelled, None, false),
            // An escalation fires every webhook, whatever its `on` or `enabled`.
            (false, NotifyOn::Always, RunStatus::Failed, scheduled, true),
            (true, NotifyOn::Success, RunStatus::Failed, scheduled, true),
            (
                false,
                NotifyOn::Always,
                RunStatus::Success,
                scheduled,
                false,
            ),
        ];

        for (enabled, on, status, kind, fires) in cases {
            let case = format!("{enabled} {on:?} {status:?} {kind:?}");
            let plan = plan_delivery(
                &config(enabled, vec![target(NotifyChannel::Webhook, on)]),
                &payload(status, kind),
            );
            assert_eq!(!plan.webhooks.is_empty(), fires, "{case}");
        }
    }

    #[test]
    fn test_webhook_without_url_is_dropped() {
        let mut no_url = target(NotifyChannel::Webhook, NotifyOn::Always);
        no_url.url = None;

        let plan = plan_delivery(
            &config(true, vec![no_url]),
            &payload(RunStatus::Failed, None),
        );

        assert!(plan.webhooks.is_empty());
    }

    #[test]
    fn test_matching_desktop_target_alerts_when_away() {
        let desktop = |on| config(true, vec![target(NotifyChannel::Desktop, on)]);
        let failed = payload(RunStatus::Failed, None);

        assert_eq!(
            plan_delivery(&desktop(NotifyOn::Failure), &failed).alert,
            AlertPolicy::WhenAway
        );
        assert_eq!(
            plan_delivery(&desktop(NotifyOn::Success), &failed).alert,
            AlertPolicy::None
        );
    }

    #[test]
    fn test_several_desktop_targets_alert_once_and_webhook_alone_does_not_alert() {
        let failed = payload(RunStatus::Failed, None);

        let webhook_only = config(true, vec![target(NotifyChannel::Webhook, NotifyOn::Always)]);
        assert_eq!(
            plan_delivery(&webhook_only, &failed).alert,
            AlertPolicy::None
        );

        let both = config(
            true,
            vec![
                target(NotifyChannel::Desktop, NotifyOn::Always),
                target(NotifyChannel::Desktop, NotifyOn::Failure),
                target(NotifyChannel::Webhook, NotifyOn::Always),
            ],
        );
        let plan = plan_delivery(&both, &failed);
        assert_eq!(plan.alert, AlertPolicy::WhenAway);
        assert_eq!(plan.webhooks, vec![HOOK.to_string()]);
    }

    #[test]
    fn test_escalation_alerts_even_with_only_webhook_targets() {
        let plan = plan_delivery(
            &config(true, vec![target(NotifyChannel::Webhook, NotifyOn::Always)]),
            &payload(RunStatus::Failed, Some(RunKind::Scheduled)),
        );

        assert_eq!(plan.alert, AlertPolicy::Always);
    }

    #[test]
    fn test_switching_escalation_off_silences_a_disabled_config() {
        let config = NotifyConfig {
            unattended: UnattendedNotify {
                always_on_failure: false,
                on_success: false,
            },
            ..Default::default()
        };

        let plan = plan_delivery(&config, &payload(RunStatus::Failed, Some(RunKind::Watch)));

        assert_eq!(plan.alert, AlertPolicy::None);
    }

    /// A hostile pipeline name must reach the OS command as inert data.
    #[test]
    fn test_hostile_text_never_enters_script_source() {
        for hostile in [
            "x'; Remove-Item C:\\ -Recurse; '",
            "x\" & do shell script \"id\" & \"",
            "trailing\\",
            "--icon=evil",
        ] {
            let command = desktop_command(hostile, "body");
            let args: Vec<&OsStr> = command.get_args().collect();

            // Everything before `--` is script source; text only ever follows it.
            let separator = args.iter().position(|a| *a == "--").unwrap_or(args.len());
            for arg in &args[..separator] {
                assert!(
                    !arg.to_string_lossy().contains(hostile),
                    "{hostile} leaked into {arg:?}"
                );
            }

            #[cfg(target_os = "windows")]
            assert!(command
                .get_envs()
                .any(|(k, v)| k == "CHIBBY_NOTIFY_TITLE" && v == Some(OsStr::new(hostile))));

            #[cfg(not(target_os = "windows"))]
            assert_eq!(
                args[separator + 1..],
                [OsStr::new(hostile), OsStr::new("body")]
            );
        }
    }

    #[test]
    fn test_unparseable_config_still_alerts() {
        let (_data, _guard) = crate::engine::persistence::scoped_test_data_dir();
        let repo = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(repo.path().join(".chibby")).unwrap();
        std::fs::write(repo.path().join(".chibby/notify.toml"), "not [valid").unwrap();
        TEST_ALERTS.with(|count| count.set(0));

        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(announce(repo.path(), &payload(RunStatus::Failed, None)));

        assert_eq!(TEST_ALERTS.with(|count| count.get()), 1);
    }

    #[test]
    fn test_desktop_text_tells_the_rollback_story() {
        let mut failed = payload(RunStatus::Failed, None);
        failed.rollback = Some(RollbackOutcome::Failed);

        let (title, body) = desktop_text(&failed);

        assert_eq!(title, "Chibby: shop failed");
        assert_eq!(
            body,
            "Pipeline 'shop' failed — AUTO-ROLLBACK FAILED, manual intervention required (1s)"
        );
    }
}
