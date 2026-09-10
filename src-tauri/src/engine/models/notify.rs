//! Notification configuration and payload types.

#[allow(unused_imports)]
use super::*;
use serde::{Deserialize, Serialize};

/// Notification channel type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NotifyChannel {
    Desktop,
    Webhook,
}

/// When to send notifications.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NotifyOn {
    Success,
    Failure,
    Always,
}

impl Default for NotifyOn {
    fn default() -> Self {
        Self::Always
    }
}

/// A single notification target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotifyTarget {
    /// Channel type.
    pub channel: NotifyChannel,
    /// Webhook URL (required for Webhook channel).
    #[serde(default)]
    pub url: Option<String>,
    /// When to fire this notification.
    #[serde(default)]
    pub on: NotifyOn,
}

fn default_true() -> bool {
    true
}

/// How much noise an unattended run (scheduled or file-watch) is allowed to make.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnattendedNotify {
    /// Send on failure even when notifications are otherwise off — nobody is
    /// watching the screen, so a silent failure is a lost failure.
    #[serde(default = "default_true")]
    pub always_on_failure: bool,
    /// Also announce successful unattended runs.
    #[serde(default)]
    pub on_success: bool,
}

impl Default for UnattendedNotify {
    fn default() -> Self {
        Self {
            always_on_failure: true,
            on_success: false,
        }
    }
}

/// Notification configuration (stored in .chibby/notify.toml).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotifyConfig {
    /// Whether notifications are enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Notification targets.
    #[serde(default)]
    pub targets: Vec<NotifyTarget>,
    /// Escalation policy for runs no human is watching.
    #[serde(default)]
    pub unattended: UnattendedNotify,
}

impl Default for NotifyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            targets: Vec::new(),
            unattended: UnattendedNotify::default(),
        }
    }
}

/// Payload sent with a notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotifyPayload {
    pub project: String,
    pub version: Option<String>,
    pub environment: Option<String>,
    pub status: RunStatus,
    pub duration_ms: Option<u64>,
    pub message: String,
    /// How the automatic rollback for this run turned out, when one ran.
    /// Additive: existing webhook consumers ignore it.
    #[serde(default)]
    pub rollback: Option<RollbackOutcome>,
    /// What started the run, when it was not a person.
    #[serde(default)]
    pub run_kind: Option<RunKind>,
    /// The trigger that started it (`scheduled:nightly`, `hook:pre-push`, ...).
    #[serde(default)]
    pub trigger_id: Option<String>,
}
