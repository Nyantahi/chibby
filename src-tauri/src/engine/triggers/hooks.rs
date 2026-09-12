//! Git hook installation.
//!
//! Chibby's block is delimited by sentinels so installs are idempotent and
//! uninstalls are surgical: whatever else lives in the user's hook is left
//! exactly as it was.

use super::HookSpec;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const BLOCK_START: &str = "# >>> chibby managed >>>";
const BLOCK_END: &str = "# <<< chibby managed <<<";
const SHEBANG: &str = "#!/bin/sh";

/// Git hooks Chibby knows how to write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookKind {
    PrePush,
    PreCommit,
}

impl HookKind {
    /// The hook's file name, and the `--trigger` tag runs from it carry.
    pub fn file_name(&self) -> &'static str {
        match self {
            HookKind::PrePush => "pre-push",
            HookKind::PreCommit => "pre-commit",
        }
    }

    pub fn trigger_tag(&self) -> String {
        format!("hook:{}", self.file_name())
    }
}

/// What is currently sitting at the hook path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookState {
    NotInstalled,
    /// Written by Chibby and nothing else.
    ChibbyManaged,
    /// Someone else's hook, with no Chibby block.
    Foreign,
    /// Someone else's hook that already contains a Chibby block.
    ForeignWithChibbyBlock,
}

/// How to treat a hook file that already exists.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallMode {
    /// Refuse to touch a foreign hook; hand the snippet back instead.
    #[default]
    Safe,
    /// Back the foreign hook up, then replace it.
    Force,
    /// Insert the Chibby block after the shebang, leaving the rest intact.
    Append,
}

/// Outcome of an install attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallReport {
    pub path: PathBuf,
    pub state_before: HookState,
    pub installed: bool,
    /// Where the previous hook was saved, when `Force` backed one up.
    pub backup_path: Option<PathBuf>,
    /// The generated block — printed for manual pasting when `Safe` refuses.
    pub snippet: String,
    pub message: String,
}

/// Path to a repo's hook file.
pub fn hook_path(repo: &Path, kind: HookKind) -> PathBuf {
    repo.join(".git").join("hooks").join(kind.file_name())
}

/// Inspect the hook currently installed for `repo`.
pub fn status(repo: &Path, kind: HookKind) -> Result<HookState> {
    let path = hook_path(repo, kind);
    if !path.exists() {
        return Ok(HookState::NotInstalled);
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    Ok(classify(&content))
}

fn classify(content: &str) -> HookState {
    if !content.contains(BLOCK_START) {
        return HookState::Foreign;
    }
    // Ours alone when nothing outside the shebang and the block survives.
    let leftovers = strip_block(content)
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with("#!"))
        .count();
    if leftovers == 0 {
        HookState::ChibbyManaged
    } else {
        HookState::ForeignWithChibbyBlock
    }
}

/// Install (or refresh) the Chibby block in a repo's hook.
pub fn install(
    repo: &Path,
    kind: HookKind,
    spec: &HookSpec,
    mode: InstallMode,
) -> Result<InstallReport> {
    let path = hook_path(repo, kind);
    let hooks_dir = path
        .parent()
        .context("Hook path has no parent directory")?
        .to_path_buf();
    if !hooks_dir.exists() {
        anyhow::bail!(
            "{} does not exist — is {} a git repository?",
            hooks_dir.display(),
            repo.display()
        );
    }

    let snippet = render_block(repo, kind, spec);
    let state_before = status(repo, kind)?;
    let existing = std::fs::read_to_string(&path).unwrap_or_default();

    let mut backup_path = None;
    let content = match state_before {
        HookState::NotInstalled => fresh_hook(&snippet),
        // Already ours: rewrite the block in place, whatever the mode.
        HookState::ChibbyManaged | HookState::ForeignWithChibbyBlock => {
            replace_block(&existing, &snippet)
        }
        HookState::Foreign => match mode {
            InstallMode::Safe => {
                return Ok(InstallReport {
                    path: path.clone(),
                    state_before,
                    installed: false,
                    backup_path: None,
                    snippet,
                    message: format!(
                        "{} already exists and was not written by Chibby. \
                         Add the snippet manually, or re-run with --append or --force.",
                        path.display()
                    ),
                })
            }
            InstallMode::Force => {
                backup_path = Some(back_up(&path)?);
                fresh_hook(&snippet)
            }
            InstallMode::Append => insert_after_shebang(&existing, &snippet),
        },
    };

    write_executable(&path, &content)?;

    Ok(InstallReport {
        path: path.clone(),
        state_before,
        installed: true,
        backup_path,
        snippet,
        message: format!("Installed Chibby {} hook", kind.file_name()),
    })
}

/// Remove Chibby's block, leaving any foreign hook exactly as it was.
pub fn uninstall(repo: &Path, kind: HookKind) -> Result<()> {
    let path = hook_path(repo, kind);
    if !path.exists() {
        return Ok(());
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    if !content.contains(BLOCK_START) {
        return Ok(());
    }

    let stripped = strip_block(&content);
    let has_foreign_content = stripped
        .lines()
        .any(|l| !l.trim().is_empty() && !l.starts_with("#!"));

    if has_foreign_content {
        return write_executable(&path, &stripped);
    }

    std::fs::remove_file(&path).with_context(|| format!("Failed to remove {}", path.display()))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Script generation
// ---------------------------------------------------------------------------

/// The Chibby block for one hook, sentinels included.
pub fn render_block(repo: &Path, kind: HookKind, spec: &HookSpec) -> String {
    let binary = resolve_cli_binary()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut command = format!(
        "\"$CHIBBY_BIN\" run --project {} --trigger {}",
        shell_quote(&repo.to_string_lossy()),
        kind.trigger_tag()
    );
    for stage in &spec.stages {
        command.push_str(&format!(" --stage {}", shell_quote(stage)));
    }
    if let Some(env) = &spec.environment {
        command.push_str(&format!(" --env {}", shell_quote(env)));
    }
    if let Some(file) = &spec.pipeline_file {
        command.push_str(&format!(" --pipeline {}", shell_quote(file)));
    }
    // `sh` exits with the status of its *last* command, and `--append` puts
    // this block above the user's own hook body, so a blocking failure has to
    // exit on the spot or it would be discarded. Non-blocking hooks report but
    // never stand between the user and git.
    command.push_str(match spec.blocking {
        true => " || exit 1",
        false => " || true",
    });

    [
        BLOCK_START,
        "# Generated by Chibby. Remove with: chibby hooks uninstall",
        &format!("CHIBBY_BIN={}", shell_quote(&binary)),
        "if [ ! -x \"$CHIBBY_BIN\" ]; then",
        "  CHIBBY_BIN=\"$(command -v chibby 2>/dev/null)\"",
        "fi",
        // Fail open, but only for Chibby's own command: an `exit 0` here would
        // also skip the rest of a foreign hook the block was appended to, so a
        // missing binary would silently stop the user's lint/test hook too.
        "if [ -z \"$CHIBBY_BIN\" ]; then",
        "  echo \"[chibby] binary not found — skipping hook\" >&2",
        "else",
        &format!("  {command}"),
        "fi",
        BLOCK_END,
    ]
    .join("\n")
}

/// Absolute path to the Chibby CLI, resolved at install time.
///
/// Prefers a sibling `chibby-cli`/`chibby` next to the running executable so a
/// hook installed from the desktop app still points at the CLI. The generated
/// script falls back to `command -v chibby` when nothing is found here.
///
/// Never falls back to the running executable itself: the CLI binary is
/// feature-gated and is not bundled in the .app, so that would put the desktop
/// binary in the hook — `git push` would launch a second app window (and, for
/// a blocking hook, wait for the user to close it) instead of running the
/// pipeline, with `[ -x ]` passing so the `command -v` fallback never fires.
fn resolve_cli_binary() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;

    ["chibby-cli", "chibby"]
        .iter()
        .map(|name| dir.join(name))
        .find(|candidate| candidate.is_file())
}

/// Single-quote a value for `/bin/sh`.
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

fn fresh_hook(block: &str) -> String {
    format!("{SHEBANG}\n{block}\n")
}

fn insert_after_shebang(existing: &str, block: &str) -> String {
    let mut lines: Vec<String> = existing.lines().map(str::to_string).collect();
    let insert_at = usize::from(lines.first().is_some_and(|l| l.starts_with("#!")));
    for (offset, line) in block.lines().enumerate() {
        lines.insert(insert_at + offset, line.to_string());
    }
    format!("{}\n", lines.join("\n"))
}

/// Swap an existing Chibby block for a freshly rendered one.
fn replace_block(existing: &str, block: &str) -> String {
    insert_after_shebang(&strip_block(existing), block)
}

/// Drop the sentinel-delimited block, keeping every other line untouched.
fn strip_block(content: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut inside = false;
    for line in content.lines() {
        if line.trim() == BLOCK_START {
            inside = true;
            continue;
        }
        if line.trim() == BLOCK_END {
            inside = false;
            continue;
        }
        if !inside {
            out.push(line);
        }
    }
    if out.is_empty() {
        return String::new();
    }
    format!("{}\n", out.join("\n"))
}

fn back_up(path: &Path) -> Result<PathBuf> {
    let stamp = chrono::Utc::now().format("%Y%m%d%H%M%S");
    let name = format!(
        "{}.chibby-backup.{stamp}",
        path.file_name().unwrap_or_default().to_string_lossy()
    );
    let backup = path.with_file_name(name);
    std::fs::copy(path, &backup)
        .with_context(|| format!("Failed to back up {} ", path.display()))?;
    Ok(backup)
}

fn write_executable(path: &Path, content: &str) -> Result<()> {
    std::fs::write(path, content).with_context(|| format!("Failed to write {}", path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .with_context(|| format!("Failed to chmod {}", path.display()))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const FOREIGN: &str = "#!/bin/sh\necho \"my own pre-push\"\nexit 0\n";

    fn repo_with_hooks() -> TempDir {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join(".git").join("hooks")).unwrap();
        temp
    }

    fn spec() -> HookSpec {
        HookSpec {
            stages: vec!["lint".to_string(), "test".to_string()],
            ..Default::default()
        }
    }

    fn read_hook(repo: &Path) -> String {
        std::fs::read_to_string(hook_path(repo, HookKind::PrePush)).unwrap()
    }

    #[test]
    fn test_fresh_install_writes_an_executable_hook() {
        let temp = repo_with_hooks();
        assert_eq!(
            status(temp.path(), HookKind::PrePush).unwrap(),
            HookState::NotInstalled
        );

        let report = install(temp.path(), HookKind::PrePush, &spec(), InstallMode::Safe).unwrap();

        assert!(report.installed);
        let body = read_hook(temp.path());
        assert!(body.starts_with(SHEBANG));
        assert!(body.contains("--trigger hook:pre-push"));
        assert!(body.contains("--stage 'lint'"));
        assert_eq!(
            status(temp.path(), HookKind::PrePush).unwrap(),
            HookState::ChibbyManaged
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(hook_path(temp.path(), HookKind::PrePush))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o755);
        }
    }

    /// The generated body must carry an absolute path and never break `git push`.
    #[test]
    fn test_generated_body_is_absolute_and_fails_open() {
        let temp = repo_with_hooks();
        install(temp.path(), HookKind::PrePush, &spec(), InstallMode::Safe).unwrap();
        let body = read_hook(temp.path());

        // Either an absolute sibling CLI or empty — never a relative path, and
        // never the running (possibly GUI) executable.
        let exe = std::env::current_exe().unwrap();
        assert!(
            body.contains("CHIBBY_BIN='/") || body.contains("CHIBBY_BIN=''"),
            "not an absolute path: {body}"
        );
        assert!(
            !body.contains(&format!("CHIBBY_BIN='{}'", exe.display())),
            "hook points at the running executable: {body}"
        );
        assert!(body.contains("command -v chibby"));
        assert!(
            body.contains("skipping hook"),
            "missing fail-open guard: {body}"
        );
    }

    /// The fail-open branch must skip only Chibby's command: an `exit 0` there
    /// would also stop the foreign hook body an appended block sits above.
    #[test]
    fn test_fail_open_does_not_abort_a_foreign_hook() {
        let temp = repo_with_hooks();
        std::fs::write(hook_path(temp.path(), HookKind::PrePush), FOREIGN).unwrap();

        install(temp.path(), HookKind::PrePush, &spec(), InstallMode::Append).unwrap();
        let body = read_hook(temp.path());

        assert!(body.contains("else"), "fail-open should branch: {body}");
        assert!(
            !body.contains("  exit 0"),
            "fail-open still aborts the script: {body}"
        );
        assert!(
            body.contains("echo \"my own pre-push\""),
            "lost the foreign body"
        );
    }

    /// Blocking hooks sit above the foreign body under `--append`, so they must
    /// exit on failure rather than let the last command decide the status.
    #[test]
    fn test_blocking_spec_exits_on_failure() {
        let temp = repo_with_hooks();

        install(temp.path(), HookKind::PrePush, &spec(), InstallMode::Safe).unwrap();

        assert!(read_hook(temp.path()).contains("|| exit 1"));
    }

    #[test]
    fn test_safe_mode_refuses_a_foreign_hook() {
        let temp = repo_with_hooks();
        std::fs::write(hook_path(temp.path(), HookKind::PrePush), FOREIGN).unwrap();

        let report = install(temp.path(), HookKind::PrePush, &spec(), InstallMode::Safe).unwrap();

        assert!(!report.installed);
        assert_eq!(report.state_before, HookState::Foreign);
        assert!(report.snippet.contains(BLOCK_START));
        assert_eq!(read_hook(temp.path()), FOREIGN, "foreign hook was modified");
    }

    #[test]
    fn test_force_mode_backs_up_before_replacing() {
        let temp = repo_with_hooks();
        std::fs::write(hook_path(temp.path(), HookKind::PrePush), FOREIGN).unwrap();

        let report = install(temp.path(), HookKind::PrePush, &spec(), InstallMode::Force).unwrap();

        assert!(report.installed);
        let backup = report.backup_path.expect("force should back up");
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), FOREIGN);
        assert!(!read_hook(temp.path()).contains("my own pre-push"));
    }

    #[test]
    fn test_append_mode_is_idempotent_and_keeps_foreign_content() {
        let temp = repo_with_hooks();
        std::fs::write(hook_path(temp.path(), HookKind::PrePush), FOREIGN).unwrap();

        install(temp.path(), HookKind::PrePush, &spec(), InstallMode::Append).unwrap();
        let once = read_hook(temp.path());
        install(temp.path(), HookKind::PrePush, &spec(), InstallMode::Append).unwrap();
        let twice = read_hook(temp.path());

        assert_eq!(once, twice, "second install changed the hook");
        assert_eq!(once.matches(BLOCK_START).count(), 1);
        assert!(once.contains("my own pre-push"));
        assert_eq!(
            status(temp.path(), HookKind::PrePush).unwrap(),
            HookState::ForeignWithChibbyBlock
        );
    }

    #[test]
    fn test_uninstall_restores_the_foreign_script_exactly() {
        let temp = repo_with_hooks();
        std::fs::write(hook_path(temp.path(), HookKind::PrePush), FOREIGN).unwrap();
        install(temp.path(), HookKind::PrePush, &spec(), InstallMode::Append).unwrap();

        uninstall(temp.path(), HookKind::PrePush).unwrap();

        assert_eq!(read_hook(temp.path()), FOREIGN);
    }

    #[test]
    fn test_uninstall_removes_a_hook_chibby_owns_entirely() {
        let temp = repo_with_hooks();
        install(temp.path(), HookKind::PrePush, &spec(), InstallMode::Safe).unwrap();

        uninstall(temp.path(), HookKind::PrePush).unwrap();

        assert!(!hook_path(temp.path(), HookKind::PrePush).exists());
        assert_eq!(
            status(temp.path(), HookKind::PrePush).unwrap(),
            HookState::NotInstalled
        );
    }

    #[test]
    fn test_non_blocking_spec_never_fails_the_git_operation() {
        let temp = repo_with_hooks();
        let spec = HookSpec {
            blocking: false,
            ..Default::default()
        };

        install(temp.path(), HookKind::PreCommit, &spec, InstallMode::Safe).unwrap();
        let body = std::fs::read_to_string(hook_path(temp.path(), HookKind::PreCommit)).unwrap();

        assert!(body.contains("|| true"));
        assert!(body.contains("--trigger hook:pre-commit"));
    }

    #[test]
    fn test_install_rejects_a_directory_without_git_hooks() {
        let temp = TempDir::new().unwrap();
        let err = install(temp.path(), HookKind::PrePush, &spec(), InstallMode::Safe).unwrap_err();

        assert!(err.to_string().contains("git repository"));
    }
}
