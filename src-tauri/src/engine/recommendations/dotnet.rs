//! .NET / C# recommendations.

use super::exists_nested;
use crate::engine::models::{FileRecommendation, RecommendationCategory, RecommendationPriority};
use std::path::Path;

/// Add .NET/C# specific recommendations.
pub(super) fn add_dotnet_recommendations(repo_path: &Path, recs: &mut Vec<FileRecommendation>) {
    // .editorconfig for C#
    recs.push(FileRecommendation {
        file_name: ".editorconfig".to_string(),
        title: "EditorConfig with C# Rules".to_string(),
        description: "Code style and analyzer rules for C# projects.".to_string(),
        priority: RecommendationPriority::High,
        category: RecommendationCategory::CodeQuality,
        docs_url: Some("https://learn.microsoft.com/en-us/dotnet/fundamentals/code-analysis/code-style-rule-options".to_string()),
        exists: exists_nested(repo_path, &[".editorconfig"]),
        template_hint: Some("Include C# naming and formatting rules".to_string()),
    });

    // Directory.Build.props
    recs.push(FileRecommendation {
        file_name: "Directory.Build.props".to_string(),
        title: "Directory Build Props".to_string(),
        description: "Centralized MSBuild properties for all projects.".to_string(),
        priority: RecommendationPriority::Medium,
        category: RecommendationCategory::Dependencies,
        docs_url: Some(
            "https://learn.microsoft.com/en-us/visualstudio/msbuild/customize-your-build"
                .to_string(),
        ),
        exists: exists_nested(repo_path, &["Directory.Build.props"]),
        template_hint: Some("Set TreatWarningsAsErrors, nullable, etc.".to_string()),
    });

    // NuGet config
    recs.push(FileRecommendation {
        file_name: "nuget.config".to_string(),
        title: "NuGet Config".to_string(),
        description: "Configure package sources and settings.".to_string(),
        priority: RecommendationPriority::Low,
        category: RecommendationCategory::Dependencies,
        docs_url: Some(
            "https://learn.microsoft.com/en-us/nuget/reference/nuget-config-file".to_string(),
        ),
        exists: exists_nested(repo_path, &["nuget.config", "NuGet.Config"]),
        template_hint: Some("Useful for private feeds".to_string()),
    });
}
