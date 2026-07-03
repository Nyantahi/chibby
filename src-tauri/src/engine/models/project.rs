//! Project registry type.

#[allow(unused_imports)]
use super::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A project tracked by Chibby.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub added_at: DateTime<Utc>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_run_status: Option<RunStatus>,
}

impl Project {
    /// Create a new project from a repo path.
    pub fn new(name: &str, path: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            path: path.to_string(),
            added_at: Utc::now(),
            last_run_at: None,
            last_run_status: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Pipeline Validation (pre-run checks)
// ---------------------------------------------------------------------------
