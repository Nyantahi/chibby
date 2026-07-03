//! Import env vars from Railway via the `railway` CLI.
//!
//! Strategy: `railway variables --json` returns a flat `{ NAME: "value" }` object.
//! Railway treats every var as a single bucket — Chibby's classifier still
//! splits them into the `environments.toml` vs keychain on the way in.

use super::{cli_present, entries_from_map, ImportContext, ImportReport, Importer};
use anyhow::{Context, Result};
use std::collections::BTreeMap;

pub struct RailwayImporter;

impl Importer for RailwayImporter {
    fn name(&self) -> &'static str {
        "railway"
    }

    fn detect_cli(&self) -> Result<()> {
        if cli_present("railway") {
            Ok(())
        } else {
            anyhow::bail!(
                "Railway CLI not found on PATH. Install from https://docs.railway.app/develop/cli, then run `railway login` and `railway link`."
            )
        }
    }

    fn run(&self, ctx: &ImportContext) -> Result<ImportReport> {
        self.detect_cli()?;
        let stdout = super::vercel::run_cli("railway", &["variables", "--json"], &ctx.repo_path)?;
        let raw: serde_json::Value =
            serde_json::from_str(&stdout).context("Failed to parse `railway variables --json`")?;

        let obj = raw
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("Expected JSON object from railway variables"))?;

        let mut map: BTreeMap<String, Option<String>> = BTreeMap::new();
        for (k, v) in obj {
            let value = if ctx.include_values {
                v.as_str().map(|s| s.to_string())
            } else {
                None
            };
            map.insert(k.clone(), value);
        }

        Ok(ImportReport {
            source: self.name().to_string(),
            env_name: ctx.env_name.clone(),
            entries: entries_from_map(map),
        })
    }
}
