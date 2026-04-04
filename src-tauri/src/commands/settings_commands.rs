use crate::engine::app_settings;
use crate::engine::audit;

/// Load app-level settings.
#[tauri::command]
pub fn load_app_settings() -> Result<app_settings::AppSettings, String> {
    app_settings::load_app_settings().map_err(|e| e.to_string())
}

/// Save app-level settings.
#[tauri::command]
pub fn save_app_settings(settings: app_settings::AppSettings) -> Result<(), String> {
    audit::log_event("save_app_settings", "settings updated");
    app_settings::save_app_settings(&settings).map_err(|e| e.to_string())
}

/// Store an API key in the OS keychain.
#[tauri::command]
pub fn set_app_api_key(provider: String, value: String) -> Result<(), String> {
    audit::log_event("set_app_api_key", &format!("provider={}", provider));
    app_settings::set_app_secret(&provider, &value).map_err(|e| e.to_string())
}

/// Delete an API key from the OS keychain.
#[tauri::command]
pub fn delete_app_api_key(provider: String) -> Result<(), String> {
    audit::log_event("delete_app_api_key", &format!("provider={}", provider));
    app_settings::delete_app_secret(&provider).map_err(|e| e.to_string())
}

/// Check if an API key exists in the OS keychain.
#[tauri::command]
pub fn has_app_api_key(provider: String) -> bool {
    app_settings::has_app_secret(&provider)
}

/// Get the app data directory path.
#[tauri::command]
pub fn get_app_data_dir() -> Result<String, String> {
    app_settings::get_app_data_dir_string().map_err(|e| e.to_string())
}

/// Get the app version.
#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Read the crash log (if any) for debugging.
#[tauri::command]
pub fn get_crash_log() -> Result<Option<String>, String> {
    let dir = crate::engine::persistence::data_dir().map_err(|e| e.to_string())?;
    let crash_file = dir.join("crash.log");
    if crash_file.exists() {
        let content = std::fs::read_to_string(&crash_file).map_err(|e| e.to_string())?;
        Ok(Some(content))
    } else {
        Ok(None)
    }
}

/// Clear the crash log after it has been reviewed.
#[tauri::command]
pub fn clear_crash_log() -> Result<(), String> {
    let dir = crate::engine::persistence::data_dir().map_err(|e| e.to_string())?;
    let crash_file = dir.join("crash.log");
    if crash_file.exists() {
        std::fs::remove_file(&crash_file).map_err(|e| e.to_string())?;
    }
    Ok(())
}
