use crate::engine::app_settings;

/// Load app-level settings.
#[tauri::command]
pub fn load_app_settings() -> Result<app_settings::AppSettings, String> {
    app_settings::load_app_settings().map_err(|e| e.to_string())
}

/// Save app-level settings.
#[tauri::command]
pub fn save_app_settings(settings: app_settings::AppSettings) -> Result<(), String> {
    app_settings::save_app_settings(&settings).map_err(|e| e.to_string())
}

/// Store an API key in the OS keychain.
#[tauri::command]
pub fn set_app_api_key(provider: String, value: String) -> Result<(), String> {
    app_settings::set_app_secret(&provider, &value).map_err(|e| e.to_string())
}

/// Delete an API key from the OS keychain.
#[tauri::command]
pub fn delete_app_api_key(provider: String) -> Result<(), String> {
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
