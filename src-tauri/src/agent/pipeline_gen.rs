use super::PipelineFormat;

/// Return the default file path for a given pipeline format.
pub fn default_file_path(format: &PipelineFormat) -> &'static str {
    match format {
        PipelineFormat::Chibby => ".chibby/pipeline.toml",
        PipelineFormat::GithubActions => ".github/workflows/ci.yml",
        PipelineFormat::CircleCi => ".circleci/config.yml",
        PipelineFormat::Drone => ".drone.yml",
    }
}

/// Build a description of the project for the LLM to use when generating a pipeline.
pub fn describe_project(
    project_path: &str,
    project_types: &[String],
    detected_scripts: &[String],
) -> String {
    let mut parts = Vec::new();

    parts.push(format!("Project path: {}", project_path));

    if !project_types.is_empty() {
        parts.push(format!("Detected project types: {}", project_types.join(", ")));
    }

    if !detected_scripts.is_empty() {
        parts.push("Detected scripts/commands:".to_string());
        for script in detected_scripts {
            parts.push(format!("  - {}", script));
        }
    }

    parts.join("\n")
}

/// Validate that a generated pipeline config looks reasonable for the format.
pub fn validate_generated_content(content: &str, format: &PipelineFormat) -> Result<(), String> {
    if content.trim().is_empty() {
        return Err("Generated pipeline content is empty".to_string());
    }

    match format {
        PipelineFormat::Chibby => {
            // Should contain TOML-like structure
            if !content.contains("[[stages]]") && !content.contains("[stages]") && !content.contains("name") {
                return Err("Generated Chibby pipeline doesn't look like valid TOML".to_string());
            }
        }
        PipelineFormat::GithubActions => {
            if !content.contains("on:") && !content.contains("jobs:") {
                return Err(
                    "Generated GitHub Actions config missing 'on:' or 'jobs:'".to_string(),
                );
            }
        }
        PipelineFormat::CircleCi => {
            if !content.contains("version:") && !content.contains("jobs:") {
                return Err(
                    "Generated CircleCI config missing 'version:' or 'jobs:'".to_string(),
                );
            }
        }
        PipelineFormat::Drone => {
            if !content.contains("kind:") && !content.contains("steps:") {
                return Err("Generated Drone config missing 'kind:' or 'steps:'".to_string());
            }
        }
    }

    Ok(())
}
