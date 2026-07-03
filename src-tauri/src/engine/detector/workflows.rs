//! GitHub Actions workflow parsing and conversion to stages.

#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::engine::models::{
    Backend, DeploymentConfig, DeploymentMethod, Environment, EnvironmentsConfig, FileConflict,
    HealthCheck, Pipeline, PipelineValidation, PipelineWarning, Stage, WarningSeverity,
};
use std::collections::HashSet;
use std::path::Path;

/// A parsed step from a CI workflow.
#[derive(Debug, Clone)]
pub struct CiWorkflowStep {
    /// Name of the step (from CI config).
    pub name: Option<String>,
    /// The run command(s).
    pub run: String,
    /// Working directory if specified.
    pub working_directory: Option<String>,
}

/// A parsed job from a CI workflow.
#[derive(Debug, Clone)]
pub struct CiWorkflowJob {
    /// Job ID/name.
    pub name: String,
    /// Steps in this job.
    pub steps: Vec<CiWorkflowStep>,
}

/// A parsed CI workflow file.
#[derive(Debug, Clone)]
pub struct CiWorkflow {
    /// Workflow name.
    pub name: String,
    /// Source file path.
    pub file_path: String,
    /// Jobs in this workflow.
    pub jobs: Vec<CiWorkflowJob>,
}

/// Parse all GitHub Actions workflows in a repository.
pub fn parse_github_workflows(repo_path: &Path) -> Vec<CiWorkflow> {
    let workflows_dir = repo_path.join(".github/workflows");
    let mut workflows = Vec::new();

    if !workflows_dir.is_dir() {
        return workflows;
    }

    if let Ok(entries) = std::fs::read_dir(&workflows_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if name.ends_with(".yml") || name.ends_with(".yaml") {
                if let Some(workflow) = parse_single_github_workflow(&path) {
                    workflows.push(workflow);
                }
            }
        }
    }

    workflows
}

/// Parse a single GitHub Actions workflow file.
fn parse_single_github_workflow(file_path: &Path) -> Option<CiWorkflow> {
    let content = std::fs::read_to_string(file_path).ok()?;
    let yaml: serde_yaml::Value = serde_yaml::from_str(&content).ok()?;

    let workflow_name = yaml
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            file_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        });

    let mut jobs = Vec::new();

    if let Some(jobs_map) = yaml.get("jobs").and_then(|v| v.as_mapping()) {
        for (job_key, job_value) in jobs_map {
            let job_name = job_key.as_str().unwrap_or("unknown").to_string();
            let mut steps = Vec::new();

            if let Some(steps_arr) = job_value.get("steps").and_then(|v| v.as_sequence()) {
                for step in steps_arr {
                    // Only include steps with "run" commands (skip actions like checkout)
                    if let Some(run_cmd) = step.get("run").and_then(|v| v.as_str()) {
                        let step_name = step
                            .get("name")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        let working_dir = step
                            .get("working-directory")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());

                        steps.push(CiWorkflowStep {
                            name: step_name,
                            run: run_cmd.to_string(),
                            working_directory: working_dir,
                        });
                    }
                }
            }

            if !steps.is_empty() {
                jobs.push(CiWorkflowJob {
                    name: job_name,
                    steps,
                });
            }
        }
    }

    if jobs.is_empty() {
        return None;
    }

    Some(CiWorkflow {
        name: workflow_name,
        file_path: file_path.to_string_lossy().to_string(),
        jobs,
    })
}

/// Convert parsed CI workflows into pipeline stages.
///
/// This creates one stage per job, with each step's run commands combined.
pub fn workflows_to_stages(workflows: &[CiWorkflow]) -> Vec<Stage> {
    let mut stages = Vec::new();
    let mut seen_names: HashSet<String> = HashSet::new();

    for workflow in workflows {
        for job in &workflow.jobs {
            // Create a unique stage name
            let base_name = format!("ci-{}", job.name);
            let stage_name = if seen_names.contains(&base_name) {
                format!(
                    "{}-{}",
                    base_name,
                    workflow.name.to_lowercase().replace(' ', "-")
                )
            } else {
                base_name.clone()
            };
            seen_names.insert(stage_name.clone());

            // Collect all run commands from this job
            let commands: Vec<String> = job
                .steps
                .iter()
                .flat_map(|step| {
                    // Split multi-line run commands
                    step.run
                        .lines()
                        .map(|line| line.trim().to_string())
                        .filter(|line| !line.is_empty() && !line.starts_with('#'))
                        .collect::<Vec<_>>()
                })
                .collect();

            if commands.is_empty() {
                continue;
            }

            // Use working directory from first step that has one
            let working_dir = job.steps.iter().find_map(|s| s.working_directory.clone());

            stages.push(Stage {
                name: stage_name,
                commands,
                backend: Backend::Local,
                working_dir,
                fail_fast: true,
                health_check: None,
            });
        }
    }

    stages
}

// ---------------------------------------------------------------------------
// Pipeline Validation
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_github_workflows_empty() {
        let temp = TempDir::new().unwrap();
        let workflows = parse_github_workflows(temp.path());
        assert!(workflows.is_empty());
    }

    #[test]
    fn test_parse_github_workflows_with_file() {
        let temp = TempDir::new().unwrap();
        let workflows_dir = temp.path().join(".github/workflows");
        std::fs::create_dir_all(&workflows_dir).unwrap();

        std::fs::write(
            workflows_dir.join("ci.yml"),
            r#"
name: CI
on: [push]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Build
        run: npm run build
"#,
        )
        .unwrap();

        let workflows = parse_github_workflows(temp.path());
        assert!(!workflows.is_empty());
        assert_eq!(workflows[0].name, "CI");
    }
}
