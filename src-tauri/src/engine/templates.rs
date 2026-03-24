//! Pipeline template loading, resolution, and variable substitution.
//!
//! Templates live in three layers (highest priority first):
//! 1. Per-repo:  `.chibby/templates/`
//! 2. User-global: `~/.chibby/templates/`
//! 3. Built-in: bundled with the application binary

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;

use super::models::{
    Pipeline, PipelineTemplate, Stage, TemplateFile, TemplateSource, TemplateType,
    TemplateVariable,
};

// ---------------------------------------------------------------------------
// Built-in templates (embedded at compile time)
// ---------------------------------------------------------------------------

/// Pairs of (filename, TOML content) for every built-in template.
/// The `include_str!` calls are generated in `builtin_template_entries()`.
fn builtin_template_entries() -> Vec<(&'static str, &'static str)> {
    vec![
        // Full pipeline templates
        (
            "rust-cli.toml",
            include_str!("../../templates/pipelines/rust-cli.toml"),
        ),
        (
            "rust-library.toml",
            include_str!("../../templates/pipelines/rust-library.toml"),
        ),
        (
            "node-webapp.toml",
            include_str!("../../templates/pipelines/node-webapp.toml"),
        ),
        (
            "python-django.toml",
            include_str!("../../templates/pipelines/python-django.toml"),
        ),
        (
            "python-fastapi.toml",
            include_str!("../../templates/pipelines/python-fastapi.toml"),
        ),
        (
            "go-web-service.toml",
            include_str!("../../templates/pipelines/go-web-service.toml"),
        ),
        (
            "static-site.toml",
            include_str!("../../templates/pipelines/static-site.toml"),
        ),
        (
            "tauri-desktop.toml",
            include_str!("../../templates/pipelines/tauri-desktop.toml"),
        ),
        (
            "docker-compose-deploy.toml",
            include_str!("../../templates/pipelines/docker-compose-deploy.toml"),
        ),
        // Stage snippet templates
        (
            "github-release.toml",
            include_str!("../../templates/stages/github-release.toml"),
        ),
        (
            "docker-build-push.toml",
            include_str!("../../templates/stages/docker-build-push.toml"),
        ),
        (
            "docker-compose-ssh.toml",
            include_str!("../../templates/stages/docker-compose-ssh.toml"),
        ),
        (
            "ssh-rsync-deploy.toml",
            include_str!("../../templates/stages/ssh-rsync-deploy.toml"),
        ),
        (
            "cargo-publish.toml",
            include_str!("../../templates/stages/cargo-publish.toml"),
        ),
        (
            "npm-publish.toml",
            include_str!("../../templates/stages/npm-publish.toml"),
        ),
        (
            "s3-deploy.toml",
            include_str!("../../templates/stages/s3-deploy.toml"),
        ),
        (
            "tauri-bundle.toml",
            include_str!("../../templates/stages/tauri-bundle.toml"),
        ),
        (
            "version-bump-tag.toml",
            include_str!("../../templates/stages/version-bump-tag.toml"),
        ),
        (
            "homebrew-formula.toml",
            include_str!("../../templates/stages/homebrew-formula.toml"),
        ),
    ]
}

// ---------------------------------------------------------------------------
// Directory helpers
// ---------------------------------------------------------------------------

/// `~/.chibby/templates/`
fn user_templates_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".chibby").join("templates"))
}

/// `<repo>/.chibby/templates/`
fn repo_templates_dir(repo_path: &Path) -> PathBuf {
    repo_path.join(".chibby").join("templates")
}

// ---------------------------------------------------------------------------
// Loading helpers
// ---------------------------------------------------------------------------

/// Parse a single TOML string into a `PipelineTemplate` with the given source.
fn parse_template(toml_content: &str, source: TemplateSource) -> Result<PipelineTemplate, String> {
    let file: TemplateFile =
        toml::from_str(toml_content).map_err(|e| format!("Invalid template TOML: {e}"))?;
    Ok(PipelineTemplate {
        meta: file.meta,
        source,
        pipeline: file.pipeline,
        stages: file.stages,
    })
}

/// Read all `*.toml` files from a directory and parse them as templates.
fn load_templates_from_dir(dir: &Path, source: TemplateSource) -> Vec<PipelineTemplate> {
    let mut templates = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return templates,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map_or(true, |ext| ext != "toml") {
            // Recurse one level into subdirectories (e.g. pipelines/, stages/)
            if path.is_dir() {
                templates.extend(load_templates_from_dir(&path, source.clone()));
            }
            continue;
        }
        if let Ok(content) = fs::read_to_string(&path) {
            match parse_template(&content, source.clone()) {
                Ok(t) => templates.push(t),
                Err(e) => {
                    log::warn!("Skipping template {}: {}", path.display(), e);
                }
            }
        }
    }
    templates
}

// ---------------------------------------------------------------------------
// Public API — loading
// ---------------------------------------------------------------------------

/// Load all built-in templates embedded in the binary.
pub fn load_builtin_templates() -> Vec<PipelineTemplate> {
    let mut templates = Vec::new();
    for (name, content) in builtin_template_entries() {
        match parse_template(content, TemplateSource::BuiltIn) {
            Ok(t) => templates.push(t),
            Err(e) => log::warn!("Bad built-in template {name}: {e}"),
        }
    }
    templates
}

/// Load templates from the user-global directory (`~/.chibby/templates/`).
pub fn load_user_templates() -> Vec<PipelineTemplate> {
    match user_templates_dir() {
        Some(dir) => load_templates_from_dir(&dir, TemplateSource::User),
        None => Vec::new(),
    }
}

/// Load templates from a project's `.chibby/templates/` directory.
pub fn load_repo_templates(repo_path: &Path) -> Vec<PipelineTemplate> {
    load_templates_from_dir(&repo_templates_dir(repo_path), TemplateSource::Project)
}

/// Merge all three layers, de-duplicating by `meta.name` (highest-priority wins).
pub fn get_all_templates(repo_path: Option<&Path>) -> Vec<PipelineTemplate> {
    let mut by_name: HashMap<String, PipelineTemplate> = HashMap::new();

    // Insert in priority order: lowest first, so higher-priority overwrites.
    for t in load_builtin_templates() {
        by_name.insert(t.meta.name.clone(), t);
    }
    for t in load_user_templates() {
        by_name.insert(t.meta.name.clone(), t);
    }
    if let Some(rp) = repo_path {
        for t in load_repo_templates(rp) {
            by_name.insert(t.meta.name.clone(), t);
        }
    }

    let mut all: Vec<PipelineTemplate> = by_name.into_values().collect();
    all.sort_by(|a, b| a.meta.name.cmp(&b.meta.name));
    all
}

/// Look up a single template by name, respecting the priority order.
pub fn get_template_by_name(
    name: &str,
    repo_path: Option<&Path>,
) -> Option<PipelineTemplate> {
    get_all_templates(repo_path)
        .into_iter()
        .find(|t| t.meta.name == name)
}

// ---------------------------------------------------------------------------
// Public API — variable extraction & substitution
// ---------------------------------------------------------------------------

/// Well-known template variables with descriptions and defaults.
fn well_known_variable(name: &str) -> Option<(String, String, bool)> {
    // Returns (description, default_value, required)
    match name {
        "bump_level" => Some((
            "Version bump level: patch, minor, or major".into(),
            "patch".into(),
            true,
        )),
        "project_name" => Some((
            "Name of the project".into(),
            String::new(),
            true,
        )),
        _ => None,
    }
}

/// Extract every `{{variable_name}}` placeholder from a template.
pub fn extract_template_variables(template: &PipelineTemplate) -> Vec<TemplateVariable> {
    let re = Regex::new(r"\{\{(\w+)\}\}").expect("valid regex");
    let mut seen = HashMap::new();

    let mut scan = |text: &str| {
        for cap in re.captures_iter(text) {
            let var = cap[1].to_string();
            seen.entry(var.clone()).or_insert_with(|| {
                if let Some((desc, default, required)) = well_known_variable(&var) {
                    TemplateVariable {
                        name: var,
                        description: desc,
                        default_value: default,
                        required,
                    }
                } else {
                    TemplateVariable {
                        name: var,
                        description: String::new(),
                        default_value: String::new(),
                        required: true,
                    }
                }
            });
        }
    };

    // Scan pipeline name
    if let Some(ref p) = template.pipeline {
        scan(&p.name);
        for stage in &p.stages {
            scan_stage(&mut scan, stage);
        }
    }
    // Scan stage snippets
    if let Some(ref stages) = template.stages {
        for stage in stages {
            scan_stage(&mut scan, stage);
        }
    }

    let mut vars: Vec<TemplateVariable> = seen.into_values().collect();
    vars.sort_by(|a, b| a.name.cmp(&b.name));
    vars
}

fn scan_stage(scan: &mut impl FnMut(&str), stage: &Stage) {
    scan(&stage.name);
    for cmd in &stage.commands {
        scan(cmd);
    }
    if let Some(ref wd) = stage.working_dir {
        scan(wd);
    }
    if let Some(ref hc) = stage.health_check {
        scan(&hc.command);
    }
}

/// Replace all `{{variable_name}}` placeholders in the template's pipeline/stages
/// and return a concrete `Pipeline`.
///
/// For stage-type templates the returned pipeline has a placeholder name and
/// contains only the snippet stages.
pub fn apply_template_variables(
    template: &PipelineTemplate,
    variables: &HashMap<String, String>,
) -> Result<Pipeline, String> {
    let subst = |text: &str| -> String {
        let mut out = text.to_string();
        for (key, val) in variables {
            out = out.replace(&format!("{{{{{key}}}}}"), val);
        }
        out
    };

    let subst_stage = |s: &Stage| -> Stage {
        Stage {
            name: subst(&s.name),
            commands: s.commands.iter().map(|c| subst(c)).collect(),
            backend: s.backend.clone(),
            working_dir: s.working_dir.as_ref().map(|w| subst(w)),
            fail_fast: s.fail_fast,
            health_check: s.health_check.as_ref().map(|hc| {
                let mut hc = hc.clone();
                hc.command = subst(&hc.command);
                hc
            }),
        }
    };

    match template.meta.template_type {
        TemplateType::Pipeline => {
            let p = template
                .pipeline
                .as_ref()
                .ok_or_else(|| "Template marked as pipeline but has no pipeline field".to_string())?;
            Ok(Pipeline {
                name: subst(&p.name),
                stages: p.stages.iter().map(subst_stage).collect(),
            })
        }
        TemplateType::Stage => {
            let stages = template
                .stages
                .as_ref()
                .ok_or_else(|| "Template marked as stage but has no stages field".to_string())?;
            Ok(Pipeline {
                name: "Applied stage snippet".to_string(),
                stages: stages.iter().map(subst_stage).collect(),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Public API — saving & deleting custom templates
// ---------------------------------------------------------------------------

/// Serialize a `PipelineTemplate` back to TOML (the on-disk `TemplateFile` format).
fn template_to_toml(template: &PipelineTemplate) -> Result<String, String> {
    let file = TemplateFile {
        meta: template.meta.clone(),
        pipeline: template.pipeline.clone(),
        stages: template.stages.clone(),
    };
    toml::to_string_pretty(&file).map_err(|e| format!("Failed to serialize template: {e}"))
}

/// Derive a filesystem-safe slug from a template name.
fn slug(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// Save a template to the user-global directory.
pub fn save_user_template(template: &PipelineTemplate) -> Result<(), String> {
    let dir = user_templates_dir().ok_or("Cannot determine home directory")?;
    save_template_to_dir(&dir, template)
}

/// Save a template to a project's `.chibby/templates/` directory.
pub fn save_repo_template(repo_path: &Path, template: &PipelineTemplate) -> Result<(), String> {
    save_template_to_dir(&repo_templates_dir(repo_path), template)
}

fn save_template_to_dir(dir: &Path, template: &PipelineTemplate) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("Cannot create templates directory: {e}"))?;
    let filename = format!("{}.toml", slug(&template.meta.name));
    let path = dir.join(&filename);
    let content = template_to_toml(template)?;
    fs::write(&path, content).map_err(|e| format!("Failed to write template: {e}"))
}

/// Delete a user-global template by name.
pub fn delete_user_template(name: &str) -> Result<(), String> {
    let dir = user_templates_dir().ok_or("Cannot determine home directory")?;
    delete_template_from_dir(&dir, name)
}

/// Delete a repo-local template by name.
pub fn delete_repo_template(repo_path: &Path, name: &str) -> Result<(), String> {
    delete_template_from_dir(&repo_templates_dir(repo_path), name)
}

fn delete_template_from_dir(dir: &Path, name: &str) -> Result<(), String> {
    let filename = format!("{}.toml", slug(name));
    let path = dir.join(&filename);
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("Failed to delete template: {e}"))
    } else {
        Err(format!("Template '{}' not found in {}", name, dir.display()))
    }
}

// ---------------------------------------------------------------------------
// Import / export
// ---------------------------------------------------------------------------

/// Export a template as a TOML string (for sharing).
pub fn export_template(name: &str, repo_path: Option<&Path>) -> Result<String, String> {
    let template = get_template_by_name(name, repo_path)
        .ok_or_else(|| format!("Template '{name}' not found"))?;
    template_to_toml(&template)
}

/// Import a template from a TOML string.
pub fn import_template(toml_content: &str, source: TemplateSource) -> Result<PipelineTemplate, String> {
    parse_template(toml_content, source)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_PIPELINE_TEMPLATE: &str = r#"
[meta]
name = "Test Pipeline"
description = "A test template"
author = "test"
version = "1.0.0"
category = "testing"
tags = ["test"]
project_types = ["rust"]
required_tools = ["cargo"]
template_type = "pipeline"

[pipeline]
name = "{{project_name}} Pipeline"

[[pipeline.stages]]
name = "build"
commands = ["cargo build --release"]
backend = "local"
fail_fast = true

[[pipeline.stages]]
name = "test"
commands = ["cargo test"]
backend = "local"
fail_fast = true
"#;

    const SAMPLE_STAGE_TEMPLATE: &str = r#"
[meta]
name = "Test Stage"
description = "A deploy stage"
author = "test"
version = "1.0.0"
category = "deployment"
tags = ["deploy"]
template_type = "stage"

[[stages]]
name = "deploy-{{env}}"
commands = ["rsync -avz ./dist/ {{ssh_host}}:{{deploy_path}}/"]
backend = "local"
fail_fast = true
"#;

    #[test]
    fn test_parse_pipeline_template() {
        let t = parse_template(SAMPLE_PIPELINE_TEMPLATE, TemplateSource::BuiltIn).unwrap();
        assert_eq!(t.meta.name, "Test Pipeline");
        assert_eq!(t.meta.template_type, TemplateType::Pipeline);
        assert!(t.pipeline.is_some());
        assert_eq!(t.pipeline.as_ref().unwrap().stages.len(), 2);
        assert_eq!(t.source, TemplateSource::BuiltIn);
    }

    #[test]
    fn test_parse_stage_template() {
        let t = parse_template(SAMPLE_STAGE_TEMPLATE, TemplateSource::User).unwrap();
        assert_eq!(t.meta.name, "Test Stage");
        assert_eq!(t.meta.template_type, TemplateType::Stage);
        assert!(t.stages.is_some());
        assert_eq!(t.stages.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_extract_variables_pipeline() {
        let t = parse_template(SAMPLE_PIPELINE_TEMPLATE, TemplateSource::BuiltIn).unwrap();
        let vars = extract_template_variables(&t);
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].name, "project_name");
    }

    #[test]
    fn test_extract_variables_stage() {
        let t = parse_template(SAMPLE_STAGE_TEMPLATE, TemplateSource::User).unwrap();
        let vars = extract_template_variables(&t);
        assert_eq!(vars.len(), 3);
        let names: Vec<&str> = vars.iter().map(|v| v.name.as_str()).collect();
        assert!(names.contains(&"env"));
        assert!(names.contains(&"ssh_host"));
        assert!(names.contains(&"deploy_path"));
    }

    #[test]
    fn test_apply_variables_pipeline() {
        let t = parse_template(SAMPLE_PIPELINE_TEMPLATE, TemplateSource::BuiltIn).unwrap();
        let mut vars = HashMap::new();
        vars.insert("project_name".to_string(), "MyApp".to_string());
        let pipeline = apply_template_variables(&t, &vars).unwrap();
        assert_eq!(pipeline.name, "MyApp Pipeline");
        assert_eq!(pipeline.stages.len(), 2);
    }

    #[test]
    fn test_apply_variables_stage() {
        let t = parse_template(SAMPLE_STAGE_TEMPLATE, TemplateSource::User).unwrap();
        let mut vars = HashMap::new();
        vars.insert("env".to_string(), "production".to_string());
        vars.insert("ssh_host".to_string(), "deploy@server".to_string());
        vars.insert("deploy_path".to_string(), "/var/www/app".to_string());
        let pipeline = apply_template_variables(&t, &vars).unwrap();
        assert_eq!(pipeline.stages[0].name, "deploy-production");
        assert_eq!(
            pipeline.stages[0].commands[0],
            "rsync -avz ./dist/ deploy@server:/var/www/app/"
        );
    }

    #[test]
    fn test_slug() {
        assert_eq!(slug("Docker Build & Push"), "docker-build---push");
        assert_eq!(slug("Rust CLI"), "rust-cli");
        assert_eq!(slug("S3 / Static Site Deploy"), "s3---static-site-deploy");
    }

    #[test]
    fn test_template_to_toml_roundtrip() {
        let t = parse_template(SAMPLE_PIPELINE_TEMPLATE, TemplateSource::BuiltIn).unwrap();
        let toml_str = template_to_toml(&t).unwrap();
        let t2 = parse_template(&toml_str, TemplateSource::User).unwrap();
        assert_eq!(t.meta.name, t2.meta.name);
        assert_eq!(
            t.pipeline.as_ref().unwrap().stages.len(),
            t2.pipeline.as_ref().unwrap().stages.len()
        );
    }

    #[test]
    fn test_load_builtin_templates() {
        let templates = load_builtin_templates();
        assert!(
            !templates.is_empty(),
            "Should have at least one built-in template"
        );
        for t in &templates {
            assert_eq!(t.source, TemplateSource::BuiltIn);
            assert!(!t.meta.name.is_empty());
        }
    }
}
