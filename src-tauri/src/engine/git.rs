//! Thin git helpers (branch / commit / diff) used by the agent's git-safe CI
//! file editing. Arg-vector `git` invocations only — no shell.

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

/// Run `git <args>` in `repo`, returning trimmed stdout. Errors on non-zero exit.
fn run_git(repo: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .with_context(|| format!("Failed to run git {}", args.join(" ")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git {} failed: {}", args.join(" "), stderr.trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Whether `repo` is inside a git work tree.
pub fn is_git_repo(repo: &Path) -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(repo)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Current branch name (or `HEAD` when detached).
pub fn current_branch(repo: &Path) -> Result<String> {
    run_git(repo, &["rev-parse", "--abbrev-ref", "HEAD"])
}

/// Whether the working tree has no uncommitted changes.
pub fn is_working_tree_clean(repo: &Path) -> bool {
    run_git(repo, &["status", "--porcelain"])
        .map(|s| s.is_empty())
        .unwrap_or(false)
}

/// Whether a repo-relative path is ignored by gitignore rules. `git add` refuses
/// ignored paths, so the caller can skip the commit instead of erroring.
pub fn is_path_ignored(repo: &Path, rel_path: &str) -> bool {
    Command::new("git")
        .args(["check-ignore", "-q", "--", rel_path])
        .current_dir(repo)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Whether a local branch exists.
pub fn branch_exists(repo: &Path, name: &str) -> bool {
    run_git(
        repo,
        &["rev-parse", "--verify", &format!("refs/heads/{name}")],
    )
    .is_ok()
}

/// Create and switch to a new branch off the current HEAD.
pub fn create_branch(repo: &Path, name: &str) -> Result<()> {
    run_git(repo, &["checkout", "-b", name]).map(|_| ())
}

/// Switch to an existing branch.
pub fn checkout(repo: &Path, name: &str) -> Result<()> {
    run_git(repo, &["checkout", name]).map(|_| ())
}

/// Stage a single pathspec (relative to repo root).
pub fn add(repo: &Path, pathspec: &str) -> Result<()> {
    run_git(repo, &["add", "--", pathspec]).map(|_| ())
}

/// Commit staged changes; returns the new commit SHA.
pub fn commit(repo: &Path, message: &str) -> Result<String> {
    run_git(repo, &["commit", "-m", message])?;
    run_git(repo, &["rev-parse", "HEAD"])
}

/// Unified diff between `old` and `new` files via `git diff --no-index`.
/// `git diff --no-index` exits non-zero when files differ, which is expected —
/// we return whatever it prints to stdout.
pub fn diff_no_index(old: &Path, new: &Path) -> String {
    Command::new("git")
        .args([
            "diff",
            "--no-index",
            "--",
            &old.to_string_lossy(),
            &new.to_string_lossy(),
        ])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default()
}
