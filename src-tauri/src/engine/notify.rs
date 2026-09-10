use crate::engine::app_settings;
use crate::engine::models::{
    NotifyChannel, NotifyConfig, NotifyOn, NotifyPayload, NotifyTarget, RollbackOutcome, RunStatus,
};
use anyhow::{Context, Result};
use std::path::Path;

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

/// Resolve notification config for a repo, falling back to app-level defaults.
///
/// If a per-repo `.chibby/notify.toml` exists, it is used as-is.
/// Otherwise, the app-level settings (`default_notify_on_success` /
/// `default_notify_on_failure`) are used to build a desktop notification
/// config automatically.
pub fn resolve_notify_config(repo_path: &Path) -> Result<NotifyConfig> {
    let file_path = repo_path.join(".chibby").join("notify.toml");
    if file_path.exists() {
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

/// Send notifications based on config and run status.
/// Failures are logged but never returned as errors — notifications must not block pipelines.
pub async fn send_notifications(config: &NotifyConfig, payload: &NotifyPayload) {
    let escalate = escalates_unattended(config, payload);

    if !config.enabled && !escalate {
        return;
    }

    // An escalation with nowhere to go still has to reach the user.
    if escalate && config.targets.is_empty() {
        if let Err(e) = send_desktop_notification(payload) {
            log::warn!("Unattended escalation failed: {e}");
        }
        return;
    }

    for target in &config.targets {
        let should_fire = escalate
            || match target.on {
                NotifyOn::Always => true,
                NotifyOn::Success => payload.status == RunStatus::Success,
                NotifyOn::Failure => payload.status == RunStatus::Failed,
            };

        if !should_fire {
            continue;
        }

        match target.channel {
            NotifyChannel::Desktop => {
                if let Err(e) = send_desktop_notification(payload) {
                    log::warn!("Desktop notification failed: {e}");
                }
            }
            NotifyChannel::Webhook => {
                if let Some(ref url) = target.url {
                    if let Err(e) = send_webhook(url, payload).await {
                        log::warn!("Webhook notification to {url} failed: {e}");
                    }
                } else {
                    log::warn!("Webhook target has no URL configured");
                }
            }
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

/// Send a desktop notification using the OS notification system.
fn send_desktop_notification(payload: &NotifyPayload) -> Result<()> {
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

    // Use osascript on macOS, notify-send on Linux, PowerShell on Windows
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "display notification \"{}\" with title \"{}\"",
            body.replace('\"', "\\\""),
            title.replace('\"', "\\\"")
        );
        std::process::Command::new("osascript")
            .args(["-e", &script])
            .output()
            .context("Failed to send macOS notification")?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("notify-send")
            .args([&title, &body])
            .output()
            .context("Failed to send Linux notification — is notify-send installed?")?;
    }

    #[cfg(target_os = "windows")]
    {
        let script = format!(
            "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null; \
             $template = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent(0); \
             $text = $template.GetElementsByTagName('text'); \
             $text[0].AppendChild($template.CreateTextNode('{title}')) | Out-Null; \
             $text[1].AppendChild($template.CreateTextNode('{body}')) | Out-Null; \
             $notifier = [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('Chibby'); \
             $notifier.Show([Windows.UI.Notifications.ToastNotification]::new($template))",
        );
        std::process::Command::new("powershell")
            .args(["-Command", &script])
            .output()
            .context("Failed to send Windows notification")?;
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
}
