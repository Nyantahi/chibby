use crate::engine::models::{NotifyChannel, NotifyConfig, NotifyOn, NotifyPayload, RunStatus};
use anyhow::{Context, Result};
use std::path::Path;

// ---------------------------------------------------------------------------
// Notification config persistence (.chibby/notify.toml)
// ---------------------------------------------------------------------------

/// Save notification config to .chibby/notify.toml.
pub fn save_notify_config(repo_path: &Path, config: &NotifyConfig) -> Result<()> {
    let chibby_dir = repo_path.join(".chibby");
    std::fs::create_dir_all(&chibby_dir)?;

    let toml_str = toml::to_string_pretty(config)
        .context("Failed to serialize notification config")?;

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

// ---------------------------------------------------------------------------
// Notification dispatch
// ---------------------------------------------------------------------------

/// Send notifications based on config and run status.
/// Failures are logged but never returned as errors — notifications must not block pipelines.
pub async fn send_notifications(config: &NotifyConfig, payload: &NotifyPayload) {
    if !config.enabled {
        return;
    }

    for target in &config.targets {
        let should_fire = match target.on {
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

/// Send a desktop notification using the OS notification system.
fn send_desktop_notification(payload: &NotifyPayload) -> Result<()> {
    let status_text = match payload.status {
        RunStatus::Success => "succeeded",
        RunStatus::Failed => "failed",
        RunStatus::Cancelled => "cancelled",
        _ => "completed",
    };

    let title = format!("Chibby: {} {}", payload.project, status_text);

    let mut body = payload.message.clone();
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

    let mut text = format!(
        "{status_emoji} *{}* {}",
        payload.project, payload.message
    );
    if let Some(ref version) = payload.version {
        text = format!("{status_emoji} *{}* v{version} — {}", payload.project, payload.message);
    }
    if let Some(ref env) = payload.environment {
        text.push_str(&format!(" (env: {env})"));
    }
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
