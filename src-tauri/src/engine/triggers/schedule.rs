//! Cron scheduling decisions.
//!
//! Everything here is pure and clock-injected: `due_now` takes `now` and the
//! last fire time as arguments, so the whole policy — including missed-run
//! catch-up — is unit-testable without sleeping.

use super::{MissedPolicy, ScheduleTrigger};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, Utc};
use cron::Schedule;
use std::str::FromStr;

/// A fire time this recent counts as "on time" rather than missed. Wider than
/// any sane tick interval, narrow enough that a nightly job never runs at noon.
const ON_TIME_GRACE_SECS: i64 = 300;

/// Upper bound on occurrences counted while catching up, so a `@yearly`-style
/// gap can never spin the iterator.
const MAX_MISSED_SCANNED: usize = 1000;

/// What the scheduler should do with one trigger right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Run it, as of this scheduled time.
    Fire { scheduled_for: DateTime<Utc> },
    /// Do not run, but move the trigger's baseline forward and say why.
    Skip { reason: String },
    /// Nothing to do yet.
    NotDue,
}

/// Parse a cron expression, accepting the 5-field form everyone actually types.
///
/// The `cron` crate wants `sec min hour dom mon dow [year]`, so a plain
/// `0 3 * * *` fails to parse outright. A 5-field expression is normalised by
/// prepending a `0` seconds field.
pub fn parse_cron(expr: &str) -> Result<Schedule> {
    let trimmed = expr.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("Cron expression is empty"));
    }

    let field_count = trimmed.split_whitespace().count();
    let normalised = match field_count {
        5 => format!("0 {trimmed}"),
        6 | 7 => trimmed.to_string(),
        // Named expressions like `@daily` are a single "field" the crate handles.
        1 if trimmed.starts_with('@') => trimmed.to_string(),
        n => {
            return Err(anyhow!(
                "Cron expression '{expr}' has {n} fields; expected 5 (min hour dom mon dow) or 6-7 (sec min hour dom mon dow [year])"
            ))
        }
    };

    Schedule::from_str(&normalised).map_err(|e| anyhow!("Invalid cron expression '{expr}': {e}"))
}

/// The first fire time strictly after `after`.
pub fn next_due(cron: &str, after: DateTime<Utc>) -> Result<Option<DateTime<Utc>>> {
    Ok(parse_cron(cron)?.after(&after).next())
}

/// The next `count` fire times after `after` — powers `--dry-run` and the UI.
pub fn next_run_times(
    cron: &str,
    after: DateTime<Utc>,
    count: usize,
) -> Result<Vec<DateTime<Utc>>> {
    Ok(parse_cron(cron)?.after(&after).take(count).collect())
}

/// Decide whether `trig` should fire, given when it last fired and the time now.
pub fn due_now(
    trig: &ScheduleTrigger,
    last_fired: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Decision {
    if !trig.enabled {
        return Decision::NotDue;
    }

    let schedule = match parse_cron(&trig.cron) {
        Ok(s) => s,
        Err(e) => {
            return Decision::Skip {
                reason: e.to_string(),
            }
        }
    };

    // Never fired: arm the trigger from now rather than firing immediately or
    // backfilling from the epoch. The caller persists `now` as the baseline.
    let Some(last) = last_fired else {
        return Decision::Skip {
            reason: "no previous fire recorded — scheduling from now".to_string(),
        };
    };

    let elapsed: Vec<DateTime<Utc>> = schedule
        .after(&last)
        .take(MAX_MISSED_SCANNED)
        .take_while(|t| *t <= now)
        .collect();

    let Some(latest) = elapsed.last().copied() else {
        return Decision::NotDue;
    };

    // "On time" is decided by the most recent occurrence, never by how many
    // elapsed: a second- or minute-level cron legitimately produces several
    // occurrences per tick and is not behind — they coalesce into this one
    // run. Only a latest occurrence outside the grace window means the machine
    // was actually away.
    if now - latest <= Duration::seconds(ON_TIME_GRACE_SECS) {
        return Decision::Fire {
            scheduled_for: latest,
        };
    }

    match trig.missed {
        // A laptop closed for a week must not wake up and run seven deploys.
        MissedPolicy::Skip => Decision::Skip {
            reason: format!("{} missed occurrence(s) skipped", elapsed.len()),
        },
        MissedPolicy::RunOnce => Decision::Fire {
            scheduled_for: latest,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    fn nightly(missed: MissedPolicy) -> ScheduleTrigger {
        ScheduleTrigger {
            id: "nightly".to_string(),
            enabled: true,
            cron: "0 3 * * *".to_string(),
            missed,
            pipeline_file: None,
            environment: None,
            stages: Vec::new(),
        }
    }

    #[test]
    fn test_parse_cron_accepts_five_field_form() {
        let next = next_due("0 3 * * *", at("2024-05-01T00:00:00Z")).unwrap();
        assert_eq!(next, Some(at("2024-05-01T03:00:00Z")));
    }

    #[test]
    fn test_parse_cron_accepts_six_field_form() {
        let next = next_due("30 0 3 * * *", at("2024-05-01T00:00:00Z")).unwrap();
        assert_eq!(next, Some(at("2024-05-01T03:00:30Z")));
    }

    #[test]
    fn test_parse_cron_rejects_garbage() {
        assert!(parse_cron("").is_err());
        assert!(parse_cron("not a cron").is_err());
        assert!(parse_cron("0 3 * *").is_err());
    }

    #[test]
    fn test_next_run_times_lists_consecutive_fires() {
        let times = next_run_times("0 3 * * *", at("2024-05-01T00:00:00Z"), 3).unwrap();
        assert_eq!(
            times,
            vec![
                at("2024-05-01T03:00:00Z"),
                at("2024-05-02T03:00:00Z"),
                at("2024-05-03T03:00:00Z"),
            ]
        );
    }

    #[test]
    fn test_due_now_is_not_due_before_the_next_occurrence() {
        let decision = due_now(
            &nightly(MissedPolicy::Skip),
            Some(at("2024-05-01T03:00:00Z")),
            at("2024-05-01T12:00:00Z"),
        );
        assert_eq!(decision, Decision::NotDue);
    }

    #[test]
    fn test_due_now_fires_on_a_fresh_occurrence() {
        let decision = due_now(
            &nightly(MissedPolicy::Skip),
            Some(at("2024-05-01T03:00:00Z")),
            at("2024-05-02T03:00:20Z"),
        );
        assert_eq!(
            decision,
            Decision::Fire {
                scheduled_for: at("2024-05-02T03:00:00Z")
            }
        );
    }

    #[test]
    fn test_due_now_skips_a_week_of_missed_runs_by_default() {
        let decision = due_now(
            &nightly(MissedPolicy::Skip),
            Some(at("2024-05-01T03:00:00Z")),
            at("2024-05-08T09:00:00Z"),
        );
        match decision {
            Decision::Skip { reason } => assert!(reason.contains("7 missed"), "{reason}"),
            other => panic!("expected Skip, got {other:?}"),
        }
    }

    #[test]
    fn test_due_now_run_once_fires_exactly_once_for_a_week_of_misses() {
        let decision = due_now(
            &nightly(MissedPolicy::RunOnce),
            Some(at("2024-05-01T03:00:00Z")),
            at("2024-05-08T09:00:00Z"),
        );
        assert_eq!(
            decision,
            Decision::Fire {
                scheduled_for: at("2024-05-08T03:00:00Z")
            }
        );
    }

    /// A single occurrence that went stale (machine asleep past 3am) is a miss,
    /// not an on-time fire: a nightly deploy must not land at noon.
    #[test]
    fn test_due_now_treats_a_stale_single_occurrence_as_missed() {
        let decision = due_now(
            &nightly(MissedPolicy::Skip),
            Some(at("2024-05-01T04:00:00Z")),
            at("2024-05-02T12:00:00Z"),
        );
        match decision {
            Decision::Skip { reason } => assert!(reason.contains("1 missed"), "{reason}"),
            other => panic!("expected Skip, got {other:?}"),
        }
    }

    /// A second-level cron produces several occurrences inside one 30s tick.
    /// Those coalesce into one on-time run — they are not a missed backlog,
    /// which under the default policy would mean it could never fire at all.
    #[test]
    fn test_due_now_fires_a_frequent_cron_despite_several_occurrences() {
        let mut trig = nightly(MissedPolicy::Skip);
        trig.cron = "*/30 * * * * *".to_string();

        let decision = due_now(
            &trig,
            Some(at("2024-05-01T03:00:00Z")),
            at("2024-05-01T03:01:00Z"),
        );

        assert_eq!(
            decision,
            Decision::Fire {
                scheduled_for: at("2024-05-01T03:01:00Z")
            }
        );
    }

    #[test]
    fn test_due_now_arms_a_trigger_that_never_fired() {
        let decision = due_now(
            &nightly(MissedPolicy::Skip),
            None,
            at("2024-05-01T12:00:00Z"),
        );
        match decision {
            Decision::Skip { reason } => assert!(reason.contains("no previous fire"), "{reason}"),
            other => panic!("expected Skip, got {other:?}"),
        }
    }

    #[test]
    fn test_due_now_ignores_disabled_triggers() {
        let mut trig = nightly(MissedPolicy::RunOnce);
        trig.enabled = false;

        let decision = due_now(
            &trig,
            Some(at("2024-05-01T03:00:00Z")),
            at("2024-05-08T09:00:00Z"),
        );

        assert_eq!(decision, Decision::NotDue);
    }

    #[test]
    fn test_due_now_reports_an_unparseable_cron_as_a_skip() {
        let mut trig = nightly(MissedPolicy::Skip);
        trig.cron = "nope".to_string();

        match due_now(
            &trig,
            Some(at("2024-05-01T03:00:00Z")),
            at("2024-05-02T03:00:00Z"),
        ) {
            Decision::Skip { reason } => assert!(reason.contains("'nope'"), "{reason}"),
            other => panic!("expected Skip, got {other:?}"),
        }
    }
}
