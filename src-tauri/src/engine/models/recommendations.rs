//! Project recommendation types.

#[allow(unused_imports)]
use super::*;
use serde::{Deserialize, Serialize};

/// Priority level for a CI/CD recommendation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum RecommendationPriority {
    /// Critical - Essential for basic CI/CD setup
    Critical,
    /// High - Important for code quality and automation
    High,
    /// Medium - Good practices for maintainability
    Medium,
    /// Low - Nice to have for mature projects
    Low,
}

/// Category of CI/CD recommendation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationCategory {
    /// Version control hygiene
    VersionControl,
    /// Documentation
    Documentation,
    /// CI/CD workflows
    CiCd,
    /// Code quality and linting
    CodeQuality,
    /// Testing
    Testing,
    /// Security
    Security,
    /// Containerization
    Container,
    /// Dependencies
    Dependencies,
}

/// A single CI/CD file recommendation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecommendation {
    /// File name or path to create.
    pub file_name: String,
    /// Human-readable title.
    pub title: String,
    /// Description of why this file is important.
    pub description: String,
    /// Priority level.
    pub priority: RecommendationPriority,
    /// Category of recommendation.
    pub category: RecommendationCategory,
    /// Link to documentation or template.
    pub docs_url: Option<String>,
    /// Whether this file already exists in the project.
    pub exists: bool,
    /// Suggested template content (if available).
    pub template_hint: Option<String>,
}

/// Full recommendations result for a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRecommendations {
    /// Detected project type(s).
    pub project_types: Vec<String>,
    /// List of all recommendations.
    pub recommendations: Vec<FileRecommendation>,
    /// Overall CI/CD readiness score (0-100).
    pub readiness_score: u8,
    /// Summary counts by priority.
    pub summary: RecommendationSummary,
}

/// Summary of recommendations by priority.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationSummary {
    pub critical_missing: u32,
    pub high_missing: u32,
    pub medium_missing: u32,
    pub low_missing: u32,
    pub total_recommendations: u32,
    pub total_present: u32,
}

// ---------------------------------------------------------------------------
// Phase 5.5: Tauri Updater Integration
// ---------------------------------------------------------------------------
