use crate::engine::audit;
use crate::engine::bootstrap::{self, ApplyMode, BootstrapReport};
use crate::engine::importers::{
    self, dotenv::DotEnvImporter, flyio::FlyImporter, railway::RailwayImporter,
    vercel::VercelImporter, ApplyOptions, ApplyReport, ImportContext, ImportReport, Importer,
};
use crate::engine::models::{EnvironmentsConfig, SecretsConfig};
use crate::engine::pipeline;
use crate::engine::preflight;
use crate::engine::secret_audit::{self, Provenance, SecretAudit};
use crate::engine::secrets;
use std::path::{Path, PathBuf};

/// Load environments config from a project's .chibby/environments.toml (committed file only).
#[tauri::command]
pub fn load_environments(repo_path: String) -> Result<EnvironmentsConfig, String> {
    pipeline::load_environments(Path::new(&repo_path)).map_err(|e| e.to_string())
}

/// Load environments with `environments.local.toml` overrides applied.
/// Use this for read-only/run-time views; use `load_environments` for editing the committed file.
#[tauri::command]
pub fn load_environments_layered(repo_path: String) -> Result<EnvironmentsConfig, String> {
    pipeline::load_environments_layered(Path::new(&repo_path)).map_err(|e| e.to_string())
}

/// Load per-developer overrides from `.chibby/environments.local.toml`.
#[tauri::command]
pub fn load_environments_local(repo_path: String) -> Result<EnvironmentsConfig, String> {
    pipeline::load_environments_local(Path::new(&repo_path)).map_err(|e| e.to_string())
}

/// Save the committed environments config.
#[tauri::command]
pub fn save_environments(repo_path: String, config: EnvironmentsConfig) -> Result<(), String> {
    let env_names: Vec<&str> = config
        .environments
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    audit::log_event(
        "save_environments",
        &format!("project={} envs={:?}", repo_path, env_names),
    );
    pipeline::save_environments(Path::new(&repo_path), &config).map_err(|e| e.to_string())
}

/// Save per-developer overrides (auto-adds `.gitignore` entries).
#[tauri::command]
pub fn save_environments_local(
    repo_path: String,
    config: EnvironmentsConfig,
) -> Result<(), String> {
    let env_names: Vec<&str> = config
        .environments
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    audit::log_event(
        "save_environments_local",
        &format!("project={} envs={:?}", repo_path, env_names),
    );
    pipeline::save_environments_local(Path::new(&repo_path), &config).map_err(|e| e.to_string())
}

/// Load secrets config from a project's .chibby/secrets.toml.
#[tauri::command]
pub fn load_secrets_config(repo_path: String) -> Result<SecretsConfig, String> {
    pipeline::load_secrets_config(Path::new(&repo_path)).map_err(|e| e.to_string())
}

/// Save secrets config to a project's .chibby/secrets.toml.
#[tauri::command]
pub fn save_secrets_config(repo_path: String, config: SecretsConfig) -> Result<(), String> {
    pipeline::save_secrets_config(Path::new(&repo_path), &config).map_err(|e| e.to_string())
}

/// Store a secret value in the OS keychain.
#[tauri::command]
pub fn set_secret(
    project_path: String,
    env_name: String,
    secret_name: String,
    value: String,
) -> Result<(), String> {
    audit::log_event(
        "set_secret",
        &format!(
            "project={} env={} secret={}",
            project_path, env_name, secret_name
        ),
    );
    secrets::set_secret(&project_path, &env_name, &secret_name, &value)
        .map_err(|e| e.to_string())?;
    secret_audit::record_set_quietly(&project_path, &env_name, &secret_name, Provenance::Gui);
    Ok(())
}

/// Delete a secret from the OS keychain.
#[tauri::command]
pub fn delete_secret(
    project_path: String,
    env_name: String,
    secret_name: String,
) -> Result<(), String> {
    audit::log_event(
        "delete_secret",
        &format!(
            "project={} env={} secret={}",
            project_path, env_name, secret_name
        ),
    );
    secrets::delete_secret(&project_path, &env_name, &secret_name).map_err(|e| e.to_string())?;
    secret_audit::record_delete_quietly(&project_path, &env_name, &secret_name, Provenance::Gui);
    Ok(())
}

/// Fetch the per-secret audit snapshot for the GUI's Secrets panel.
#[tauri::command]
pub fn get_secret_audit(
    project_path: String,
    env_name: String,
    secret_name: String,
) -> Result<Option<SecretAudit>, String> {
    secret_audit::get(&project_path, &env_name, &secret_name).map_err(|e| e.to_string())
}

/// Scan environments.toml for variable values that look like real credentials.
#[tauri::command]
pub fn scan_environments_for_leaks(repo_path: String) -> Result<Vec<pipeline::EnvLeakHit>, String> {
    pipeline::scan_environments_for_leaks(Path::new(&repo_path)).map_err(|e| e.to_string())
}

/// Check which secrets are set in the keychain for a given environment.
#[tauri::command]
pub fn check_secrets_status(
    project_path: String,
    env_name: String,
) -> Result<Vec<secrets::SecretStatus>, String> {
    let secrets_config =
        pipeline::load_secrets_config(Path::new(&project_path)).map_err(|e| e.to_string())?;
    Ok(secrets::check_secrets_status(
        &project_path,
        &env_name,
        &secrets_config,
    ))
}

/// Test SSH connectivity to a host.
#[tauri::command]
pub async fn test_ssh_connection(host: String, port: Option<u16>) -> Result<String, String> {
    preflight::test_ssh_connectivity(&host, port)
        .await
        .map_err(|e| e.to_string())
}

/// Scan a project for env/secret references without writing anything.
#[tauri::command]
pub fn scan_bootstrap(repo_path: String) -> Result<BootstrapReport, String> {
    bootstrap::scan_project(Path::new(&repo_path)).map_err(|e| e.to_string())
}

/// Apply a previously-generated `BootstrapReport` to the project.
/// `merge=true` appends missing names to existing configs; `merge=false` is the
/// "Safe" mode that refuses if either config already exists.
#[tauri::command]
pub fn apply_bootstrap(
    repo_path: String,
    report: BootstrapReport,
    merge: bool,
) -> Result<bool, String> {
    let mode = if merge {
        ApplyMode::Merge
    } else {
        ApplyMode::Safe
    };
    audit::log_event(
        "apply_bootstrap",
        &format!(
            "project={} mode={:?} detected={}",
            repo_path,
            mode,
            report.detected.len()
        ),
    );
    bootstrap::apply_bootstrap(Path::new(&repo_path), &report, mode).map_err(|e| e.to_string())
}

/// Result of `auto_bootstrap_for_project`.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AutoBootstrapOutcome {
    /// Which behaviour was applied this run.
    pub mode: String,
    /// The scan result. `None` when mode is `off`.
    pub report: Option<BootstrapReport>,
    /// `true` if files were written this call. `false` when in confirm mode
    /// (UI should show review), when mode is off, or when nothing was detected.
    pub applied: bool,
}

/// Honor `AppSettings.bootstrap_mode` and either scan-only (Confirm),
/// scan-and-apply (Silent), or do nothing (Off). Called by the Add Project
/// wizard after `add_project` resolves. Also seeds a default
/// `.chibby/gates.toml` so the Quality tab and pipeline-regen are populated
/// out of the box (won't overwrite an existing file).
#[tauri::command]
pub fn auto_bootstrap_for_project(repo_path: String) -> Result<AutoBootstrapOutcome, String> {
    use crate::engine::app_settings::{load_app_settings, BootstrapMode};

    // Always ensure a default gates.toml exists for the project — independent
    // of bootstrap mode. Failure here is non-fatal; the user can recreate it
    // manually via `chibby gates init` or the Quality tab.
    if let Err(e) = ensure_default_gates_toml(Path::new(&repo_path)) {
        log::warn!("ensure_default_gates_toml failed for {}: {}", repo_path, e);
    }

    let settings = load_app_settings().map_err(|e| e.to_string())?;
    let mode_label = match settings.bootstrap_mode {
        BootstrapMode::Confirm => "confirm",
        BootstrapMode::Silent => "silent",
        BootstrapMode::Off => "off",
    };
    if settings.bootstrap_mode == BootstrapMode::Off {
        return Ok(AutoBootstrapOutcome {
            mode: mode_label.to_string(),
            report: None,
            applied: false,
        });
    }
    let report = bootstrap::scan_project(Path::new(&repo_path)).map_err(|e| e.to_string())?;
    if report.detected.is_empty() {
        return Ok(AutoBootstrapOutcome {
            mode: mode_label.to_string(),
            report: Some(report),
            applied: false,
        });
    }
    let applied = if settings.bootstrap_mode == BootstrapMode::Silent {
        bootstrap::apply_bootstrap(Path::new(&repo_path), &report, ApplyMode::Merge)
            .map_err(|e| e.to_string())?
    } else {
        false
    };
    audit::log_event(
        "auto_bootstrap",
        &format!(
            "project={} mode={} detected={} applied={}",
            repo_path,
            mode_label,
            report.detected.len(),
            applied
        ),
    );
    Ok(AutoBootstrapOutcome {
        mode: mode_label.to_string(),
        report: Some(report),
        applied,
    })
}

/// Run an importer (dotenv | vercel | railway | fly) and apply its report.
#[tauri::command]
pub fn run_importer(
    source: String,
    repo_path: String,
    env_name: String,
    source_path: Option<String>,
    with_values: bool,
    persist_secret_values: bool,
) -> Result<(ImportReport, ApplyReport), String> {
    let ctx = ImportContext {
        repo_path: PathBuf::from(&repo_path),
        env_name: env_name.clone(),
        source_path: source_path.map(PathBuf::from),
        include_values: with_values,
    };
    let report: ImportReport = match source.as_str() {
        "dotenv" => DotEnvImporter.run(&ctx).map_err(|e| e.to_string())?,
        "vercel" => VercelImporter.run(&ctx).map_err(|e| e.to_string())?,
        "railway" => RailwayImporter.run(&ctx).map_err(|e| e.to_string())?,
        "fly" | "flyio" => FlyImporter.run(&ctx).map_err(|e| e.to_string())?,
        other => return Err(format!("Unknown importer source: {}", other)),
    };
    let apply = importers::apply_report(
        &report,
        Path::new(&repo_path),
        ApplyOptions {
            persist_variable_values: true,
            persist_secret_values,
        },
    )
    .map_err(|e| e.to_string())?;
    audit::log_event(
        "run_importer",
        &format!(
            "project={} source={} env={} variables={} secrets_refs={} secrets_saved={}",
            repo_path,
            source,
            env_name,
            apply.variables_added,
            apply.secrets_ref_added,
            apply.secrets_value_saved
        ),
    );
    Ok((report, apply))
}

/// Probe whether a vendor CLI required by an importer is installed.
#[tauri::command]
pub fn importer_cli_status(source: String) -> Result<bool, String> {
    let installed = match source.as_str() {
        "dotenv" => true,
        "vercel" => importers::cli_present("vercel"),
        "railway" => importers::cli_present("railway"),
        "fly" | "flyio" => importers::cli_present("flyctl") || importers::cli_present("fly"),
        other => return Err(format!("Unknown importer source: {}", other)),
    };
    Ok(installed)
}

/// Export resolved variables + secret values for an environment to a .env file.
#[tauri::command]
pub fn export_dotenv(
    repo_path: String,
    env_name: String,
    output_path: String,
) -> Result<usize, String> {
    audit::log_event(
        "export_dotenv",
        &format!("project={} env={} out={}", repo_path, env_name, output_path),
    );
    importers::export_dotenv(Path::new(&repo_path), &env_name, Path::new(&output_path))
        .map_err(|e| e.to_string())
}

/// Run preflight validation for a pipeline against an environment.
#[tauri::command]
pub async fn run_preflight(
    repo_path: String,
    environment: String,
) -> Result<preflight::PreflightResult, String> {
    let path = Path::new(&repo_path);
    let pipe = pipeline::load_pipeline(path).map_err(|e| e.to_string())?;
    let envs = pipeline::load_environments(path).map_err(|e| e.to_string())?;
    let secs = pipeline::load_secrets_config(path).map_err(|e| e.to_string())?;

    preflight::validate_preflight(&pipe, &repo_path, &environment, &envs, &secs)
        .await
        .map_err(|e| e.to_string())
}

/// Write a default `.chibby/gates.toml` if none exists yet. Used by
/// `auto_bootstrap_for_project` to seed new projects with security gates
/// configured (all in `warn` mode initially so first runs surface findings
/// without blocking the user).
fn ensure_default_gates_toml(repo_path: &Path) -> Result<(), anyhow::Error> {
    use crate::engine::gates;
    use crate::engine::models::{GateMode, GatesConfig};

    let target = repo_path.join(".chibby").join("gates.toml");
    if target.exists() {
        return Ok(());
    }
    let mut cfg = GatesConfig::default();
    // Sensible defaults: warn everywhere so a fresh project sees findings
    // surfaced in the Quality tab without breaking pipelines. Users can bump
    // anything to "block" after triaging the initial baseline.
    cfg.secret_scanning = GateMode::Warn;
    cfg.dependency_scanning = GateMode::Warn;
    cfg.commit_lint = GateMode::Warn;
    cfg.sast = GateMode::Warn;
    cfg.container_scan = GateMode::Warn;
    cfg.iac_scan = GateMode::Warn;
    cfg.license_check = GateMode::Warn;
    cfg.secret_scan_baseline = true;
    cfg.secret_scan_allowlist = vec![
        "**/__tests__/**".into(),
        "**/__mocks__/**".into(),
        "**/tests/**".into(),
        "**/test/**".into(),
        "**/*.test.ts".into(),
        "**/*.test.tsx".into(),
        "**/*.spec.ts".into(),
        "**/node_modules/**".into(),
        "**/dist/**".into(),
        "**/build/**".into(),
        "**/.next/**".into(),
        "**/.vercel/**".into(),
    ];
    gates::save_gates_config(repo_path, &cfg)?;
    audit::log_event(
        "auto_seed_gates",
        &format!("project={}", repo_path.display()),
    );
    Ok(())
}
