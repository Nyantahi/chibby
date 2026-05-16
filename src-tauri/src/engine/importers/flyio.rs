//! Import secret refs from Fly.io via `flyctl secrets list --json`.
//!
//! Fly.io's secrets API is *write-only* — `flyctl secrets list` returns names
//! and digests but never values. This importer is therefore names-only by
//! design: `include_values` has no effect.

use super::{cli_present, entries_from_map, ImportContext, ImportReport, Importer};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;

pub struct FlyImporter;

#[derive(Debug, Deserialize)]
struct FlySecret {
    #[serde(rename = "Name", alias = "name")]
    name: String,
}

impl Importer for FlyImporter {
    fn name(&self) -> &'static str {
        "fly"
    }

    fn detect_cli(&self) -> Result<()> {
        if cli_present("flyctl") || cli_present("fly") {
            Ok(())
        } else {
            anyhow::bail!(
                "Fly.io CLI not found on PATH. Install from https://fly.io/docs/hands-on/install-flyctl/, then run `flyctl auth login`."
            )
        }
    }

    fn run(&self, ctx: &ImportContext) -> Result<ImportReport> {
        self.detect_cli()?;
        let bin = if cli_present("flyctl") { "flyctl" } else { "fly" };
        let stdout = super::vercel::run_cli(bin, &["secrets", "list", "--json"], &ctx.repo_path)?;
        let raw: Vec<FlySecret> =
            serde_json::from_str(&stdout).context("Failed to parse `flyctl secrets list --json`")?;

        // Fly's `secrets list` is names-only by design (digests, not values).
        // Ignore `include_values` and never emit values for this source.
        let map: BTreeMap<String, Option<String>> =
            raw.into_iter().map(|s| (s.name, None)).collect();

        Ok(ImportReport {
            source: self.name().to_string(),
            env_name: ctx.env_name.clone(),
            entries: entries_from_map(map),
        })
    }
}
