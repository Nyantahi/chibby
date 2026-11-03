use crate::engine::models::{
    NotifyConfig, NotifyOn, NotifyPayload, ResolvedNotifyConfig, RunStatus,
};
use crate::engine::notify;
use std::path::Path;

/// Load the notification config a repo's runs actually use.
#[tauri::command]
pub fn resolve_notify_config(repo_path: String) -> Result<ResolvedNotifyConfig, String> {
    let path = Path::new(&repo_path);
    Ok(ResolvedNotifyConfig {
        config: notify::resolve_notify_config(path).map_err(|e| e.to_string())?,
        using_app_defaults: !notify::has_repo_notify_config(path),
    })
}

/// Save notification config to .chibby/notify.toml.
#[tauri::command]
pub fn save_notify_config(repo_path: String, config: NotifyConfig) -> Result<(), String> {
    notify::save_notify_config(Path::new(&repo_path), &config).map_err(|e| e.to_string())
}

/// Send a test notification using the current config.
#[tauri::command]
pub async fn send_test_notification(repo_path: String) -> Result<String, String> {
    let mut config =
        notify::resolve_notify_config(Path::new(&repo_path)).map_err(|e| e.to_string())?;

    if !config.enabled {
        return Err("Notifications are not enabled".to_string());
    }

    // A test exercises every target, including failure-only ones.
    for target in &mut config.targets {
        target.on = NotifyOn::Always;
    }

    let payload = NotifyPayload {
        project: "Chibby Test".to_string(),
        version: Some("0.0.0".to_string()),
        environment: Some("test".to_string()),
        status: RunStatus::Success,
        duration_ms: Some(1234),
        message: "This is a test notification from Chibby".to_string(),
        rollback: None,
        run_kind: None,
        trigger_id: None,
    };

    notify::send_notifications(&config, &payload).await;

    Ok("Test notification sent".to_string())
}
