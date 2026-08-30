//! AI-backed pipeline generation for the CLI `--ai` flags.
//!
//! Summarizes a project directory and asks the configured LLM provider (via the
//! shared `chibby_lib::agent`) to generate a Chibby pipeline, then writes it to
//! `.chibby/pipeline.toml`.

use anyhow::{Context, Result};
use chibby_lib::agent::{self, PipelineFormat};
use std::path::Path;

/// Manifest files that hint at a project's stack.
const MANIFESTS: &[&str] = &[
    "package.json",
    "Cargo.toml",
    "pyproject.toml",
    "requirements.txt",
    "go.mod",
    "pom.xml",
    "build.gradle",
    "Gemfile",
    "composer.json",
    "Dockerfile",
    "docker-compose.yml",
    "tauri.conf.json",
];

/// Build a concise, text summary of the project for the pipeline generator.
pub fn project_info(path: &Path) -> String {
    let mut lines: Vec<String> = Vec::new();

    let present: Vec<&str> = MANIFESTS
        .iter()
        .copied()
        .filter(|m| path.join(m).exists())
        .collect();
    if !present.is_empty() {
        lines.push(format!("Manifest files: {}", present.join(", ")));
    }

    // package.json scripts are the strongest signal for JS/TS projects.
    if let Ok(content) = std::fs::read_to_string(path.join("package.json")) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(scripts) = json.get("scripts").and_then(|s| s.as_object()) {
                let names: Vec<String> = scripts
                    .iter()
                    .map(|(k, v)| format!("  {} = {}", k, v.as_str().unwrap_or("")))
                    .collect();
                if !names.is_empty() {
                    lines.push(format!("package.json scripts:\n{}", names.join("\n")));
                }
            }
        }
    }

    // Top-level entries (skip dotfiles) give the model a sense of layout.
    if let Ok(rd) = std::fs::read_dir(path) {
        let mut names: Vec<String> = rd
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| !n.starts_with('.'))
            .collect();
        names.sort();
        names.truncate(40);
        if !names.is_empty() {
            lines.push(format!("Top-level entries: {}", names.join(", ")));
        }
    }

    if lines.is_empty() {
        "No recognizable project files detected.".to_string()
    } else {
        lines.join("\n")
    }
}

/// Generate a Chibby pipeline via the AI agent and write it to
/// `<path>/.chibby/pipeline.toml`. Returns the model's explanation of the stages.
pub async fn generate_pipeline_toml(path: &Path) -> Result<String> {
    let agent = agent::build_agent()
        .context("AI is not configured. Add an API key and select a provider in the desktop app's Settings.")?;

    let info = project_info(path);
    let generated = agent
        .generate_pipeline(
            &path.to_string_lossy(),
            PipelineFormat::Chibby,
            &info,
        )
        .await?;

    let dir = path.join(".chibby");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create {}", dir.display()))?;
    let file = dir.join("pipeline.toml");
    std::fs::write(&file, &generated.content)
        .with_context(|| format!("Failed to write {}", file.display()))?;

    Ok(generated.explanation)
}
