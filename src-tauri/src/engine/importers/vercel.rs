//! Import env vars from Vercel via the `vercel` CLI.
//!
//! Strategy:
//! - `vercel env ls --json` returns metadata only (no values) — sufficient for
//!   names-only imports.
//! - When `include_values=true`, run `vercel env pull` to a tempfile, then
//!   parse it as a dotenv (reuses our dotenv importer).
//!
//! The CLI must be authenticated (`vercel login`) and the project must be
//! linked (`vercel link`) before this importer can succeed.

use super::dotenv::DotEnvImporter;
use super::{cli_present, entries_from_map, ImportContext, ImportReport, Importer};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::process::Command;

pub struct VercelImporter;

#[derive(Debug, Deserialize)]
struct VercelEnv {
    key: String,
    #[allow(dead_code)]
    target: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct VercelEnvList {
    envs: Vec<VercelEnv>,
}

/// Run the vendor CLI and capture stdout. Common helper shared with other
/// PaaS adapters that follow the same pattern.
pub(super) fn run_cli(bin: &str, args: &[&str], cwd: &std::path::Path) -> Result<String> {
    let output = Command::new(bin)
        .args(args)
        .current_dir(cwd)
        .output()
        .with_context(|| format!("Failed to run `{} {}`", bin, args.join(" ")))?;
    if !output.status.success() {
        anyhow::bail!(
            "`{} {}` exited {}: {}",
            bin,
            args.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

impl Importer for VercelImporter {
    fn name(&self) -> &'static str {
        "vercel"
    }

    fn detect_cli(&self) -> Result<()> {
        if cli_present("vercel") {
            Ok(())
        } else {
            anyhow::bail!(
                "Vercel CLI not found on PATH. Install with `npm i -g vercel`, then run `vercel login` and `vercel link`."
            )
        }
    }

    fn run(&self, ctx: &ImportContext) -> Result<ImportReport> {
        self.detect_cli()?;

        // Names from `vercel env ls --json`
        let stdout = run_cli(
            "vercel",
            &["env", "ls", "--json", "--environment", &ctx.env_name],
            &ctx.repo_path,
        )?;
        let parsed: VercelEnvList = serde_json::from_str(&stdout)
            .with_context(|| "Failed to parse `vercel env ls --json` output")?;
        let mut map: BTreeMap<String, Option<String>> = parsed
            .envs
            .into_iter()
            .map(|e| (e.key, None))
            .collect();

        // Values via `vercel env pull` if requested
        if ctx.include_values {
            let tmp = tempfile::Builder::new()
                .prefix("chibby-vercel-")
                .suffix(".env")
                .tempfile()?;
            run_cli(
                "vercel",
                &[
                    "env",
                    "pull",
                    "--environment",
                    &ctx.env_name,
                    "--yes",
                    tmp.path().to_str().unwrap(),
                ],
                &ctx.repo_path,
            )?;
            let dotenv_ctx = ImportContext {
                repo_path: ctx.repo_path.clone(),
                env_name: ctx.env_name.clone(),
                source_path: Some(tmp.path().to_path_buf()),
                include_values: true,
            };
            let pulled = DotEnvImporter.run(&dotenv_ctx)?;
            for entry in pulled.entries {
                map.insert(entry.name, entry.value);
            }
        }

        Ok(ImportReport {
            source: self.name().to_string(),
            env_name: ctx.env_name.clone(),
            entries: entries_from_map(map),
        })
    }
}
