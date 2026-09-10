//! Secret redaction for logs.
//!
//! Two independent layers:
//! - **Patterns** ([`redact`]) — heuristics for secret-shaped text (API keys,
//!   JWTs, PEM markers). Always applied.
//! - **Literals** ([`Redactor`]) — the exact secret values resolved for a run,
//!   masked wherever they appear. Only available where the run context is known.
//!
//! [`sanitize_log_line`] adds markdown-fence escaping on top of the patterns;
//! that is agent/LLM-prompt specific and must not be applied to pipeline logs,
//! which are shown verbatim to the user.

use regex::Regex;
use std::sync::OnceLock;

/// Placeholder written in place of a redacted value.
const PLACEHOLDER: &str = "[REDACTED]";

/// Values shorter than this are too generic to mask safely.
const MIN_LITERAL_LEN: usize = 6;

/// Common non-secret values that occasionally land in a secret store.
const LITERAL_DENYLIST: &[&str] = &["true", "false", "1", "0", "localhost"];

/// Regex patterns for common secret values that should be redacted from logs.
static SECRET_PATTERNS: &[&str] = &[
    // Generic API keys / tokens after a key-like prefix. Optional quotes/space
    // around the separator so JSON (`"api_key":"<v>"`) and shell forms both match.
    r#"(?i)(password|passwd|secret|token|api[_-]?key|apikey|auth|credential|private[_-]?key)["' ]*[:=]["' ]*\S+"#,
    // AWS-style keys
    r"(?i)AKIA[0-9A-Z]{16}",
    // Bearer tokens
    r"(?i)bearer\s+[a-zA-Z0-9\-._~+/]+=*",
    // GitHub personal / OAuth / user / server / refresh tokens
    r"gh[pousr]_[A-Za-z0-9]{20,}",
    // GitHub fine-grained personal access tokens
    r"github_pat_[A-Za-z0-9_]{20,}",
    // GitLab personal access tokens
    r"glpat-[A-Za-z0-9_-]{16,}",
    // Slack tokens (bot / app / user / refresh / legacy)
    r"xox[baprs]-[A-Za-z0-9-]{10,}",
    // Stripe live secret / restricted keys
    r"(?:sk|rk)_live_[A-Za-z0-9]{16,}",
    // Google API keys
    r"AIza[0-9A-Za-z_-]{35}",
    // Anthropic API keys (kept before the generic sk- rule so the ant- form is
    // fully covered rather than partially matched)
    r"sk-ant-[A-Za-z0-9_-]{20,}",
    // OpenAI-style secret keys
    r"sk-[A-Za-z0-9]{20,}",
    // JSON Web Tokens (base64url header.payload.signature)
    r"eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+",
    // PEM private key block markers (body lines are caught by the base64 rule)
    r"-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----",
    r"-----END [A-Z0-9 ]*PRIVATE KEY-----",
    // Base64-encoded long strings that look like secrets (64+ chars)
    r"[A-Za-z0-9+/]{64,}={0,3}",
];

static COMPILED_PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();

/// Compile the patterns once. They run on every stdout line of every build, so
/// compiling per call (as the agent path used to) is not affordable here.
/// `OnceLock` rather than `LazyLock` to stay within the crate's MSRV.
fn compiled_patterns() -> &'static [Regex] {
    COMPILED_PATTERNS.get_or_init(|| {
        SECRET_PATTERNS
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect()
    })
}

/// Redact secret-shaped substrings from a log line.
pub fn redact(line: &str) -> String {
    let mut out = line.to_string();
    for re in compiled_patterns() {
        if re.is_match(&out) {
            out = re.replace_all(&out, PLACEHOLDER).to_string();
        }
    }
    out
}

/// [`redact`] plus escaping of markdown code-fence breaks. Use for text headed
/// into an LLM prompt; never for pipeline logs shown to the user.
pub fn sanitize_log_line(line: &str) -> String {
    redact(line).replace("```", "` ` `")
}

/// Masks the exact secret values resolved for a run, wherever they appear in
/// output. Complements the pattern layer, which only catches known shapes.
#[derive(Debug, Default, Clone)]
pub struct Redactor {
    /// Literals to mask, longest first so a secret that is a prefix of another
    /// never leaves the longer one's tail behind.
    literals: Vec<String>,
}

impl Redactor {
    /// Build a redactor from resolved secret values. Values shorter than 6
    /// chars, and common non-secrets ("true", "false", "1", "0", "localhost"),
    /// are ignored — masking them would shred ordinary build output.
    pub fn new(secret_values: impl IntoIterator<Item = String>) -> Self {
        let mut literals: Vec<String> = secret_values
            .into_iter()
            .filter(|v| is_maskable(v))
            .collect();

        literals.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        literals.dedup();

        Self { literals }
    }

    /// Whether this redactor would leave every line untouched.
    pub fn is_noop(&self) -> bool {
        self.literals.is_empty()
    }

    /// Replace every known secret literal in `line` with the placeholder.
    pub fn redact(&self, line: &str) -> String {
        if self.is_noop() {
            return line.to_string();
        }

        let mut out = line.to_string();
        for literal in &self.literals {
            if out.contains(literal.as_str()) {
                out = out.replace(literal.as_str(), PLACEHOLDER);
            }
        }
        out
    }

    /// Full redaction for a pipeline log line: built-in patterns first, then
    /// this run's secret literals. This is what the executor applies at ingest.
    pub fn redact_log(&self, line: &str) -> String {
        self.redact(&redact(line))
    }
}

/// Whether a secret value is specific enough to mask without false positives.
fn is_maskable(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.len() < MIN_LITERAL_LEN {
        return false;
    }
    !LITERAL_DENYLIST
        .iter()
        .any(|deny| deny.eq_ignore_ascii_case(trimmed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_literal_secret_values() {
        let redactor = Redactor::new(["hunter2hunter2".to_string()]);

        let out = redactor.redact("connecting with pw=hunter2hunter2 done");

        assert_eq!(out, "connecting with pw=[REDACTED] done");
    }

    #[test]
    fn masks_longest_literal_first_leaving_no_tail() {
        // "abcdef123" is a prefix of "abcdef123456"; naive ordering would mask
        // the short one and leave "456" behind.
        let redactor = Redactor::new(["abcdef123".to_string(), "abcdef123456".to_string()]);

        let out = redactor.redact("token=abcdef123456");

        assert_eq!(out, "token=[REDACTED]");
        assert!(!out.contains("456"));
    }

    #[test]
    fn ignores_short_and_common_values() {
        let redactor = Redactor::new([
            "true".to_string(),
            "0".to_string(),
            "abc".to_string(),
            "localhost".to_string(),
        ]);

        assert!(redactor.is_noop());
        assert_eq!(
            redactor.redact("listening on localhost, debug=true"),
            "listening on localhost, debug=true"
        );
    }

    #[test]
    fn empty_redactor_is_noop() {
        assert!(Redactor::new(Vec::<String>::new()).is_noop());
    }

    #[test]
    fn pattern_redaction_still_works() {
        let key = format!("AKIA{}", "ABCDEFGHIJKLMNOP");

        let out = redact(&format!("aws key {key} used"));

        assert!(out.contains(PLACEHOLDER), "not redacted: {out:?}");
        assert!(!out.contains(&key), "secret leaked: {out:?}");
    }

    #[test]
    fn pattern_redaction_leaves_ordinary_lines_intact() {
        for line in [
            "Compiling chibby v0.2.3 (/app/src-tauri)",
            "test result: ok. 42 passed; 0 failed",
        ] {
            assert_eq!(redact(line), line, "line was altered: {line:?}");
        }
    }

    #[test]
    fn pipeline_redaction_does_not_mangle_backticks() {
        let redactor = Redactor::new(Vec::<String>::new());
        let line = "run ```rust code``` now";

        assert_eq!(redactor.redact_log(line), line);
        // The agent path still escapes fences.
        assert!(sanitize_log_line(line).contains("` ` `"));
    }

    #[test]
    fn redact_log_applies_patterns_and_literals() {
        let redactor = Redactor::new(["s3cr3tvalue".to_string()]);

        let out = redactor.redact_log("deploying with TOKEN s3cr3tvalue");

        assert!(!out.contains("s3cr3tvalue"), "literal leaked: {out:?}");
    }
}
