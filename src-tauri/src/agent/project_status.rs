//! Compact, cheap CI/CD status summary injected into the agent's context so it
//! is aware of a project's pipeline state and readiness. All reads are local
//! (filesystem + local run store + local `git`); no network/SSH.

use std::path::Path;

use crate::engine::git;
use crate::engine::models::{
    FileRecommendation, GateMode, RecommendationCategory, RecommendationPriority,
};
use crate::engine::{gates, persistence, pipeline, recommendations};

const MAX_MISSING_FILES: usize = 8;

/// Whether a recommendation category is within the agent's CI/CD remit.
/// Excludes general repo hygiene (version control, documentation).
fn is_cicd_category(cat: RecommendationCategory) -> bool {
    matches!(
        cat,
        RecommendationCategory::CiCd
            | RecommendationCategory::Security
            | RecommendationCategory::Testing
            | RecommendationCategory::CodeQuality
            | RecommendationCategory::Container
            | RecommendationCategory::Dependencies
    )
}

fn priority_label(p: RecommendationPriority) -> &'static str {
    match p {
        RecommendationPriority::Critical => "critical",
        RecommendationPriority::High => "high",
        RecommendationPriority::Medium => "medium",
        RecommendationPriority::Low => "low",
    }
}

/// Build a compact markdown CI/CD status block for a project.
pub fn build_ci_status(project_path: &str) -> String {
    let path = Path::new(project_path);
    let mut lines: Vec<String> = vec!["## Project CI/CD Status".to_string()];

    // Project type + readiness + missing files — cheap, offline.
    let recs = recommendations::analyze_repository(path);
    if !recs.project_types.is_empty() {
        lines.push(format!(
            "- Detected type(s): {}",
            recs.project_types.join(", ")
        ));
    }
    lines.push(format!("- CI/CD readiness: {}/100", recs.readiness_score));

    // Pipeline configured?
    if pipeline::has_pipeline(path) {
        let names = pipeline::list_pipelines(path);
        let names = if names.is_empty() {
            "pipeline".to_string()
        } else {
            names.join(", ")
        };
        lines.push(format!("- Pipeline: configured ({names})"));
    } else {
        lines.push("- Pipeline: none configured".to_string());
    }

    // Last run + last successful run.
    if let Ok(runs) = persistence::load_runs_for_project(project_path) {
        match runs.first() {
            Some(r) => lines.push(format!(
                "- Last run: {:?} — {} ({})",
                r.status,
                r.started_at.format("%Y-%m-%d %H:%M"),
                r.pipeline_name
            )),
            None => lines.push("- Last run: none yet".to_string()),
        }
    }
    if let Ok(Some(ok)) = persistence::last_successful_run(project_path, None) {
        lines.push(format!(
            "- Last successful: {}",
            ok.started_at.format("%Y-%m-%d %H:%M")
        ));
    }

    // Enabled security/quality gates.
    if let Ok(cfg) = gates::load_gates_config(path) {
        let enabled: Vec<&str> = [
            ("secret_scanning", cfg.secret_scanning),
            ("dependency_scanning", cfg.dependency_scanning),
            ("commit_lint", cfg.commit_lint),
            ("sast", cfg.sast),
            ("container_scan", cfg.container_scan),
            ("iac_scan", cfg.iac_scan),
            ("license_check", cfg.license_check),
        ]
        .into_iter()
        .filter(|(_, m)| !matches!(m, GateMode::Off))
        .map(|(n, _)| n)
        .collect();
        lines.push(if enabled.is_empty() {
            "- Security gates: none enabled".to_string()
        } else {
            format!("- Security gates: {}", enabled.join(", "))
        });
    }

    // Git branch + working-tree state.
    if git::is_git_repo(path) {
        let branch = git::current_branch(path).unwrap_or_else(|_| "unknown".to_string());
        let state = if git::is_working_tree_clean(path) {
            "clean"
        } else {
            "uncommitted changes"
        };
        lines.push(format!("- Git: branch {branch} ({state})"));
    }

    // Missing CI/CD files (agent's remit only, most important first).
    let mut missing: Vec<&FileRecommendation> = recs
        .recommendations
        .iter()
        .filter(|r| !r.exists && is_cicd_category(r.category))
        .collect();
    missing.sort_by_key(|r| r.priority); // Critical < High < Medium < Low (derived Ord)

    if missing.is_empty() {
        lines.push("- Missing CI/CD files: none".to_string());
    } else {
        lines.push("- Missing CI/CD files (create these to improve readiness):".to_string());
        for r in missing.iter().take(MAX_MISSING_FILES) {
            lines.push(format!(
                "  - [{}] {} — {}",
                priority_label(r.priority),
                r.file_name,
                r.description
            ));
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cicd_categories_included_hygiene_excluded() {
        assert!(is_cicd_category(RecommendationCategory::CiCd));
        assert!(is_cicd_category(RecommendationCategory::Security));
        assert!(is_cicd_category(RecommendationCategory::Testing));
        assert!(!is_cicd_category(RecommendationCategory::VersionControl));
        assert!(!is_cicd_category(RecommendationCategory::Documentation));
    }

    #[test]
    fn status_has_header_and_readiness() {
        // A path with no project still yields a well-formed summary.
        let out = build_ci_status(".");
        assert!(out.starts_with("## Project CI/CD Status"));
        assert!(out.contains("CI/CD readiness:"));
    }
}
