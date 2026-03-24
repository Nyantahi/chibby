use std::collections::HashMap;
use std::path::Path;

use crate::engine::models::{Pipeline, PipelineTemplate, TemplateSource, TemplateVariable};
use crate::engine::templates;

// ---------------------------------------------------------------------------
// Template queries
// ---------------------------------------------------------------------------

/// Get all templates (built-in + user + project), merged and de-duplicated.
#[tauri::command]
pub fn get_templates(repo_path: Option<String>) -> Result<Vec<PipelineTemplate>, String> {
    let rp = repo_path.as_deref().map(Path::new);
    Ok(templates::get_all_templates(rp))
}

/// Get a single template by name.
#[tauri::command]
pub fn get_template(
    name: String,
    repo_path: Option<String>,
) -> Result<PipelineTemplate, String> {
    let rp = repo_path.as_deref().map(Path::new);
    templates::get_template_by_name(&name, rp)
        .ok_or_else(|| format!("Template '{}' not found", name))
}

/// Extract the `{{variable}}` placeholders from a template.
#[tauri::command]
pub fn get_template_variables(
    name: String,
    repo_path: Option<String>,
) -> Result<Vec<TemplateVariable>, String> {
    let rp = repo_path.as_deref().map(Path::new);
    let template = templates::get_template_by_name(&name, rp)
        .ok_or_else(|| format!("Template '{}' not found", name))?;
    Ok(templates::extract_template_variables(&template))
}

/// Apply a template with the given variable values, producing a concrete Pipeline.
#[tauri::command]
pub fn apply_template(
    name: String,
    repo_path: Option<String>,
    variables: HashMap<String, String>,
) -> Result<Pipeline, String> {
    let rp = repo_path.as_deref().map(Path::new);
    let template = templates::get_template_by_name(&name, rp)
        .ok_or_else(|| format!("Template '{}' not found", name))?;
    templates::apply_template_variables(&template, &variables)
}

// ---------------------------------------------------------------------------
// Template CRUD
// ---------------------------------------------------------------------------

/// Save a custom template to either user-global or project scope.
#[tauri::command]
pub fn save_custom_template(
    template: PipelineTemplate,
    scope: String,
    repo_path: Option<String>,
) -> Result<(), String> {
    match scope.as_str() {
        "user" => templates::save_user_template(&template),
        "project" => {
            let rp = repo_path
                .as_deref()
                .ok_or("repo_path required for project-scoped templates")?;
            templates::save_repo_template(Path::new(rp), &template)
        }
        _ => Err(format!("Invalid scope '{}', expected 'user' or 'project'", scope)),
    }
}

/// Delete a custom template from either user-global or project scope.
#[tauri::command]
pub fn delete_custom_template(
    name: String,
    scope: String,
    repo_path: Option<String>,
) -> Result<(), String> {
    match scope.as_str() {
        "user" => templates::delete_user_template(&name),
        "project" => {
            let rp = repo_path
                .as_deref()
                .ok_or("repo_path required for project-scoped templates")?;
            templates::delete_repo_template(Path::new(rp), &name)
        }
        _ => Err(format!("Invalid scope '{}', expected 'user' or 'project'", scope)),
    }
}

// ---------------------------------------------------------------------------
// Import / export
// ---------------------------------------------------------------------------

/// Export a template as a TOML string for sharing.
#[tauri::command]
pub fn export_template(
    name: String,
    repo_path: Option<String>,
) -> Result<String, String> {
    let rp = repo_path.as_deref().map(Path::new);
    templates::export_template(&name, rp)
}

/// Import a template from a TOML string and save it to the given scope.
#[tauri::command]
pub fn import_template(
    toml_content: String,
    scope: String,
    repo_path: Option<String>,
) -> Result<PipelineTemplate, String> {
    let source = match scope.as_str() {
        "user" => TemplateSource::User,
        "project" => TemplateSource::Project,
        _ => return Err(format!("Invalid scope '{}', expected 'user' or 'project'", scope)),
    };
    let template = templates::import_template(&toml_content, source)?;

    // Also persist the imported template
    match scope.as_str() {
        "user" => templates::save_user_template(&template)?,
        "project" => {
            let rp = repo_path
                .as_deref()
                .ok_or("repo_path required for project-scoped templates")?;
            templates::save_repo_template(Path::new(rp), &template)?;
        }
        _ => {}
    }

    Ok(template)
}
