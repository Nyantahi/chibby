//! Deploy health: what is actually live in each environment, right now.
//!
//! A "deployment" is a run that named an environment and succeeded. Rollback
//! runs are deliberately *not* deployments: a rollback restores an older
//! release, so counting it as the current deployment would report the wrong
//! commit as live. Rollbacks surface through `last_rollback_at` /
//! `last_rollback_outcome` instead.
//!
//! Unlike the trend numbers this ignores the report window: "what is live on
//! production" is a fact about the newest successful deploy, however old.

use crate::engine::models::{RollbackOutcome, RunKind, RunStatus};
use crate::engine::run_index::RunSummary;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One project × environment row of the deploy matrix.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentStatus {
    pub repo_path: String,
    pub project_name: String,
    pub environment: String,
    /// Newest successful deployment — what is live.
    pub current_run_id: Option<String>,
    pub commit: Option<String>,
    pub branch: Option<String>,
    pub deployed_at: Option<DateTime<Utc>>,
    /// Deploys have failed since the live one, so this environment is behind
    /// what the team thinks is shipped.
    pub is_stale: bool,
    pub failed_since: u32,
    pub last_rollback_at: Option<DateTime<Utc>>,
    pub last_rollback_outcome: Option<RollbackOutcome>,
}

/// Whether a run counts as an attempt to deploy an environment.
fn is_deploy_attempt(summary: &RunSummary) -> bool {
    summary.environment.is_some() && summary.run_kind != RunKind::Rollback
}

/// Build the project × environment matrix. Projects with no
/// environment-scoped runs contribute no rows.
pub fn environment_matrix(
    summaries: &[RunSummary],
    project_name: impl Fn(&str) -> String,
) -> Vec<EnvironmentStatus> {
    // BTreeMap so the matrix comes out in a stable, readable order.
    let mut grouped: BTreeMap<(String, String), Vec<&RunSummary>> = BTreeMap::new();
    for summary in summaries.iter().filter(|s| s.environment.is_some()) {
        let key = (
            summary.repo_path.clone(),
            summary.environment.clone().unwrap_or_default(),
        );
        grouped.entry(key).or_default().push(summary);
    }

    grouped
        .into_iter()
        .map(|((repo_path, environment), mut runs)| {
            runs.sort_by(|a, b| b.started_at.cmp(&a.started_at));
            let project_name = project_name(&repo_path);
            status_for(repo_path, project_name, environment, &runs)
        })
        .collect()
}

/// Collapse one environment's runs (newest first) into its current status.
fn status_for(
    repo_path: String,
    project_name: String,
    environment: String,
    runs: &[&RunSummary],
) -> EnvironmentStatus {
    let current = runs
        .iter()
        .find(|s| s.status == RunStatus::Success && is_deploy_attempt(s));

    // Failures newer than the live deployment: three of these on production
    // is the number worth showing red.
    let failed_since = runs
        .iter()
        .take_while(|s| Some(&s.id) != current.map(|c| &c.id))
        .filter(|s| s.status == RunStatus::Failed && is_deploy_attempt(s))
        .count() as u32;

    let last_rollback = runs.iter().find(|s| s.rollback_outcome.is_some());

    EnvironmentStatus {
        repo_path,
        project_name,
        environment,
        current_run_id: current.map(|s| s.id.clone()),
        commit: current.and_then(|s| s.commit.clone()),
        branch: current.and_then(|s| s.branch.clone()),
        deployed_at: current.map(|s| s.started_at),
        is_stale: failed_since > 0,
        failed_since,
        last_rollback_at: last_rollback.map(|s| s.started_at),
        last_rollback_outcome: last_rollback.and_then(|s| s.rollback_outcome),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::insights::tests_support::{deployment, summary};

    fn matrix(summaries: &[RunSummary]) -> Vec<EnvironmentStatus> {
        environment_matrix(summaries, |path| path.to_string())
    }

    #[test]
    fn test_newest_success_per_environment_wins() {
        let rows = matrix(&[
            deployment("old", "prod", RunStatus::Success, 60),
            deployment("new", "prod", RunStatus::Success, 5),
            deployment("staging", "staging", RunStatus::Success, 90),
        ]);

        assert_eq!(rows.len(), 2);
        let prod = rows.iter().find(|r| r.environment == "prod").unwrap();
        assert_eq!(prod.current_run_id.as_deref(), Some("new"));
        assert!(!prod.is_stale);
        assert_eq!(prod.failed_since, 0);
    }

    #[test]
    fn test_failures_after_the_last_success_mark_the_environment_stale() {
        let rows = matrix(&[
            deployment("live", "prod", RunStatus::Success, 60),
            deployment("f1", "prod", RunStatus::Failed, 30),
            deployment("f2", "prod", RunStatus::Failed, 20),
            deployment("f3", "prod", RunStatus::Failed, 10),
        ]);

        let prod = &rows[0];
        assert_eq!(prod.current_run_id.as_deref(), Some("live"));
        assert!(prod.is_stale);
        assert_eq!(prod.failed_since, 3);
    }

    #[test]
    fn test_a_rollback_run_is_not_the_current_deployment() {
        let mut rollback = deployment("rb", "prod", RunStatus::Success, 5);
        rollback.run_kind = crate::engine::models::RunKind::Rollback;
        let mut failed = deployment("bad", "prod", RunStatus::Failed, 10);
        failed.rollback_outcome = Some(RollbackOutcome::Succeeded);

        let rows = matrix(&[
            deployment("live", "prod", RunStatus::Success, 60),
            failed,
            rollback,
        ]);

        let prod = &rows[0];
        assert_eq!(prod.current_run_id.as_deref(), Some("live"));
        assert_eq!(prod.failed_since, 1);
        assert_eq!(prod.last_rollback_outcome, Some(RollbackOutcome::Succeeded));
        assert!(prod.last_rollback_at.is_some());
    }

    #[test]
    fn test_runs_without_an_environment_yield_no_rows() {
        let rows = matrix(&[
            summary("a", RunStatus::Success, 1),
            summary("b", RunStatus::Failed, 2),
        ]);

        assert!(rows.is_empty());
    }

    #[test]
    fn test_environment_with_only_failures_has_no_current_deployment() {
        let rows = matrix(&[deployment("f1", "prod", RunStatus::Failed, 5)]);

        let prod = &rows[0];
        assert!(prod.current_run_id.is_none());
        assert!(prod.commit.is_none());
        assert_eq!(prod.failed_since, 1);
        assert!(prod.is_stale);
    }
}
