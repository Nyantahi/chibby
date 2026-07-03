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

/// Notification configuration (stored in .chibby/notify.toml).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotifyConfig {
    /// Whether notifications are enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Notification targets.
    #[serde(default)]
    pub targets: Vec<NotifyTarget>,
}

impl Default for NotifyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            targets: Vec::new(),
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
}
