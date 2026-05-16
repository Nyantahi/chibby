use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

use crate::agent::{
    AgentAnalysis, AgentResponse, ChibbyAgent, GeneratedPipeline, PipelineFormat,
    context::AnalysisContext,
};
use crate::ai::identity_loader::{AgentIdentityRegistry, resolve_identity_path};
use crate::ai::memory::{self, MemoryEntry, MemoryStore};
use crate::ai::provider;
use crate::engine::{audit, persistence, pipeline};

/// Maximum allowed length for a chat message (in bytes).
const MAX_CHAT_MESSAGE_LEN: usize = 8192;
/// Maximum allowed length for project info sent with pipeline generation.
const MAX_PROJECT_INFO_LEN: usize = 16384;

// ---------------------------------------------------------------------------
// Agent state — managed by Tauri
// ---------------------------------------------------------------------------

pub struct AgentState {
    agent: Option<ChibbyAgent>,
    memory_store: MemoryStore,
    first_run: bool,
}

impl AgentState {
    pub fn new() -> Self {
        let data_dir = persistence::data_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let memory_store = MemoryStore::new(&data_dir);

        // Try to build the agent (may fail if no API keys configured)
        let agent = build_agent().ok();

        Self {
            agent,
            memory_store,
            first_run: true,
        }
    }

    /// Rebuild the agent (e.g., after API keys change).
    pub fn rebuild_agent(&mut self) {
        self.agent = build_agent().ok();
    }
}

pub type SharedAgentState = Arc<RwLock<AgentState>>;

pub fn create_agent_state() -> SharedAgentState {
    Arc::new(RwLock::new(AgentState::new()))
}

fn build_agent() -> anyhow::Result<ChibbyAgent> {
    let llm_provider = provider::build_provider()?;

    let identity = match resolve_identity_path() {
        Some(path) => AgentIdentityRegistry::load_from_dir(&path)?,
        None => AgentIdentityRegistry::load_fallback(),
    };

    Ok(ChibbyAgent::new(llm_provider, identity))
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
pub struct AgentSystemStatus {
    pub available: bool,
    pub has_anthropic_key: bool,
    pub has_openai_key: bool,
    pub error: Option<String>,
}

/// Get the current status of the agent system.
#[tauri::command]
pub async fn get_agent_status(
    state: State<'_, SharedAgentState>,
) -> Result<AgentSystemStatus, String> {
    let agent_state = state.read().await;
    Ok(AgentSystemStatus {
        available: agent_state.agent.is_some(),
        has_anthropic_key: crate::engine::app_settings::has_app_secret("anthropic"),
        has_openai_key: crate::engine::app_settings::has_app_secret("openai"),
        error: if agent_state.agent.is_none() {
            Some("No AI provider configured. Add an API key in Settings.".to_string())
        } else {
            None
        },
    })
}

/// Analyze a pipeline run — auto-detects the appropriate skill.
#[tauri::command]
pub async fn analyze_run(
    run_id: String,
    state: State<'_, SharedAgentState>,
) -> Result<AgentAnalysis, String> {
    let agent_state = state.read().await;
    let agent = agent_state
        .agent
        .as_ref()
        .ok_or("Agent not available. Configure an API key in Settings.")?;

    // Load the run from history
    let run = persistence::load_run(&run_id)
        .map_err(|e| format!("Failed to load run: {}", e))?
        .ok_or_else(|| format!("Run '{}' not found", run_id))?;

    // Load pipeline definition if available
    let pipeline = pipeline::load_pipeline(std::path::Path::new(&run.repo_path)).ok();

    // Build context
    let mut ctx = AnalysisContext::from_run(&run, pipeline.as_ref());

    // Load memories for this project
    ctx.memories = agent_state
        .memory_store
        .load_all_for_project(&run.repo_path)
        .unwrap_or_default()
        .into_iter()
        .map(|m| crate::agent::context::MemoryContext {
            key: m.key,
            value: m.value,
        })
        .collect();

    // Run analysis
    let analysis = agent
        .analyze(ctx)
        .await
        .map_err(|e| format!("Analysis failed: {}", e))?;

    Ok(analysis)
}

/// Chat with the agent about CI/CD topics.
#[tauri::command]
pub async fn agent_chat(
    message: String,
    project_id: Option<String>,
    run_id: Option<String>,
    state: State<'_, SharedAgentState>,
) -> Result<AgentResponse, String> {
    // Validate input length to prevent resource exhaustion and excessive API costs
    if message.trim().is_empty() {
        return Err("Message cannot be empty".to_string());
    }
    if message.len() > MAX_CHAT_MESSAGE_LEN {
        return Err(format!(
            "Message exceeds maximum length of {} characters",
            MAX_CHAT_MESSAGE_LEN
        ));
    }

    audit::log_event("agent_chat", &format!("project={:?} run={:?}", project_id, run_id));

    let mut agent_state = state.write().await;
    let is_first_run = agent_state.first_run;

    let agent = agent_state
        .agent
        .as_ref()
        .ok_or("Agent not available. Configure an API key in Settings.")?;

    // Build context
    let mut ctx = AnalysisContext::empty();

    if let Some(rid) = &run_id {
        if let Ok(Some(run)) = persistence::load_run(rid) {
            let pipeline = pipeline::load_pipeline(std::path::Path::new(&run.repo_path)).ok();
            ctx = AnalysisContext::from_run(&run, pipeline.as_ref());
        }
    }

    if let Some(pid) = &project_id {
        ctx.project_path = Some(pid.clone());
        ctx.memories = agent_state
            .memory_store
            .load_all_for_project(pid)
            .unwrap_or_default()
            .into_iter()
            .map(|m| crate::agent::context::MemoryContext {
                key: m.key,
                value: m.value,
            })
            .collect();
    }

    let response = agent
        .chat(&message, ctx, is_first_run)
        .await
        .map_err(|e| format!("Chat failed: {}", e))?;

    // Extract and save any memories from the response
    let memories = memory::extract_memories(&response.message, project_id.as_deref());
    for mem in &memories {
        let _ = agent_state.memory_store.save_memory(mem);
    }

    // No longer first run
    agent_state.first_run = false;

    Ok(response)
}

/// Generate a pipeline config for a project.
#[tauri::command]
pub async fn generate_pipeline_config(
    project_path: String,
    format: PipelineFormat,
    project_info: String,
    state: State<'_, SharedAgentState>,
) -> Result<GeneratedPipeline, String> {
    if project_info.len() > MAX_PROJECT_INFO_LEN {
        return Err(format!(
            "Project info exceeds maximum length of {} characters",
            MAX_PROJECT_INFO_LEN
        ));
    }

    audit::log_event(
        "generate_pipeline_config",
        &format!("project={}", project_path),
    );

    let agent_state = state.read().await;
    let agent = agent_state
        .agent
        .as_ref()
        .ok_or("Agent not available. Configure an API key in Settings.")?;

    agent
        .generate_pipeline(&project_path, format, &project_info)
        .await
        .map_err(|e| format!("Pipeline generation failed: {}", e))
}

/// Save a generated pipeline to disk.
///
/// Validates that `file_path` stays within the project directory to prevent
/// agent-generated content from writing to arbitrary locations.
#[tauri::command]
pub fn save_generated_pipeline(
    project_path: String,
    file_path: String,
    content: String,
) -> Result<(), String> {
    // Validate file_path: no absolute paths or traversal
    if file_path.contains("..") || file_path.starts_with('/') || file_path.starts_with('\\') {
        return Err("Invalid file path: must be a relative path within the project".to_string());
    }

    let full_path = std::path::Path::new(&project_path).join(&file_path);

    // Ensure the resolved path is still within the project directory
    let canonical_project = std::fs::canonicalize(&project_path)
        .map_err(|e| format!("Invalid project path: {}", e))?;
    // Create parent dirs first so canonicalize works on the target
    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }
    // For new files, check that the parent is within the project
    let canonical_parent = std::fs::canonicalize(
        full_path.parent().unwrap_or(std::path::Path::new(&project_path)),
    )
    .map_err(|e| format!("Failed to resolve path: {}", e))?;
    if !canonical_parent.starts_with(&canonical_project) {
        return Err("File path resolves outside the project directory".to_string());
    }

    audit::log_event(
        "save_generated_pipeline",
        &format!("project={} file={}", project_path, file_path),
    );

    std::fs::write(&full_path, &content)
        .map_err(|e| format!("Failed to write pipeline file: {}", e))?;

    Ok(())
}

/// Get agent memories for a project (or global).
#[tauri::command]
pub async fn get_agent_memories(
    project_id: Option<String>,
    state: State<'_, SharedAgentState>,
) -> Result<Vec<MemoryEntry>, String> {
    let agent_state = state.read().await;
    agent_state
        .memory_store
        .list_memories(project_id.as_deref())
        .map_err(|e| format!("Failed to load memories: {}", e))
}

/// Delete an agent memory by key.
#[tauri::command]
pub async fn delete_agent_memory(
    key: String,
    project_id: Option<String>,
    state: State<'_, SharedAgentState>,
) -> Result<(), String> {
    let agent_state = state.read().await;
    agent_state
        .memory_store
        .delete_memory(&key, project_id.as_deref())
        .map_err(|e| format!("Failed to delete memory: {}", e))
}

/// Rebuild the agent (e.g., after changing API keys in settings).
#[tauri::command]
pub async fn rebuild_agent(
    state: State<'_, SharedAgentState>,
) -> Result<AgentSystemStatus, String> {
    let mut agent_state = state.write().await;
    agent_state.rebuild_agent();

    Ok(AgentSystemStatus {
        available: agent_state.agent.is_some(),
        has_anthropic_key: crate::engine::app_settings::has_app_secret("anthropic"),
        has_openai_key: crate::engine::app_settings::has_app_secret("openai"),
        error: if agent_state.agent.is_none() {
            Some("No AI provider configured. Add an API key in Settings.".to_string())
        } else {
            None
        },
    })
}
