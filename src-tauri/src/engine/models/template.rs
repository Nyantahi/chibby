//! Pipeline template types.

#[allow(unused_imports)]
use super::*;
use serde::{Deserialize, Serialize};

/// Where a template was loaded from (highest priority first).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TemplateSource {
    /// Project-local `.chibby/templates/` (highest priority).
    Project,
    /// User-global `~/.chibby/templates/`.
    User,
    /// Bundled with the application.
    BuiltIn,
}

/// Whether the template is a full pipeline or a single stage snippet.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TemplateType {
    Pipeline,
    Stage,
}

/// Metadata block for a pipeline template file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateMeta {
    /// Human-readable template name (also used as the lookup key).
    pub name: String,
    /// Short description shown in the template browser.
    pub description: String,
    /// Original author (e.g. "chibby" for built-ins).
    #[serde(default)]
    pub author: String,
    /// Semantic version of the template itself.
    #[serde(default = "default_template_version")]
    pub version: String,
    /// Primary language/domain category (e.g. "python", "deployment").
    #[serde(default)]
    pub category: String,
    /// Free-form tags for filtering/search.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Project types this template is designed for (matches detector output).
    #[serde(default)]
    pub project_types: Vec<String>,
    /// CLI tools the template expects to be available.
    #[serde(default)]
    pub required_tools: Vec<String>,
    /// `pipeline` for full pipelines, `stage` for individual snippets.
    pub template_type: TemplateType,
}

fn default_template_version() -> String {
    "1.0.0".to_string()
}

/// A `{{variable}}` placeholder found in a template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateVariable {
    /// Variable name (without braces).
    pub name: String,
    /// Human-readable label for the input field.
    #[serde(default)]
    pub description: String,
    /// Pre-filled value when the template is applied.
    #[serde(default)]
    pub default_value: String,
    /// Whether the user must supply a value.
    #[serde(default = "default_true")]
    pub required: bool,
}

/// On-disk representation of a template TOML file.
///
/// For `template_type = "pipeline"` the `pipeline` field is populated.
/// For `template_type = "stage"` the `stages` field is populated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateFile {
    pub meta: TemplateMeta,
    /// Full pipeline (only present when `template_type == pipeline`).
    #[serde(default)]
    pub pipeline: Option<Pipeline>,
    /// Individual stage snippets (only present when `template_type == stage`).
    #[serde(default)]
    pub stages: Option<Vec<Stage>>,
}

/// A template with its resolved source attached (returned to the frontend).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineTemplate {
    pub meta: TemplateMeta,
    /// Where this template was loaded from.
    pub source: TemplateSource,
    /// Full pipeline (pipeline templates only).
    #[serde(default)]
    pub pipeline: Option<Pipeline>,
    /// Stage snippets (stage templates only).
    #[serde(default)]
    pub stages: Option<Vec<Stage>>,
}

// ---------------------------------------------------------------------------
// Environment definitions (stored as .chibby/environments.toml)
// ---------------------------------------------------------------------------
