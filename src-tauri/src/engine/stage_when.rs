//! Declarative `when` conditions deciding whether a stage runs.
//!
//! Deliberately not an expression language: populated fields are ANDed,
//! entries within a field are ORed, and every entry is a glob pattern.

use crate::engine::models::StageWhen;

/// The run facts a `when` block is evaluated against.
pub struct WhenContext<'a> {
    pub branch: Option<&'a str>,
    pub environment: Option<&'a str>,
}

/// Whether the stage should run. `Err` when a pattern is not a valid glob.
pub fn should_run(when: &Option<StageWhen>, ctx: &WhenContext) -> Result<bool, String> {
    skip_reason(when, ctx).map(|reason| reason.is_none())
}

/// `Ok(None)` when the stage should run, `Ok(Some(reason))` when it should be
/// skipped. `Err` when a pattern is not a valid glob — an unparseable condition
/// is a configuration error, never a silent skip.
pub fn skip_reason(when: &Option<StageWhen>, ctx: &WhenContext) -> Result<Option<String>, String> {
    let Some(when) = when else {
        return Ok(None);
    };

    if let Some(reason) = check_include("branch", ctx.branch, &when.branch)? {
        return Ok(Some(reason));
    }
    if let Some(reason) = check_exclude("branch", ctx.branch, &when.branch_not)? {
        return Ok(Some(reason));
    }
    if let Some(reason) = check_include("environment", ctx.environment, &when.environment)? {
        return Ok(Some(reason));
    }
    if let Some(reason) = check_exclude("environment", ctx.environment, &when.environment_not)? {
        return Ok(Some(reason));
    }

    Ok(None)
}

/// The value must match at least one pattern (when any are configured).
fn check_include(
    field: &str,
    value: Option<&str>,
    patterns: &[String],
) -> Result<Option<String>, String> {
    if patterns.is_empty() {
        return Ok(None);
    }

    let Some(value) = value else {
        return Ok(Some(format!(
            "when: {field} is unknown, cannot match {}",
            render(patterns)
        )));
    };

    if matches_any(value, patterns)? {
        return Ok(None);
    }

    Ok(Some(format!(
        "when: {field} '{value}' does not match {}",
        render(patterns)
    )))
}

/// The value must match none of the patterns.
fn check_exclude(
    field: &str,
    value: Option<&str>,
    patterns: &[String],
) -> Result<Option<String>, String> {
    if patterns.is_empty() {
        return Ok(None);
    }

    let Some(value) = value else {
        return Ok(None);
    };

    if !matches_any(value, patterns)? {
        return Ok(None);
    }

    Ok(Some(format!(
        "when: {field} '{value}' is excluded by {}",
        render(patterns)
    )))
}

/// Whether `value` matches any glob in `patterns`.
fn matches_any(value: &str, patterns: &[String]) -> Result<bool, String> {
    for pattern in patterns {
        let compiled = glob::Pattern::new(pattern)
            .map_err(|e| format!("invalid glob pattern '{pattern}': {e}"))?;
        if compiled.matches(value) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Render a pattern list for a human-readable skip reason.
fn render(patterns: &[String]) -> String {
    format!("[{}]", patterns.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx<'a>(branch: Option<&'a str>, environment: Option<&'a str>) -> WhenContext<'a> {
        WhenContext {
            branch,
            environment,
        }
    }

    #[test]
    fn none_always_runs() {
        assert!(should_run(&None, &ctx(Some("feat/x"), None)).unwrap());
    }

    #[test]
    fn empty_when_always_runs() {
        let when = Some(StageWhen::default());

        assert!(should_run(&when, &ctx(Some("feat/x"), Some("prod"))).unwrap());
    }

    #[test]
    fn branch_glob_matches_release_prefix() {
        let when = Some(StageWhen {
            branch: vec!["main".to_string(), "release/*".to_string()],
            ..Default::default()
        });

        assert!(should_run(&when, &ctx(Some("release/1.2"), None)).unwrap());
        assert!(should_run(&when, &ctx(Some("main"), None)).unwrap());
        assert!(!should_run(&when, &ctx(Some("feat/x"), None)).unwrap());
    }

    #[test]
    fn skip_reason_names_the_branch_and_patterns() {
        let when = Some(StageWhen {
            branch: vec!["main".to_string()],
            ..Default::default()
        });

        let reason = skip_reason(&when, &ctx(Some("feat/x"), None))
            .unwrap()
            .unwrap();

        assert_eq!(reason, "when: branch 'feat/x' does not match [main]");
    }

    #[test]
    fn branch_not_excludes() {
        let when = Some(StageWhen {
            branch_not: vec!["wip/*".to_string()],
            ..Default::default()
        });

        assert!(!should_run(&when, &ctx(Some("wip/thing"), None)).unwrap());
        assert!(should_run(&when, &ctx(Some("main"), None)).unwrap());
    }

    #[test]
    fn fields_are_anded() {
        let when = Some(StageWhen {
            branch: vec!["main".to_string()],
            environment: vec!["prod".to_string()],
            ..Default::default()
        });

        assert!(should_run(&when, &ctx(Some("main"), Some("prod"))).unwrap());
        assert!(!should_run(&when, &ctx(Some("main"), Some("staging"))).unwrap());
        assert!(!should_run(&when, &ctx(Some("dev"), Some("prod"))).unwrap());
    }

    #[test]
    fn unknown_branch_cannot_satisfy_an_include() {
        let when = Some(StageWhen {
            branch: vec!["main".to_string()],
            ..Default::default()
        });

        let reason = skip_reason(&when, &ctx(None, None)).unwrap().unwrap();

        assert!(reason.contains("branch is unknown"), "got {reason:?}");
    }

    #[test]
    fn invalid_glob_is_an_error() {
        let when = Some(StageWhen {
            branch: vec!["release/[".to_string()],
            ..Default::default()
        });

        let err = should_run(&when, &ctx(Some("main"), None)).unwrap_err();

        assert!(err.contains("invalid glob pattern"), "got {err:?}");
    }
}
