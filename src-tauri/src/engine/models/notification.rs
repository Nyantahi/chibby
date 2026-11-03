//! Inbox entries: the persistent record of what happened while you were away.

use super::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// What an inbox entry is about.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationKind {
    RunFinished,
    PreRunFailure,
    RunInterrupted,
    AgentApproval,
    AgentFinished,
    ReleaseAction,
}

/// How an inbox entry should be presented.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationSeverity {
    Success,
    Failure,
    Info,
    Attention,
}

/// One inbox entry, stored as `<data_dir>/notifications/<id>.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationEntry {
    /// Also the file stem; starts with creation millis so names sort by age.
    pub id: String,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub read_at: Option<DateTime<Utc>>,
    pub kind: NotificationKind,
    pub severity: NotificationSeverity,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub repo_path: Option<String>,
    #[serde(default)]
    pub project_name: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub run_kind: Option<RunKind>,
    #[serde(default)]
    pub trigger_id: Option<String>,
    #[serde(default)]
    pub pipeline_file: Option<String>,
    #[serde(default)]
    pub agent_session_id: Option<String>,
    #[serde(default)]
    pub agent_action_id: Option<String>,
    pub alert: AlertPolicy,
    /// True once no other process should raise this entry's OS alert.
    #[serde(default)]
    pub alert_delivered: bool,
}

impl NotificationEntry {
    pub fn new(
        kind: NotificationKind,
        severity: NotificationSeverity,
        title: String,
        body: String,
    ) -> Self {
        let created_at = Utc::now();
        Self {
            id: format!("{:013}-{}", created_at.timestamp_millis(), Uuid::new_v4()),
            created_at,
            read_at: None,
            kind,
            severity,
            title,
            body,
            repo_path: None,
            project_name: None,
            run_id: None,
            run_kind: None,
            trigger_id: None,
            pipeline_file: None,
            agent_session_id: None,
            agent_action_id: None,
            alert: AlertPolicy::None,
            alert_delivered: false,
        }
    }
}

/// Unread counts behind the badge.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NotificationSummary {
    pub unread: u32,
    pub unread_failures: u32,
}
