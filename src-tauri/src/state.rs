use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Tracks running pipelines and their cancellation status
#[derive(Default)]
pub struct PipelineState {
    /// Maps repo_path to cancellation requested flag
    cancelled: HashMap<String, bool>,
    /// Maps repo_path to the currently running child process ID (if any)
    running_pids: HashMap<String, u32>,
}

impl PipelineState {
    pub fn new() -> Self {
        Self {
            cancelled: HashMap::new(),
            running_pids: HashMap::new(),
        }
    }

    /// Mark a pipeline as started (not cancelled)
    pub fn start(&mut self, repo_path: &str) {
        self.cancelled.insert(repo_path.to_string(), false);
        self.running_pids.remove(repo_path);
    }

    /// Request cancellation for a pipeline
    pub fn cancel(&mut self, repo_path: &str) {
        self.cancelled.insert(repo_path.to_string(), true);
    }

    /// Check if cancellation was requested
    pub fn is_cancelled(&self, repo_path: &str) -> bool {
        self.cancelled.get(repo_path).copied().unwrap_or(false)
    }

    /// Check if a pipeline is currently running (has an entry, not yet cleaned up)
    pub fn is_running(&self, repo_path: &str) -> bool {
        self.cancelled.contains_key(repo_path)
    }

    /// Clean up after pipeline completes
    pub fn cleanup(&mut self, repo_path: &str) {
        self.cancelled.remove(repo_path);
        self.running_pids.remove(repo_path);
    }

    /// Register a child process PID for a running pipeline
    pub fn set_running_pid(&mut self, repo_path: &str, pid: u32) {
        self.running_pids.insert(repo_path.to_string(), pid);
    }

    /// Get the currently running PID for a pipeline (if any)
    pub fn get_running_pid(&self, repo_path: &str) -> Option<u32> {
        self.running_pids.get(repo_path).copied()
    }

    /// Clear the running PID for a pipeline
    pub fn clear_running_pid(&mut self, repo_path: &str) {
        self.running_pids.remove(repo_path);
    }
}

/// Thread-safe wrapper for PipelineState
pub type SharedPipelineState = Arc<RwLock<PipelineState>>;

/// Create a new shared pipeline state
pub fn create_pipeline_state() -> SharedPipelineState {
    Arc::new(RwLock::new(PipelineState::new()))
}
