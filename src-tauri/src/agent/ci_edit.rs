//! Git-safe editing of CI/CD config files for the agent: validate → diff →
//! backup → write, committing to a dedicated agent branch when the tree is
//! clean. The user's current branch is never committed to.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::agent::ci_files::{self, CiFormat};
use crate::engine::{audit, git};

/// Outcome of a CI file edit, surfaced to the agent and UI.
#[derive(Debug, Clone, Serialize)]
pub struct EditResult {
    pub path: String,
    pub format: CiFormat,
    /// Unified diff of the change (may be empty if unavailable).
    pub diff: String,
    pub backup_path: Option<String>,
    pub git_committed: bool,
    pub branch: Option<String>,
    pub commit_sha: Option<String>,
    pub original_branch: Option<String>,
    /// Why a git commit was skipped, if it was.
    pub note: Option<String>,
}

/// Validate a project-relative path stays inside the project and create its
/// parent directory. Shared with `save_generated_pipeline`.
pub fn guard_project_path(project_path: &str, rel_path: &str) -> Result<PathBuf, String> {
    if rel_path.contains("..") || rel_path.starts_with('/') || rel_path.starts_with('\\') {
        return Err("Invalid file path: must be a relative path within the project".to_string());
    }

    let canonical_project =
        std::fs::canonicalize(project_path).map_err(|e| format!("Invalid project path: {e}"))?;
    let full_path = canonical_project.join(rel_path);

    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {e}"))?;
    }

    let canonical_parent =
        std::fs::canonicalize(full_path.parent().unwrap_or(&canonical_project))
            .map_err(|e| format!("Failed to resolve path: {e}"))?;

    if !canonical_parent.starts_with(&canonical_project) {
        return Err("File path resolves outside the project directory".to_string());
    }

    // Refuse a final component that is a symlink: `fs::write` follows it and
    // would clobber a file outside the project (a malicious repo can ship such
    // a symlink at an allowed CI path). The parent check above can't catch this.
    if let Ok(meta) = std::fs::symlink_metadata(&full_path) {
        if meta.file_type().is_symlink() {
            return Err("Refusing to write through a symlink".to_string());
        }
    }

    Ok(full_path)
}

/// Validate a candidate CI edit and compute its diff without writing anything.
/// Used to show an approval preview before applying.
pub fn preview_ci_edit(
    project_path: &str,
    rel_path: &str,
    new_content: &str,
) -> Result<(CiFormat, String), String> {
    let format = ci_files::is_ci_file(rel_path).ok_or_else(|| {
        format!(
            "'{rel_path}' is not an editable CI/CD file. Allowed: .chibby/*.toml, \
             .github/workflows/*.yml, .circleci/config.yml, .drone.yml, .gitlab-ci.yml"
        )
    })?;
    ci_files::validate_content(format, new_content)
        .map_err(|e| format!("Invalid {format:?} config: {e}"))?;
    let full_path = guard_project_path(project_path, rel_path)?;
    Ok((format, compute_diff(&full_path, new_content)))
}

/// Edit a CI/CD file safely. Rejects non-CI paths and invalid content; backs up
/// the existing file; commits to a fresh `chibby/agent-edit-<ts>` branch when
/// the tree is clean, otherwise writes with a backup and skips the commit.
pub fn edit_ci_file(
    project_path: &str,
    rel_path: &str,
    new_content: &str,
) -> Result<EditResult, String> {
    let project = Path::new(project_path);

    let format = ci_files::is_ci_file(rel_path).ok_or_else(|| {
        format!(
            "'{rel_path}' is not an editable CI/CD file. Allowed: .chibby/*.toml, \
             .github/workflows/*.yml, .circleci/config.yml, .drone.yml, .gitlab-ci.yml"
        )
    })?;

    ci_files::validate_content(format, new_content)
        .map_err(|e| format!("Refusing to write invalid {format:?} config: {e}"))?;

    let full_path = guard_project_path(project_path, rel_path)?;
    let diff = compute_diff(&full_path, new_content);

    let is_repo = git::is_git_repo(project);
    let original_branch = if is_repo {
        git::current_branch(project).ok()
    } else {
        None
    };
    // `git add` refuses ignored paths, so a gitignored CI file can't be committed.
    let ignored = is_repo && git::is_path_ignored(project, rel_path);
    // Commit only when it's a repo, the tree is clean, and the target is tracked.
    let tree_clean = is_repo && !ignored && git::is_working_tree_clean(project);

    // Backup any existing file.
    let backup_path = if full_path.exists() {
        let bak = PathBuf::from(format!("{}.bak", full_path.display()));
        std::fs::copy(&full_path, &bak).map_err(|e| format!("Backup failed: {e}"))?;
        Some(bak.to_string_lossy().to_string())
    } else {
        None
    };

    audit::log_event(
        "agent_ci_edit",
        &format!("project={project_path} file={rel_path} git={tree_clean}"),
    );

    let mut result = EditResult {
        path: rel_path.to_string(),
        format,
        diff,
        backup_path,
        git_committed: false,
        branch: None,
        commit_sha: None,
        original_branch,
        note: None,
    };

    if tree_clean {
        let ts = chrono::Utc::now().timestamp();
        let branch_name = format!("chibby/agent-edit-{ts}");
        git::create_branch(project, &branch_name).map_err(|e| e.to_string())?;
        std::fs::write(&full_path, new_content).map_err(|e| format!("Write failed: {e}"))?;
        git::add(project, rel_path).map_err(|e| e.to_string())?;
        let sha = git::commit(project, &format!("chore(agent): edit {rel_path}"))
            .map_err(|e| e.to_string())?;
        result.git_committed = true;
        result.branch = Some(branch_name);
        result.commit_sha = Some(sha);
    } else {
        std::fs::write(&full_path, new_content).map_err(|e| format!("Write failed: {e}"))?;
        result.note = Some(if !is_repo {
            "Not a git repository; wrote the file with a .bak backup.".to_string()
        } else if ignored {
            format!(
                "'{rel_path}' is gitignored; wrote the file with a .bak backup but skipped the \
                 git commit. Remove it from .gitignore (Chibby only ignores *.local.toml) to version it."
            )
        } else {
            "Working tree had uncommitted changes; wrote the file with a .bak backup but skipped the git commit.".to_string()
        });
    }

    Ok(result)
}

/// Unified diff between the on-disk file (or empty) and the new content.
///
/// The candidate content (which may contain secrets) is written to a private
/// temp file created with O_EXCL and 0600 perms via `tempfile`, not a
/// predictable name in a world-readable `/tmp` — closing an info-leak and a
/// symlink-preseed clobber.
fn compute_diff(full_path: &Path, new_content: &str) -> String {
    use std::io::Write;

    let mut new_tmp = match tempfile::NamedTempFile::new() {
        Ok(f) => f,
        Err(_) => return String::new(),
    };
    if new_tmp.write_all(new_content.as_bytes()).is_err() {
        return String::new();
    }

    if full_path.exists() {
        git::diff_no_index(full_path, new_tmp.path())
    } else {
        let old_tmp = match tempfile::NamedTempFile::new() {
            Ok(f) => f,
            Err(_) => return String::new(),
        };
        git::diff_no_index(old_tmp.path(), new_tmp.path())
    }
    // Temp files are removed when the `NamedTempFile` handles drop here.
}
