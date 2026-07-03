//! Lightweight in-process scanner that flags values that *look* like real
//! credentials inside Chibby config files. Used to warn before saving
//! `environments.toml` when someone typo'd a real token into a variable's
//! value (instead of declaring it as a secret).
//!
//! Distinct from `gates::run_secret_scan` which orchestrates `gitleaks`
//! across the whole repo. This module is small, dependency-free (just
//! `regex`), and synchronous so it can run in `save_*` hot paths.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// What a single match looks like.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeakMatch {
    /// Identifier of the rule that matched (e.g. `"github-pat"`).
    pub rule: String,
    /// First match offset in the input.
    pub start: usize,
    /// One-past-end offset in the input.
    pub end: usize,
    /// Redacted preview of the matched substring (first 6 chars + length hint).
    pub preview: String,
}

/// (rule_name, regex_source).
const RULES: &[(&str, &str)] = &[
    // GitHub
    ("github-pat", r"\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9_]{36,}"),
    (
        "github-fine-grained-pat",
        r"\bgithub_pat_[A-Za-z0-9_]{82}\b",
    ),
    // GitLab
    ("gitlab-pat", r"\bglpat-[A-Za-z0-9_\-]{20,}\b"),
    // OpenAI / Anthropic style
    ("openai-key", r"\bsk-[A-Za-z0-9]{20,}\b"),
    ("anthropic-key", r"\bsk-ant-[A-Za-z0-9_\-]{40,}\b"),
    // Slack
    ("slack-token", r"\bxox[bporas]-\d+-\d+-[A-Za-z0-9]+\b"),
    // Stripe
    (
        "stripe-key",
        r"\b(?:sk|pk|rk)_(?:live|test)_[A-Za-z0-9]{24,}\b",
    ),
    // SendGrid
    (
        "sendgrid-key",
        r"\bSG\.[A-Za-z0-9_\-]{22}\.[A-Za-z0-9_\-]{43}\b",
    ),
    // AWS
    ("aws-access-key-id", r"\bAKIA[0-9A-Z]{16}\b"),
    // Twilio
    ("twilio-key", r"\bSK[a-f0-9]{32}\b"),
    // Private key blocks (in case someone pastes a whole key)
    (
        "private-key-block",
        r"-----BEGIN (?:RSA |EC |DSA |OPENSSH )?PRIVATE KEY-----",
    ),
    // Database URLs with embedded credentials
    (
        "db-url-with-credentials",
        r"\b(?:postgres|mysql|mongodb|redis)://[^:\s]+:[^@\s]+@",
    ),
];

static COMPILED: OnceLock<Vec<(String, Regex)>> = OnceLock::new();

fn compiled() -> &'static [(String, Regex)] {
    COMPILED.get_or_init(|| {
        RULES
            .iter()
            .filter_map(|(name, src)| Regex::new(src).ok().map(|re| (name.to_string(), re)))
            .collect()
    })
}

/// Scan a chunk of text and return every match found.
/// Order: first match per rule, then by offset. Empty result = clean.
pub fn scan(content: &str) -> Vec<LeakMatch> {
    let mut out = Vec::new();
    for (name, re) in compiled() {
        for m in re.find_iter(content) {
            out.push(LeakMatch {
                rule: name.clone(),
                start: m.start(),
                end: m.end(),
                preview: redact(m.as_str()),
            });
        }
    }
    out.sort_by_key(|m| m.start);
    out
}

/// Returns true if the content has any matches. Cheaper than `scan` when the
/// caller only needs a yes/no decision.
pub fn has_leak(content: &str) -> bool {
    compiled().iter().any(|(_, re)| re.is_match(content))
}

/// Keep the first 4 characters + an obfuscation tail so the caller can show
/// "ghp_…(40 chars)" in a warning without echoing the secret.
fn redact(s: &str) -> String {
    let len = s.len();
    let head: String = s.chars().take(4).collect();
    format!("{}…({} chars)", head, len)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_text_is_clean() {
        assert!(scan("API_URL = \"https://api.example.com\"").is_empty());
        assert!(scan("LOG_LEVEL = \"info\"").is_empty());
        assert!(scan("PORT = 8080").is_empty());
    }

    // Test fixtures are assembled at runtime so the source file itself
    // contains no contiguous strings that look like real credentials —
    // otherwise GitHub's push protection blocks the commit.
    fn fake(prefix: &str, body_len: usize) -> String {
        let mut s = String::from(prefix);
        for _ in 0..body_len {
            s.push('A');
        }
        s
    }

    #[test]
    fn github_pat_caught() {
        let token = fake("g".to_owned().as_str(), 0) + "hp_" + &"X".repeat(40);
        let text = format!("TOKEN = \"{}\"", token);
        let hits = scan(&text);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].rule, "github-pat");
        assert!(hits[0].preview.starts_with("ghp_"));
        // The body chars never appear in the preview.
        assert!(!hits[0].preview.contains("XXXXXXX"));
    }

    #[test]
    fn stripe_key_caught_in_toml_value() {
        let key = String::from("s") + "k" + "_live_" + &"Y".repeat(28);
        let toml = format!("\n[environments.variables]\nSTRIPE_LIVE = \"{}\"\n", key);
        let hits = scan(&toml);
        assert!(hits.iter().any(|m| m.rule == "stripe-key"));
    }

    #[test]
    fn slack_token_caught() {
        let s = String::from("x") + "ox" + "b-1234567890-1234567890-" + &"Z".repeat(24);
        assert!(has_leak(&s));
        let hits = scan(&s);
        assert_eq!(hits[0].rule, "slack-token");
    }

    #[test]
    fn db_url_with_creds_caught() {
        let s = format!(
            "DATABASE_URL = \"{}://{}@localhost/db\"",
            "postgres", "user:passw0rd"
        );
        let hits = scan(&s);
        assert!(hits.iter().any(|m| m.rule == "db-url-with-credentials"));
    }

    #[test]
    fn private_key_block_caught() {
        let header = String::from("-----") + "BEGIN PRIVATE KEY" + "-----";
        let s = format!("K = \"{}\\nbody...\"", header);
        assert!(has_leak(&s));
    }

    #[test]
    fn aws_access_key_caught() {
        let key = String::from("A") + "KIA" + "IOSFODNN7EXAMPLE";
        let s = format!("ACCESS_KEY = \"{}\"", key);
        let hits = scan(&s);
        assert!(hits.iter().any(|m| m.rule == "aws-access-key-id"));
    }

    #[test]
    fn redact_obscures_real_value() {
        let v = String::from("g") + "hp_" + &"Q".repeat(36);
        let r = redact(&v);
        assert!(r.starts_with("ghp_"));
        // 8+ char body never echoed
        assert!(!r.contains("QQQQQQQQ"));
    }

    #[test]
    fn results_sorted_by_offset() {
        let aws = String::from("A") + "KIA" + "1234567890ABCDEF";
        let gh = String::from("g") + "hp_" + &"W".repeat(36);
        let s = format!("{} and later {}", aws, gh);
        let hits = scan(&s);
        assert!(hits.len() >= 2);
        for w in hits.windows(2) {
            assert!(w[0].start <= w[1].start);
        }
    }
}
