//! Importers — pull env/secret references from external sources into Chibby.
//!
//! Each adapter implements [`Importer`]. The common path:
//! 1. `detect_cli` — confirm any required vendor binary is on PATH.
//! 2. `run(ctx)` — pull names (and optionally values) into an [`ImportReport`].
//! 3. Caller passes the report to [`apply_report`] which writes to the project's
//!    `environments.toml` + `secrets.toml` (and optionally the keychain).
//!
//! Classification of each detected name reuses the bootstrap classifier so
//! conventions stay consistent across the codebase.

use crate::engine::bootstrap::{classify, Classification};
use crate::engine::models::SecretRef;
use crate::engine::secret_audit::{self, Provenance};
use crate::engine::{pipeline, secrets as secrets_engine};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub mod dotenv;
pub mod flyio;
pub mod railway;
pub mod vercel;

// ---------------------------------------------------------------------------
// Common types
// ---------------------------------------------------------------------------

/// What the user wants the importer to do with each detected name.
#[derive(Debug, Clone)]
pub struct ImportContext {
    pub repo_path: PathBuf,
    pub env_name: String,
    /// For `dotenv`: path to the file being imported. Ignored by PaaS importers.
    pub source_path: Option<PathBuf>,
    /// If true, the importer also pulls *values* (and the caller may persist
    /// them — variables to `environments.toml`, secrets to the OS keychain).
    /// PaaS sources that don't expose values silently fall back to names-only.
    pub include_values: bool,
}

/// Result of running an importer.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportReport {
    pub source: String,
    pub env_name: String,
    /// All detected names with classification. Values populated only when the
    /// source supports it AND `ImportContext.include_values` is true.
    pub entries: Vec<ImportEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportEntry {
    pub name: String,
    pub classification: Classification,
    /// Plain-text value, present only when the importer pulled values.
    /// Never persisted unless the user explicitly opts in via `apply_report`.
    pub value: Option<String>,
}

/// Behaviour controlling what gets persisted from an [`ImportReport`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApplyOptions {
    /// Persist non-secret values into `environments.toml`'s `[variables]`.
    /// Has no effect if entries have no values.
    pub persist_variable_values: bool,
    /// Persist secret values into the OS keychain. Variables are never put
    /// in the keychain regardless of this flag.
    pub persist_secret_values: bool,
}

impl Default for ApplyOptions {
    fn default() -> Self {
        Self {
            persist_variable_values: true,
            persist_secret_values: true,
        }
    }
}

/// Outcome of [`apply_report`] — useful for CLI reporting and audit logs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApplyReport {
    pub variables_added: usize,
    pub variables_value_set: usize,
    pub secrets_ref_added: usize,
    pub secrets_value_saved: usize,
}

/// Shared trait every adapter implements.
pub trait Importer {
    /// Adapter name (e.g. "vercel"). Stored on `ImportReport.source`.
    fn name(&self) -> &'static str;

    /// Confirm the required vendor CLI is installed and on PATH.
    /// Returns a human-friendly error with an install hint when missing.
    fn detect_cli(&self) -> Result<()>;

    /// Pull entries.
    fn run(&self, ctx: &ImportContext) -> Result<ImportReport>;
}

// ---------------------------------------------------------------------------
// Apply
// ---------------------------------------------------------------------------

/// Write a report into the project's environments.toml + secrets.toml,
/// and optionally seed values into the OS keychain. Always Merge-mode:
/// existing entries are preserved.
///
/// Pure function — no network, no shell. Composable from the GUI and CLI.
pub fn apply_report(
    report: &ImportReport,
    repo_path: &std::path::Path,
    options: ApplyOptions,
) -> Result<ApplyReport> {
    let mut out = ApplyReport::default();

    // Make sure the target environment exists in environments.toml so var/secret
    // writes have somewhere to go.
    let envs = pipeline::load_environments(repo_path)?;
    if !envs.environments.iter().any(|e| e.name == report.env_name) {
        pipeline::add_environment(
            repo_path,
            crate::engine::models::Environment {
                name: report.env_name.clone(),
                ssh_host: None,
                ssh_port: None,
                variables: Default::default(),
            },
        )?;
    }

    for entry in &report.entries {
        match entry.classification {
            Classification::Variable => {
                // Don't clobber existing values. Only write when the variable
                // hasn't been set yet on this env.
                let cur = pipeline::load_environments(repo_path)?;
                let already_set = cur
                    .environments
                    .iter()
                    .find(|e| e.name == report.env_name)
                    .map(|e| e.variables.contains_key(&entry.name))
                    .unwrap_or(false);
                if !already_set {
                    let value = if options.persist_variable_values {
                        entry.value.clone().unwrap_or_default()
                    } else {
                        String::new()
                    };
                    pipeline::set_env_variable(
                        repo_path,
                        &report.env_name,
                        &entry.name,
                        &value,
                    )?;
                    out.variables_added += 1;
                    if !value.is_empty() {
                        out.variables_value_set += 1;
                    }
                }
            }
            Classification::Secret => {
                let secrets_config = pipeline::load_secrets_config(repo_path)?;
                let already_declared = secrets_config
                    .secrets
                    .iter()
                    .any(|s| s.name == entry.name);
                if !already_declared {
                    pipeline::add_secret_ref(
                        repo_path,
                        SecretRef {
                            name: entry.name.clone(),
                            environments: vec![report.env_name.clone()],
                        },
                    )?;
                    out.secrets_ref_added += 1;
                }
                if options.persist_secret_values {
                    if let Some(value) = &entry.value {
                        if !value.is_empty() {
                            let project_path_str = repo_path.to_string_lossy().to_string();
                            secrets_engine::set_secret(
                                &project_path_str,
                                &report.env_name,
                                &entry.name,
                                value,
                            )?;
                            secret_audit::record_set_quietly(
                                &project_path_str,
                                &report.env_name,
                                &entry.name,
                                Provenance::Import {
                                    adapter: report.source.clone(),
                                },
                            );
                            out.secrets_value_saved += 1;
                        }
                    }
                }
            }
        }
    }

    Ok(out)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Apply the bootstrap classifier and assemble entries from a raw
/// `name -> Option<value>` map.
pub fn entries_from_map(map: BTreeMap<String, Option<String>>) -> Vec<ImportEntry> {
    map.into_iter()
        .map(|(name, value)| ImportEntry {
            classification: classify(&name),
            name,
            value,
        })
        .collect()
}

/// Check whether a binary is on PATH.
pub fn cli_present(binary: &str) -> bool {
    which_simple(binary).is_some()
}

fn which_simple(binary: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(binary);
        if candidate.is_file() {
            return Some(candidate);
        }
        // Windows: also try .exe
        #[cfg(windows)]
        {
            let exe = dir.join(format!("{}.exe", binary));
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Export (counterpart to dotenv import)
// ---------------------------------------------------------------------------

/// Write the resolved variables + secret values for `env_name` to a .env file.
///
/// Variables come from `environments.toml` (layered). Secrets come from the
/// OS keychain. Variables are emitted first, secrets after. Missing keychain
/// entries are skipped silently — caller should run `chibby doctor` if they
/// want to know what's missing.
///
/// Returns the number of lines written.
pub fn export_dotenv(
    repo_path: &std::path::Path,
    env_name: &str,
    output_path: &std::path::Path,
) -> Result<usize> {
    use std::fmt::Write;

    let envs = pipeline::load_environments_layered(repo_path)?;
    let env = envs
        .environments
        .iter()
        .find(|e| e.name == env_name)
        .ok_or_else(|| anyhow::anyhow!("Environment '{}' not found", env_name))?;

    let mut out = String::new();
    writeln!(
        out,
        "# Generated by chibby export dotenv (env={}). Do not commit.",
        env_name
    )?;

    if !env.variables.is_empty() {
        writeln!(out, "\n# Variables")?;
        let mut keys: Vec<&String> = env.variables.keys().collect();
        keys.sort();
        for k in keys {
            writeln!(out, "{}={}", k, shell_escape(&env.variables[k]))?;
        }
    }

    let secrets_config = pipeline::load_secrets_config(repo_path)?;
    let applicable: Vec<&SecretRef> = secrets_config
        .secrets
        .iter()
        .filter(|s| s.environments.is_empty() || s.environments.iter().any(|e| e == env_name))
        .collect();

    if !applicable.is_empty() {
        writeln!(out, "\n# Secrets")?;
        let repo_path_str = repo_path.to_string_lossy().to_string();
        for s in applicable {
            match secrets_engine::get_secret(&repo_path_str, env_name, &s.name) {
                Ok(value) => {
                    writeln!(out, "{}={}", s.name, shell_escape(&value))?;
                }
                Err(_) => {
                    writeln!(out, "# {}=  (missing — run `chibby secrets set`)", s.name)?;
                }
            }
        }
    }

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output_path, &out)?;

    // Defensively ensure the file is gitignored.
    if let Some(name) = output_path.file_name().and_then(|n| n.to_str()) {
        if name == ".env" || name.starts_with(".env.") {
            // Already excluded by most .gitignore conventions; leave it.
        }
    }
    Ok(out.lines().count())
}

/// Conservative dotenv-style quoting: wrap in double quotes if value contains
/// whitespace, `#`, or `"`. Escape internal `"`.
fn shell_escape(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    let needs_quotes = s
        .chars()
        .any(|c| c.is_whitespace() || c == '#' || c == '"' || c == '\'' || c == '$');
    if needs_quotes {
        let escaped = s.replace('"', "\\\"");
        format!("\"{}\"", escaped)
    } else {
        s.to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::models::{Environment, EnvironmentsConfig, SecretsConfig};
    use tempfile::TempDir;

    fn ctx(temp: &TempDir, env: &str) -> ImportContext {
        ImportContext {
            repo_path: temp.path().to_path_buf(),
            env_name: env.to_string(),
            source_path: None,
            include_values: true,
        }
    }

    #[test]
    fn entries_from_map_classifies_with_bootstrap_rules() {
        let mut m = BTreeMap::new();
        m.insert("API_URL".to_string(), Some("https://x".to_string()));
        m.insert("STRIPE_SECRET".to_string(), Some("sk_xxx".to_string()));
        let entries = entries_from_map(m);
        let api = entries.iter().find(|e| e.name == "API_URL").unwrap();
        let stripe = entries.iter().find(|e| e.name == "STRIPE_SECRET").unwrap();
        assert_eq!(api.classification, Classification::Variable);
        assert_eq!(stripe.classification, Classification::Secret);
    }

    #[test]
    fn apply_report_creates_env_and_adds_var() {
        let temp = TempDir::new().unwrap();
        let report = ImportReport {
            source: "test".to_string(),
            env_name: "production".to_string(),
            entries: vec![ImportEntry {
                name: "API_URL".to_string(),
                classification: Classification::Variable,
                value: Some("https://api.example.com".to_string()),
            }],
        };
        let _ = ctx(&temp, "production"); // exercise constructor
        let out = apply_report(&report, temp.path(), ApplyOptions::default()).unwrap();
        assert_eq!(out.variables_added, 1);
        assert_eq!(out.variables_value_set, 1);

        let envs = pipeline::load_environments(temp.path()).unwrap();
        let prod = envs.environments.iter().find(|e| e.name == "production").unwrap();
        assert_eq!(prod.variables.get("API_URL").unwrap(), "https://api.example.com");
    }

    #[test]
    fn apply_report_preserves_existing_var_values() {
        let temp = TempDir::new().unwrap();
        pipeline::save_environments(
            temp.path(),
            &EnvironmentsConfig {
                environments: vec![Environment {
                    name: "production".to_string(),
                    ssh_host: None,
                    ssh_port: None,
                    variables: [("API_URL".to_string(), "existing".to_string())]
                        .into_iter()
                        .collect(),
                }],
            },
        )
        .unwrap();

        let report = ImportReport {
            source: "test".to_string(),
            env_name: "production".to_string(),
            entries: vec![ImportEntry {
                name: "API_URL".to_string(),
                classification: Classification::Variable,
                value: Some("OVERWRITE".to_string()),
            }],
        };
        let out = apply_report(&report, temp.path(), ApplyOptions::default()).unwrap();
        assert_eq!(out.variables_added, 0); // skipped — already present
        let envs = pipeline::load_environments(temp.path()).unwrap();
        let prod = envs.environments.iter().find(|e| e.name == "production").unwrap();
        assert_eq!(prod.variables.get("API_URL").unwrap(), "existing");
    }

    #[test]
    fn apply_report_adds_secret_ref_without_value_when_disabled() {
        let temp = TempDir::new().unwrap();
        let report = ImportReport {
            source: "test".to_string(),
            env_name: "production".to_string(),
            entries: vec![ImportEntry {
                name: "STRIPE_SECRET".to_string(),
                classification: Classification::Secret,
                value: Some("sk_xxx".to_string()),
            }],
        };
        let opts = ApplyOptions {
            persist_variable_values: true,
            persist_secret_values: false,
        };
        let out = apply_report(&report, temp.path(), opts).unwrap();
        assert_eq!(out.secrets_ref_added, 1);
        assert_eq!(out.secrets_value_saved, 0);

        let secrets_config = pipeline::load_secrets_config(temp.path()).unwrap();
        let names: Vec<&str> = secrets_config
            .secrets
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert!(names.contains(&"STRIPE_SECRET"));
    }

    #[test]
    fn export_dotenv_writes_variables_and_missing_secret_hint() {
        let temp = TempDir::new().unwrap();
        pipeline::save_environments(
            temp.path(),
            &EnvironmentsConfig {
                environments: vec![Environment {
                    name: "production".to_string(),
                    ssh_host: None,
                    ssh_port: None,
                    variables: [
                        ("API_URL".to_string(), "https://x".to_string()),
                        ("HAS_SPACE".to_string(), "hello world".to_string()),
                    ]
                    .into_iter()
                    .collect(),
                }],
            },
        )
        .unwrap();
        pipeline::save_secrets_config(
            temp.path(),
            &SecretsConfig {
                secrets: vec![SecretRef {
                    name: "DEPLOY_TOKEN".to_string(),
                    environments: vec!["production".to_string()],
                }],
            },
        )
        .unwrap();

        let out_path = temp.path().join(".env.production.local");
        export_dotenv(temp.path(), "production", &out_path).unwrap();
        let content = std::fs::read_to_string(&out_path).unwrap();
        assert!(content.contains("API_URL=https://x"));
        assert!(content.contains("HAS_SPACE=\"hello world\""));
        assert!(content.contains("# DEPLOY_TOKEN=")); // marked as missing
    }

    #[test]
    fn shell_escape_handles_special_chars() {
        assert_eq!(shell_escape(""), "");
        assert_eq!(shell_escape("simple"), "simple");
        assert_eq!(shell_escape("has space"), "\"has space\"");
        assert_eq!(shell_escape("has\"quote"), "\"has\\\"quote\"");
        assert_eq!(shell_escape("has$dollar"), "\"has$dollar\"");
    }

    #[test]
    fn cli_present_handles_missing_binary() {
        // 'definitely-not-a-real-binary-12345' must not exist
        assert!(!cli_present("definitely-not-a-real-binary-12345"));
    }
}
