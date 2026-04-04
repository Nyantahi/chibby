use crate::engine::models::SecretsConfig;
use anyhow::{Context, Result};
use std::collections::HashMap;

const SERVICE_NAME: &str = "chibby";

/// Encode a segment for use in a keychain account key.
/// Replaces the delimiter character '|' so no segment can spoof another.
fn encode_key_segment(s: &str) -> String {
    s.replace('%', "%25").replace('|', "%7C")
}

/// Build a deterministic keychain account key for a secret.
/// Uses '|' as delimiter with percent-encoding to prevent key collisions.
fn account_key(project_path: &str, env_name: &str, secret_name: &str) -> String {
    format!(
        "{}|{}|{}",
        encode_key_segment(project_path),
        encode_key_segment(env_name),
        encode_key_segment(secret_name)
    )
}

/// Store a secret value in the OS keychain.
pub fn set_secret(
    project_path: &str,
    env_name: &str,
    secret_name: &str,
    value: &str,
) -> Result<()> {
    let account = account_key(project_path, env_name, secret_name);
    let entry = keyring::Entry::new(SERVICE_NAME, &account)
        .context("Failed to create keyring entry")?;
    entry
        .set_password(value)
        .context("Failed to store secret in keychain")?;
    log::info!("Stored secret '{}' for env '{}' in keychain", secret_name, env_name);
    Ok(())
}

/// Retrieve a secret value from the OS keychain.
pub fn get_secret(
    project_path: &str,
    env_name: &str,
    secret_name: &str,
) -> Result<String> {
    let account = account_key(project_path, env_name, secret_name);
    let entry = keyring::Entry::new(SERVICE_NAME, &account)
        .context("Failed to create keyring entry")?;
    entry
        .get_password()
        .context(format!("Secret '{}' not found in keychain for env '{}'", secret_name, env_name))
}

/// Delete a secret from the OS keychain.
pub fn delete_secret(
    project_path: &str,
    env_name: &str,
    secret_name: &str,
) -> Result<()> {
    let account = account_key(project_path, env_name, secret_name);
    let entry = keyring::Entry::new(SERVICE_NAME, &account)
        .context("Failed to create keyring entry")?;
    entry
        .delete_credential()
        .context(format!("Failed to delete secret '{}' for env '{}'", secret_name, env_name))
}

/// Check whether a secret exists in the OS keychain.
pub fn has_secret(
    project_path: &str,
    env_name: &str,
    secret_name: &str,
) -> bool {
    let account = match account_key(project_path, env_name, secret_name).parse::<String>() {
        Ok(a) => a,
        Err(_) => return false,
    };
    let entry = match keyring::Entry::new(SERVICE_NAME, &account) {
        Ok(e) => e,
        Err(_) => return false,
    };
    entry.get_password().is_ok()
}

/// Status of a single secret in the keychain.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SecretStatus {
    pub name: String,
    pub is_set: bool,
}

/// Check which secrets are set for a given environment.
pub fn check_secrets_status(
    project_path: &str,
    env_name: &str,
    secrets_config: &SecretsConfig,
) -> Vec<SecretStatus> {
    secrets_config
        .secrets
        .iter()
        .filter(|s| s.environments.is_empty() || s.environments.contains(&env_name.to_string()))
        .map(|s| SecretStatus {
            name: s.name.clone(),
            is_set: has_secret(project_path, env_name, &s.name),
        })
        .collect()
}

/// Resolve all secrets for an environment into a name->value map.
/// Returns an error if any required secrets are missing from the keychain.
pub fn resolve_secrets_for_env(
    project_path: &str,
    env_name: &str,
    secrets_config: &SecretsConfig,
) -> Result<HashMap<String, String>> {
    let mut resolved = HashMap::new();
    let mut missing = Vec::new();

    for secret_ref in &secrets_config.secrets {
        // Skip secrets not applicable to this environment.
        if !secret_ref.environments.is_empty()
            && !secret_ref.environments.contains(&env_name.to_string())
        {
            continue;
        }

        match get_secret(project_path, env_name, &secret_ref.name) {
            Ok(value) => {
                resolved.insert(secret_ref.name.clone(), value);
            }
            Err(_) => {
                missing.push(secret_ref.name.clone());
            }
        }
    }

    if !missing.is_empty() {
        anyhow::bail!(
            "Missing secrets for environment '{}': {}",
            env_name,
            missing.join(", ")
        );
    }

    Ok(resolved)
}
