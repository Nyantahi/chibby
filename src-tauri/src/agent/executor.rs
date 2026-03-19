use serde::{Deserialize, Serialize};

use crate::engine::models::RunStatus;

/// Tracks deploy approval state for an agent-driven pipeline execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentExecution {
    pub run_id: String,
    pub pipeline_name: String,
    pub project_path: String,
    pub stages_completed: Vec<String>,
    pub current_stage: Option<String>,
    pub status: AgentExecutionStatus,
    pub approval_pending: Option<PendingApproval>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentExecutionStatus {
    Running,
    AwaitingApproval,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingApproval {
    pub stage_name: String,
    pub stage_index: usize,
    pub reason: String,
}

impl AgentExecution {
    pub fn new(run_id: String, pipeline_name: String, project_path: String) -> Self {
        Self {
            run_id,
            pipeline_name,
            project_path,
            stages_completed: Vec::new(),
            current_stage: None,
            status: AgentExecutionStatus::Running,
            approval_pending: None,
        }
    }

    /// Check if a stage name looks like a deploy stage.
    pub fn is_deploy_stage(stage_name: &str) -> bool {
        let lower = stage_name.to_lowercase();
        lower.contains("deploy")
            || lower.contains("release")
            || lower.contains("publish")
            || lower.contains("upload")
            || lower.contains("push")
    }

    /// Mark a stage as completed.
    pub fn complete_stage(&mut self, stage_name: &str) {
        self.stages_completed.push(stage_name.to_string());
        self.current_stage = None;
    }

    /// Set a pending approval for a deploy stage.
    pub fn request_approval(&mut self, stage_name: &str, stage_index: usize) {
        self.status = AgentExecutionStatus::AwaitingApproval;
        self.approval_pending = Some(PendingApproval {
            stage_name: stage_name.to_string(),
            stage_index,
            reason: format!(
                "Stage '{}' looks like a deployment. Approve to continue.",
                stage_name
            ),
        });
    }

    /// Resume after approval.
    pub fn approve(&mut self) {
        self.status = AgentExecutionStatus::Running;
        self.approval_pending = None;
    }

    /// Reject the pending deploy — abort or skip.
    pub fn reject(&mut self) {
        self.status = AgentExecutionStatus::Cancelled;
        self.approval_pending = None;
    }

    /// Mark execution as completed.
    pub fn finish(&mut self, success: bool) {
        self.status = if success {
            AgentExecutionStatus::Completed
        } else {
            AgentExecutionStatus::Failed
        };
        self.current_stage = None;
    }

    /// Convert to the public ExecutionResult type.
    pub fn to_result(&self) -> super::ExecutionResult {
        super::ExecutionResult {
            run_id: self.run_id.clone(),
            status: match self.status {
                AgentExecutionStatus::Running => RunStatus::Running,
                AgentExecutionStatus::AwaitingApproval => RunStatus::Running,
                AgentExecutionStatus::Completed => RunStatus::Success,
                AgentExecutionStatus::Failed => RunStatus::Failed,
                AgentExecutionStatus::Cancelled => RunStatus::Cancelled,
            },
            stages_completed: self.stages_completed.clone(),
            approval_required: self.approval_pending.as_ref().map(|p| p.stage_name.clone()),
        }
    }
}
