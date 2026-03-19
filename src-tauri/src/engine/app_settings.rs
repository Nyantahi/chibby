use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::engine::persistence;

const SERVICE_NAME: &str = "chibby";

/// App-level settings stored in `<data_dir>/settings.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Notify on successful pipeline runs by default.
    #[serde(default = "default_true")]
    pub default_notify_on_success: bool,
    /// Notify on failed pipeline runs by default.
    #[serde(default = "default_true")]
    pub default_notify_on_failure: bool,
    /// Default artifact retention count for new projects.
    #[serde(default = "default_artifact_retention")]
    pub default_artifact_retention: u32,
    /// Default run history retention count for new projects.
    #[serde(default = "default_run_retention")]
    pub default_run_retention: u32,
}

fn default_true() -> bool {
    true
}

fn default_artifact_retention() -> u32 {
    5
}

fn default_run_retention() -> u32 {
    50
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            default_notify_on_success: true,
            default_notify_on_failure: true,
            default_artifact_retention: 5,
            default_run_retention: 50,
        }
    }
}

/// Load app-level settings from `<data_dir>/settings.toml`.
/// Returns defaults if the file does not exist.
pub fn load_app_settings() -> Result<AppSettings> {
    let path = persistence::data_dir()?.join("settings.toml");
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read settings: {}", path.display()))?;
    let settings: AppSettings = toml::from_str(&content)
        .with_context(|| "Failed to parse settings.toml")?;
    Ok(settings)
}

/// Save app-level settings to `<data_dir>/settings.toml`.
pub fn save_app_settings(settings: &AppSettings) -> Result<()> {
    let path = persistence::data_dir()?.join("settings.toml");
    let content = toml::to_string_pretty(settings)
        .context("Failed to serialize settings")?;
    std::fs::write(&path, content)
        .with_context(|| format!("Failed to write settings: {}", path.display()))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// App-level keychain helpers (for API keys, etc.)
// ---------------------------------------------------------------------------

/// Build a keychain account key for app-level secrets.
fn app_account_key(key_name: &str) -> String {
    format!("app:settings:{}", key_name)
}

/// Store an app-level secret in the OS keychain.
pub fn set_app_secret(key_name: &str, value: &str) -> Result<()> {
    let account = app_account_key(key_name);
    let entry = keyring::Entry::new(SERVICE_NAME, &account)
        .context("Failed to create keyring entry")?;
    entry
        .set_password(value)
        .context("Failed to store app secret in keychain")?;
    log::info!("Stored app secret '{}' in keychain", key_name);
    Ok(())
}

/// Delete an app-level secret from the OS keychain.
pub fn delete_app_secret(key_name: &str) -> Result<()> {
    let account = app_account_key(key_name);
    let entry = keyring::Entry::new(SERVICE_NAME, &account)
        .context("Failed to create keyring entry")?;
    entry
        .delete_credential()
        .context(format!("Failed to delete app secret '{}'", key_name))
}

/// Retrieve an app-level secret from the OS keychain.
pub fn get_app_secret(key_name: &str) -> Result<String> {
    let account = app_account_key(key_name);
    let entry = keyring::Entry::new(SERVICE_NAME, &account)
        .context("Failed to create keyring entry")?;
    entry
        .get_password()
        .context(format!("Failed to retrieve app secret '{}'", key_name))
}

/// Check whether an app-level secret exists in the OS keychain.
pub fn has_app_secret(key_name: &str) -> bool {
    let account = app_account_key(key_name);
    let entry = match keyring::Entry::new(SERVICE_NAME, &account) {
        Ok(e) => e,
        Err(_) => return false,
    };
    entry.get_password().is_ok()
}

/// Get the app data directory path as a string.
pub fn get_app_data_dir_string() -> Result<String> {
    let dir = persistence::data_dir()?;
    Ok(dir.to_string_lossy().to_string())
}
