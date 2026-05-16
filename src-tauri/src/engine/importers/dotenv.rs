//! Import names (and optionally values) from a `.env` file.
//!
//! `KEY=VALUE` lines, comments, and quoted values are all supported.
//! Classification reuses the bootstrap heuristic so a `.env.production`
//! with `STRIPE_SECRET=...` lands in the keychain side, while `API_URL=...`
//! lands in environments.toml.

use super::{entries_from_map, ImportContext, ImportReport, Importer};
use anyhow::{Context, Result};
use std::collections::BTreeMap;

pub struct DotEnvImporter;

impl Importer for DotEnvImporter {
    fn name(&self) -> &'static str {
        "dotenv"
    }

    fn detect_cli(&self) -> Result<()> {
        // No external tool needed.
        Ok(())
    }

    fn run(&self, ctx: &ImportContext) -> Result<ImportReport> {
        let source_path = ctx
            .source_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("dotenv importer requires a source path"))?;
        let content = std::fs::read_to_string(source_path)
            .with_context(|| format!("Failed to read {}", source_path.display()))?;

        let mut map: BTreeMap<String, Option<String>> = BTreeMap::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let line = line.strip_prefix("export ").unwrap_or(line);
            let Some(eq) = line.find('=') else { continue };
            let key = line[..eq].trim();
            if key.is_empty() {
                continue;
            }
            // Strip optional surrounding quotes from value
            let value_raw = line[eq + 1..].trim();
            let value = strip_quotes(value_raw);
            let stored = if ctx.include_values {
                Some(value)
            } else {
                None
            };
            map.insert(key.to_string(), stored);
        }

        Ok(ImportReport {
            source: self.name().to_string(),
            env_name: ctx.env_name.clone(),
            entries: entries_from_map(map),
        })
    }
}

fn strip_quotes(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"') && s.len() >= 2)
        || (s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2)
    {
        // Unescape \" inside double-quoted values; single-quoted stays literal.
        let inner = &s[1..s.len() - 1];
        if s.starts_with('"') {
            return inner.replace("\\\"", "\"");
        }
        return inner.to_string();
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::bootstrap::Classification;
    use tempfile::TempDir;

    fn write_dotenv(temp: &TempDir, content: &str) -> std::path::PathBuf {
        let path = temp.path().join(".env.production");
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn parses_simple_kv() {
        let temp = TempDir::new().unwrap();
        let path = write_dotenv(&temp, "API_URL=https://x\nSTRIPE_SECRET=sk_xxx\n");
        let ctx = ImportContext {
            repo_path: temp.path().to_path_buf(),
            env_name: "production".to_string(),
            source_path: Some(path),
            include_values: true,
        };
        let report = DotEnvImporter.run(&ctx).unwrap();
        assert_eq!(report.entries.len(), 2);
        let api = report.entries.iter().find(|e| e.name == "API_URL").unwrap();
        assert_eq!(api.classification, Classification::Variable);
        assert_eq!(api.value.as_deref(), Some("https://x"));
        let stripe = report
            .entries
            .iter()
            .find(|e| e.name == "STRIPE_SECRET")
            .unwrap();
        assert_eq!(stripe.classification, Classification::Secret);
    }

    #[test]
    fn handles_quoted_values_and_comments() {
        let temp = TempDir::new().unwrap();
        let path = write_dotenv(
            &temp,
            "# comment line\nMSG=\"hello world\"\nESCAPED=\"quote \\\" inside\"\nLITERAL='single'\n",
        );
        let ctx = ImportContext {
            repo_path: temp.path().to_path_buf(),
            env_name: "production".to_string(),
            source_path: Some(path),
            include_values: true,
        };
        let report = DotEnvImporter.run(&ctx).unwrap();
        let m = |n: &str| {
            report
                .entries
                .iter()
                .find(|e| e.name == n)
                .unwrap()
                .value
                .as_deref()
                .unwrap()
                .to_string()
        };
        assert_eq!(m("MSG"), "hello world");
        assert_eq!(m("ESCAPED"), "quote \" inside");
        assert_eq!(m("LITERAL"), "single");
    }

    #[test]
    fn names_only_mode_drops_values() {
        let temp = TempDir::new().unwrap();
        let path = write_dotenv(&temp, "API_URL=secret-value\n");
        let ctx = ImportContext {
            repo_path: temp.path().to_path_buf(),
            env_name: "production".to_string(),
            source_path: Some(path),
            include_values: false,
        };
        let report = DotEnvImporter.run(&ctx).unwrap();
        assert!(report.entries[0].value.is_none());
    }
}
