use crate::engine::models::{NotifyConfig, NotifyPayload, RunStatus};
use crate::engine::notify;
use std::path::Path;

/// Load notification config from .chibby/notify.toml.
#[tauri::command]
pub fn load_notify_config(repo_path: String) -> Result<NotifyConfig, String> {
    notify::load_notify_config(Path::new(&repo_path)).map_err(|e| e.to_string())
}

/// Save notification config to .chibby/notify.toml.
#[tauri::command]
pub fn save_notify_config(repo_path: String, config: NotifyConfig) -> Result<(), String> {
    notify::save_notify_config(Path::new(&repo_path), &config).map_err(|e| e.to_string())
}

/// Send a test notification using the current config.
#[tauri::command]
pub async fn send_test_notification(repo_path: String) -> Result<String, String> {
    let config = notify::load_notify_config(Path::new(&repo_path)).map_err(|e| e.to_string())?;

    if !config.enabled {
        return Err("Notifications are not enabled".to_string());
    }

    let payload = NotifyPayload {
        project: "Chibby Test".to_string(),
        version: Some("0.0.0".to_string()),
        environment: Some("test".to_string()),
        status: RunStatus::Success,
        duration_ms: Some(1234),
        message: "This is a test notification from Chibby".to_string(),
    };

    notify::send_notifications(&config, &payload).await;

    Ok("Test notification sent".to_string())
}
