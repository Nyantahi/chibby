//! Auto-bootstrap for `.chibby/environments.toml` + `.chibby/secrets.toml`.
//!
//! Scans the project for environment-variable references and classifies each
//! detected name as either a *secret* (value belongs in the OS keychain) or a
//! *variable* (value lives in committed `environments.toml`). Produces a
//! `BootstrapReport` the caller can review or apply.
//!
//! Sources scanned:
//! - `.env*` files (key names only — values discarded)
//! - `docker-compose*.yml` (`${VAR}` interpolations, `env_file:` includes)
//! - `.github/workflows/*.yml` (`${{ secrets.X }}` references)
//! - Source code patterns:
//!   - JS/TS: `process.env.X`, `process.env["X"]`
//!   - Python: `os.getenv("X")`, `os.environ["X"]`, `os.environ.get("X")`
//!   - Rust: `env::var("X")`, `std::env::var("X")`
//!
//! Classification is name-based and conservative: when ambiguous, default to
//! "variable" — a misclassified non-secret in `environments.toml` is recoverable,
//! a non-secret stashed in the keychain is friction.

use crate::engine::models::{Environment, SecretRef};
use crate::engine::pipeline;
use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Report types
// ---------------------------------------------------------------------------

/// Where a name was discovered.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    DotEnv,
    DockerCompose,
    GhaWorkflow,
    JsCode,
    PyCode,
    RsCode,
}

/// A single detection event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionSource {
    pub path: String,
    pub kind: SourceKind,
}

/// Final classification for a detected name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Classification {
    Secret,
    Variable,
}

/// One detected name with classification + provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedName {
    pub name: String,
    pub classification: Classification,
    pub sources: Vec<DetectionSource>,
}

/// Top-level result of a project scan.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BootstrapReport {
    pub detected: Vec<DetectedName>,
    /// Environment names suggested by the scan (e.g. "production" from
    /// `docker-compose.prod.yml` filename).
    pub suggested_environments: Vec<String>,
    pub scanned_files: usize,
}

impl BootstrapReport {
    pub fn secrets(&self) -> impl Iterator<Item = &DetectedName> {
        self.detected
            .iter()
            .filter(|d| d.classification == Classification::Secret)
    }

    pub fn variables(&self) -> impl Iterator<Item = &DetectedName> {
        self.detected
            .iter()
            .filter(|d| d.classification == Classification::Variable)
    }
}

// ---------------------------------------------------------------------------
// Classifier
// ---------------------------------------------------------------------------

/// Substrings that strongly indicate a value is sensitive.
/// Matched against `_`-segmented words so `KEYBOARD` doesn't match `KEY`.
const SECRET_INDICATORS: &[&str] = &[
    "TOKEN",
    "SECRET",
    "PASSWORD",
    "PASSWD",
    "PAT",
    "CREDENTIAL",
    "CREDENTIALS",
    "PRIVATE",
    "APIKEY",
    "SIGNING",
    "WEBHOOK",
    "DSN",
    "BEARER",
];

/// Substrings that strongly indicate a value is non-sensitive config.
/// These wins over a fuzzy secret match (e.g. `HOSTNAME` is config, not secret).
const VARIABLE_INDICATORS: &[&str] = &[
    "URL",
    "HOST",
    "HOSTNAME",
    "PORT",
    "PATH",
    "DIR",
    "DIRECTORY",
    "NAME",
    "MODE",
    "ENV",
    "REGION",
    "VERSION",
    "STAGE",
    "TIMEOUT",
];

/// Word-segment classification. Splits on `_` and `-` and matches whole segments
/// (so `MY_KEY` matches `KEY`, but `MONKEY` does not).
pub fn classify(name: &str) -> Classification {
    let upper = name.to_uppercase();
    let segments: Vec<&str> = upper.split(['_', '-']).collect();

    // Variable indicators win on collision.
    for seg in &segments {
        if VARIABLE_INDICATORS.contains(seg) {
            return Classification::Variable;
        }
    }

    // KEY is special — common as both ("API_KEY", "SSH_KEY" → secret;
    // "KEY_PREFIX" → variable). The variable-indicator pass already caught
    // most variable-y forms; if KEY appears as a segment now, treat as secret.
    if segments.iter().any(|s| *s == "KEY") {
        return Classification::Secret;
    }

    for seg in &segments {
        if SECRET_INDICATORS.contains(seg) {
            return Classification::Secret;
        }
    }
    // Substring fallback for `APIKEY` etc. (no underscore).
    if SECRET_INDICATORS
        .iter()
        .any(|ind| upper.contains(ind) && ind.len() >= 5)
    {
        return Classification::Secret;
    }

    Classification::Variable
}

// ---------------------------------------------------------------------------
// Scanners
// ---------------------------------------------------------------------------

/// Directories never worth scanning.
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "venv",
    ".venv",
    "__pycache__",
    "dist",
    "build",
    ".git",
    ".chibby",
    ".next",
    ".nuxt",
    "coverage",
];

/// File extensions considered "source" for env-var pattern matching.
const SOURCE_EXTENSIONS: &[(&str, SourceKind)] = &[
    ("js", SourceKind::JsCode),
    ("jsx", SourceKind::JsCode),
    ("ts", SourceKind::JsCode),
    ("tsx", SourceKind::JsCode),
    ("mjs", SourceKind::JsCode),
    ("cjs", SourceKind::JsCode),
    ("py", SourceKind::PyCode),
    ("rs", SourceKind::RsCode),
];

/// Heuristic cap on directory recursion depth. Keeps the scan snappy on giant repos.
const MAX_DEPTH: usize = 8;

/// Run all scanners and return a `BootstrapReport`.
pub fn scan_project(repo_path: &Path) -> Result<BootstrapReport> {
    let mut detections: BTreeMap<String, Vec<DetectionSource>> = BTreeMap::new();
    let mut envs: BTreeSet<String> = BTreeSet::new();
    let mut files_scanned: usize = 0;

    visit_dir(repo_path, repo_path, 0, &mut |path: &Path| {
        files_scanned += 1;
        let rel = path.strip_prefix(repo_path).unwrap_or(path);
        let rel_str = rel.to_string_lossy().to_string();
        let fname = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        // .env / .env.production / .env.local
        if fname == ".env" || fname.starts_with(".env.") {
            if let Ok(content) = std::fs::read_to_string(path) {
                for name in parse_dotenv_keys(&content) {
                    push_detection(
                        &mut detections,
                        name,
                        DetectionSource {
                            path: rel_str.clone(),
                            kind: SourceKind::DotEnv,
                        },
                    );
                }
                if let Some(env_name) = env_name_from_dotenv_filename(fname) {
                    envs.insert(env_name);
                }
            }
            return;
        }

        // docker-compose*.yml / .yaml
        let is_compose = (fname.starts_with("docker-compose") || fname == "compose.yml")
            && (fname.ends_with(".yml") || fname.ends_with(".yaml"));
        if is_compose {
            if let Ok(content) = std::fs::read_to_string(path) {
                for name in parse_compose_vars(&content) {
                    push_detection(
                        &mut detections,
                        name,
                        DetectionSource {
                            path: rel_str.clone(),
                            kind: SourceKind::DockerCompose,
                        },
                    );
                }
                if let Some(env_name) = env_name_from_compose_filename(fname) {
                    envs.insert(env_name);
                }
            }
            return;
        }

        // .github/workflows/*.yml — secrets.X references
        if rel_str.starts_with(".github/workflows/")
            && (fname.ends_with(".yml") || fname.ends_with(".yaml"))
        {
            if let Ok(content) = std::fs::read_to_string(path) {
                for name in parse_gha_secrets(&content) {
                    push_detection(
                        &mut detections,
                        name,
                        DetectionSource {
                            path: rel_str.clone(),
                            kind: SourceKind::GhaWorkflow,
                        },
                    );
                }
            }
            return;
        }

        // Source code patterns
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if let Some((_, kind)) = SOURCE_EXTENSIONS.iter().find(|(e, _)| *e == ext) {
                if let Ok(content) = std::fs::read_to_string(path) {
                    let names = match kind {
                        SourceKind::JsCode => parse_js_env(&content),
                        SourceKind::PyCode => parse_py_env(&content),
                        SourceKind::RsCode => parse_rs_env(&content),
                        _ => Vec::new(),
                    };
                    for name in names {
                        push_detection(
                            &mut detections,
                            name,
                            DetectionSource {
                                path: rel_str.clone(),
                                kind: kind.clone(),
                            },
                        );
                    }
                }
            }
        }
    });

    if envs.is_empty() {
        envs.insert("production".to_string());
    }

    let detected: Vec<DetectedName> = detections
        .into_iter()
        .map(|(name, sources)| DetectedName {
            classification: classify(&name),
            name,
            sources,
        })
        .collect();

    Ok(BootstrapReport {
        detected,
        suggested_environments: envs.into_iter().collect(),
        scanned_files: files_scanned,
    })
}

fn push_detection(
    map: &mut BTreeMap<String, Vec<DetectionSource>>,
    name: String,
    source: DetectionSource,
) {
    if !is_valid_env_name(&name) {
        return;
    }
    map.entry(name).or_default().push(source);
}

/// Match shell `[A-Za-z_][A-Za-z0-9_]*` — same shape the executor enforces.
fn is_valid_env_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let first = name.as_bytes()[0];
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return false;
    }
    name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

// --- Per-source parsers ---

fn parse_dotenv_keys(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Strip optional leading "export "
        let line = line.strip_prefix("export ").unwrap_or(line);
        if let Some(eq) = line.find('=') {
            let key = line[..eq].trim();
            if !key.is_empty() {
                out.push(key.to_string());
            }
        }
    }
    out
}

fn parse_compose_vars(content: &str) -> Vec<String> {
    // Matches ${VAR}, ${VAR:-default}, ${VAR:default}, $VAR
    let re = Regex::new(r"\$\{?([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    let mut out = Vec::new();
    for cap in re.captures_iter(content) {
        if let Some(m) = cap.get(1) {
            out.push(m.as_str().to_string());
        }
    }
    out
}

fn parse_gha_secrets(content: &str) -> Vec<String> {
    // Matches ${{ secrets.NAME }}
    let re = Regex::new(r"\$\{\{\s*secrets\.([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    re.captures_iter(content)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

fn parse_js_env(content: &str) -> Vec<String> {
    // process.env.NAME  or  process.env["NAME"]  or  process.env['NAME']
    let dotted = Regex::new(r"process\.env\.([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    let bracketed = Regex::new(r#"process\.env\[["']([A-Za-z_][A-Za-z0-9_]*)["']\]"#).unwrap();
    let mut out: Vec<String> = dotted
        .captures_iter(content)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect();
    out.extend(
        bracketed
            .captures_iter(content)
            .filter_map(|c| c.get(1).map(|m| m.as_str().to_string())),
    );
    out
}

fn parse_py_env(content: &str) -> Vec<String> {
    // os.getenv("X" ...), os.environ["X"], os.environ.get("X" ...)
    let re = Regex::new(
        r#"os\.(?:getenv|environ\.get|environ)\s*[\(\[]\s*["']([A-Za-z_][A-Za-z0-9_]*)["']"#,
    )
    .unwrap();
    re.captures_iter(content)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

fn parse_rs_env(content: &str) -> Vec<String> {
    // env::var("X")  or  std::env::var("X")  or  env::var_os("X")
    let re = Regex::new(r#"env::var(?:_os)?\s*\(\s*"([A-Za-z_][A-Za-z0-9_]*)""#).unwrap();
    re.captures_iter(content)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

// --- Environment-name inference from filenames ---

fn env_name_from_dotenv_filename(fname: &str) -> Option<String> {
    // .env.production -> production, .env.local -> local
    fname.strip_prefix(".env.").map(|s| s.to_string())
}

fn env_name_from_compose_filename(fname: &str) -> Option<String> {
    // docker-compose.prod.yml -> production
    // docker-compose.staging.yml -> staging
    let stem = fname
        .trim_end_matches(".yml")
        .trim_end_matches(".yaml")
        .trim_start_matches("docker-compose")
        .trim_start_matches('.');
    match stem {
        "" => None,
        "prod" => Some("production".to_string()),
        other => Some(other.to_string()),
    }
}

// --- Directory walker ---

fn visit_dir(
    root: &Path,
    current: &Path,
    depth: usize,
    on_file: &mut dyn FnMut(&Path),
) {
    if depth > MAX_DEPTH {
        return;
    }
    let entries = match std::fs::read_dir(current) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if path.is_dir() {
            if SKIP_DIRS.contains(&name) {
                continue;
            }
            visit_dir(root, &path, depth + 1, on_file);
        } else if path.is_file() {
            on_file(&path);
        }
    }
}

// ---------------------------------------------------------------------------
// Apply
// ---------------------------------------------------------------------------

/// Behaviour when applying a report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyMode {
    /// Only create the configs if neither file exists. Never overwrite.
    /// Returns Ok(false) without writing if either file is present.
    Safe,
    /// Merge with existing configs — only add names that aren't already declared.
    Merge,
}

/// Write the detected names into `.chibby/environments.toml` + `.chibby/secrets.toml`.
/// In Safe mode, refuses if either file already exists.
/// In Merge mode, appends only newly-detected names; never modifies existing entries.
///
/// Variables are written *empty* into the first suggested environment — the user
/// (or a follow-up command) supplies values via `chibby env vars set` or the UI.
/// Secrets are written as references only; values still come from the keychain.
///
/// Returns Ok(true) if anything was written, Ok(false) if nothing to do.
pub fn apply_bootstrap(
    repo_path: &Path,
    report: &BootstrapReport,
    mode: ApplyMode,
) -> Result<bool> {
    let envs_present = pipeline::has_environments(repo_path);
    let secrets_present = pipeline::has_secrets_config(repo_path);

    if mode == ApplyMode::Safe && (envs_present || secrets_present) {
        return Ok(false);
    }

    let primary_env = report
        .suggested_environments
        .first()
        .cloned()
        .unwrap_or_else(|| "production".to_string());

    // --- environments.toml ---
    let mut envs_config = pipeline::load_environments(repo_path)?;
    let existing_env_names: BTreeSet<String> = envs_config
        .environments
        .iter()
        .map(|e| e.name.clone())
        .collect();

    for env_name in &report.suggested_environments {
        if !existing_env_names.contains(env_name) {
            envs_config.environments.push(Environment {
                name: env_name.clone(),
                ssh_host: None,
                ssh_port: None,
                variables: Default::default(),
            });
        }
    }

    if let Some(target) = envs_config
        .environments
        .iter_mut()
        .find(|e| e.name == primary_env)
    {
        for v in report.variables() {
            target.variables.entry(v.name.clone()).or_default();
        }
    }

    pipeline::save_environments(repo_path, &envs_config)?;

    // --- secrets.toml ---
    let mut secrets_config = pipeline::load_secrets_config(repo_path)?;
    let existing: BTreeSet<String> = secrets_config
        .secrets
        .iter()
        .map(|s| s.name.clone())
        .collect();
    for s in report.secrets() {
        if !existing.contains(&s.name) {
            secrets_config.secrets.push(SecretRef {
                name: s.name.clone(),
                environments: report.suggested_environments.clone(),
            });
        }
    }
    pipeline::save_secrets_config(repo_path, &secrets_config)?;

    Ok(true)
}

// ---------------------------------------------------------------------------
// Helpers for callers
// ---------------------------------------------------------------------------

/// Convenience — for callers that want a one-shot "scan + apply if safe".
pub fn bootstrap_if_safe(repo_path: &Path) -> Result<Option<BootstrapReport>> {
    let report = scan_project(repo_path)?;
    if report.detected.is_empty() {
        return Ok(None);
    }
    if apply_bootstrap(repo_path, &report, ApplyMode::Safe)? {
        Ok(Some(report))
    } else {
        // Configs already present — surface the report so the UI can offer a Merge.
        Ok(Some(report))
    }
}

/// Glob-friendly absolute path helper for the CLI.
pub fn report_paths(report: &BootstrapReport) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    for d in &report.detected {
        for s in &d.sources {
            seen.insert(s.path.clone());
        }
    }
    seen.into_iter().map(PathBuf::from).collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(path: &Path, content: &str) {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    // --- Classifier ---

    #[test]
    fn classify_canonical_secrets() {
        assert_eq!(classify("STRIPE_SECRET"), Classification::Secret);
        assert_eq!(classify("GITHUB_TOKEN"), Classification::Secret);
        assert_eq!(classify("DEPLOY_PASSWORD"), Classification::Secret);
        assert_eq!(classify("MY_API_KEY"), Classification::Secret);
        assert_eq!(classify("SLACK_WEBHOOK"), Classification::Secret);
        assert_eq!(classify("PRIVATE_KEY"), Classification::Secret);
        assert_eq!(classify("APIKEY"), Classification::Secret);
    }

    #[test]
    fn classify_canonical_variables() {
        assert_eq!(classify("API_URL"), Classification::Variable);
        assert_eq!(classify("APP_NAME"), Classification::Variable);
        assert_eq!(classify("DEPLOY_DIR"), Classification::Variable);
        assert_eq!(classify("LOG_LEVEL"), Classification::Variable);
        assert_eq!(classify("AWS_REGION"), Classification::Variable);
        assert_eq!(classify("NODE_ENV"), Classification::Variable);
    }

    #[test]
    fn classify_avoids_false_positives() {
        // "MONKEY" must not match "KEY"
        assert_eq!(classify("MONKEY"), Classification::Variable);
        // "KEYBOARD_LAYOUT" must not match "KEY"
        assert_eq!(classify("KEYBOARD_LAYOUT"), Classification::Variable);
        // "PASSWORD_PATH" contains PASSWORD but PATH wins (config, not secret)
        assert_eq!(classify("PASSWORD_PATH"), Classification::Variable);
    }

    #[test]
    fn classify_unknown_defaults_to_variable() {
        assert_eq!(classify("FOOBAR"), Classification::Variable);
        assert_eq!(classify("SOMETHING_ELSE"), Classification::Variable);
    }

    // --- Parsers ---

    #[test]
    fn parse_dotenv_extracts_keys_only() {
        let content = "\
# comment\nAPI_URL=https://api.example.com\n\
SECRET=hunter2\n\
export NODE_ENV=production\n\
=invalid\n\
\n";
        let keys = parse_dotenv_keys(content);
        assert_eq!(keys, vec!["API_URL", "SECRET", "NODE_ENV"]);
    }

    #[test]
    fn parse_compose_finds_interpolations() {
        let content = "services:\n  app:\n    image: ${IMAGE_TAG}\n    environment:\n      - DB_HOST=${DB_HOST:-localhost}\n      - PORT=$PORT\n";
        let vars = parse_compose_vars(content);
        assert!(vars.contains(&"IMAGE_TAG".to_string()));
        assert!(vars.contains(&"DB_HOST".to_string()));
        assert!(vars.contains(&"PORT".to_string()));
    }

    #[test]
    fn parse_gha_finds_secret_refs() {
        let content = r#"
jobs:
  deploy:
    steps:
      - run: deploy
        env:
          TOKEN: ${{ secrets.DEPLOY_TOKEN }}
          KEY: ${{ secrets.SIGNING_KEY }}
"#;
        let names = parse_gha_secrets(content);
        assert!(names.contains(&"DEPLOY_TOKEN".to_string()));
        assert!(names.contains(&"SIGNING_KEY".to_string()));
    }

    #[test]
    fn parse_js_dotted_and_bracketed() {
        let content = r#"const a = process.env.API_URL;
const b = process.env["DATABASE_URL"];
const c = process.env['STRIPE_KEY'];"#;
        let names = parse_js_env(content);
        assert!(names.contains(&"API_URL".to_string()));
        assert!(names.contains(&"DATABASE_URL".to_string()));
        assert!(names.contains(&"STRIPE_KEY".to_string()));
    }

    #[test]
    fn parse_py_various_idioms() {
        let content = r#"import os
url = os.getenv("API_URL")
key = os.environ["SECRET_KEY"]
debug = os.environ.get("DEBUG", "false")"#;
        let names = parse_py_env(content);
        assert!(names.contains(&"API_URL".to_string()));
        assert!(names.contains(&"SECRET_KEY".to_string()));
        assert!(names.contains(&"DEBUG".to_string()));
    }

    #[test]
    fn parse_rs_env_var() {
        let content = r#"
let url = std::env::var("API_URL").unwrap();
let token = env::var("AUTH_TOKEN").ok();
let _ = env::var_os("XDG_CONFIG_HOME");
"#;
        let names = parse_rs_env(content);
        assert!(names.contains(&"API_URL".to_string()));
        assert!(names.contains(&"AUTH_TOKEN".to_string()));
        assert!(names.contains(&"XDG_CONFIG_HOME".to_string()));
    }

    // --- Env-name inference ---

    #[test]
    fn dotenv_filename_maps_to_env() {
        assert_eq!(
            env_name_from_dotenv_filename(".env.production").as_deref(),
            Some("production")
        );
        assert_eq!(
            env_name_from_dotenv_filename(".env.local").as_deref(),
            Some("local")
        );
        assert_eq!(env_name_from_dotenv_filename(".env"), None);
    }

    #[test]
    fn compose_filename_maps_prod_to_production() {
        assert_eq!(
            env_name_from_compose_filename("docker-compose.prod.yml").as_deref(),
            Some("production")
        );
        assert_eq!(
            env_name_from_compose_filename("docker-compose.staging.yaml").as_deref(),
            Some("staging")
        );
        assert_eq!(env_name_from_compose_filename("docker-compose.yml"), None);
    }

    // --- End-to-end scan + apply ---

    #[test]
    fn scan_project_finds_mixed_sources() {
        let temp = TempDir::new().unwrap();
        write(
            &temp.path().join(".env.production"),
            "API_URL=https://x\nSTRIPE_SECRET=abc\n",
        );
        write(
            &temp.path().join("docker-compose.prod.yml"),
            "services:\n  app:\n    image: ${IMAGE_TAG}\n",
        );
        write(
            &temp.path().join(".github/workflows/deploy.yml"),
            "jobs:\n  d:\n    run: ${{ secrets.DEPLOY_TOKEN }}\n",
        );
        write(
            &temp.path().join("src/index.ts"),
            "const k = process.env.API_KEY;\n",
        );
        // Should be skipped:
        write(
            &temp.path().join("node_modules/foo/index.js"),
            "process.env.SHOULD_NOT_APPEAR;\n",
        );

        let report = scan_project(temp.path()).unwrap();
        let names: Vec<&str> = report.detected.iter().map(|d| d.name.as_str()).collect();

        assert!(names.contains(&"API_URL"));
        assert!(names.contains(&"STRIPE_SECRET"));
        assert!(names.contains(&"IMAGE_TAG"));
        assert!(names.contains(&"DEPLOY_TOKEN"));
        assert!(names.contains(&"API_KEY"));
        assert!(!names.contains(&"SHOULD_NOT_APPEAR"));

        assert!(report
            .suggested_environments
            .iter()
            .any(|e| e == "production"));
    }

    #[test]
    fn apply_safe_refuses_when_configs_exist() {
        let temp = TempDir::new().unwrap();
        write(&temp.path().join(".chibby/environments.toml"), "");
        write(&temp.path().join(".env.production"), "API_URL=x\n");
        let report = scan_project(temp.path()).unwrap();
        let wrote = apply_bootstrap(temp.path(), &report, ApplyMode::Safe).unwrap();
        assert!(!wrote);
    }

    #[test]
    fn apply_merge_appends_only_new_names() {
        let temp = TempDir::new().unwrap();
        // Pre-existing env with one variable + one secret
        pipeline::save_environments(
            temp.path(),
            &EnvironmentsConfig {
                environments: vec![Environment {
                    name: "production".to_string(),
                    ssh_host: None,
                    ssh_port: None,
                    variables: [("EXISTING_VAR".to_string(), "value".to_string())]
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
                    name: "EXISTING_SECRET".to_string(),
                    environments: vec!["production".to_string()],
                }],
            },
        )
        .unwrap();

        write(
            &temp.path().join(".env.production"),
            "NEW_VAR=x\nNEW_TOKEN=y\n",
        );
        let report = scan_project(temp.path()).unwrap();
        let wrote = apply_bootstrap(temp.path(), &report, ApplyMode::Merge).unwrap();
        assert!(wrote);

        let envs = pipeline::load_environments(temp.path()).unwrap();
        let prod = envs.environments.iter().find(|e| e.name == "production").unwrap();
        // EXISTING_VAR preserved, NEW_VAR added with empty default
        assert_eq!(prod.variables.get("EXISTING_VAR").unwrap(), "value");
        assert!(prod.variables.contains_key("NEW_VAR"));

        let secrets = pipeline::load_secrets_config(temp.path()).unwrap();
        let names: Vec<&str> = secrets.secrets.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"EXISTING_SECRET"));
        assert!(names.contains(&"NEW_TOKEN"));
    }
}
