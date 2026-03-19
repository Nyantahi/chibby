use anyhow::{Context, Result};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use super::prompts;

const MAX_FILE_SIZE: usize = 8 * 1024; // 8KB per file
const MAX_TOTAL_SIZE: usize = 256 * 1024; // 256KB total budget

/// Injection markers to scan for (case-insensitive).
const INJECTION_MARKERS: &[&str] = &[
    "ignore all previous",
    "ignore your instructions",
    "system prompt override",
    "disregard your programming",
    "new instructions:",
    "you are now",
    "forget everything",
    "override your",
    "bypass your",
    "act as if",
];

/// Identity files for the Chibby agent.
#[derive(Debug, Clone)]
pub struct AgentIdentity {
    pub identity: String,
    pub tools: String,
    pub bootstrap: String,
}

/// Shared directive files applied to all agents.
#[derive(Debug, Clone)]
pub struct SharedDirectives {
    pub security: String,
    pub output_format: String,
    pub memory_instruction: String,
    pub cicd_knowledge: String,
}

/// Registry that loads and manages agent identity files.
#[derive(Debug, Clone)]
pub struct AgentIdentityRegistry {
    pub agent: AgentIdentity,
    pub shared: SharedDirectives,
    base_path: Option<PathBuf>,
    checksum: u64,
}

impl AgentIdentityRegistry {
    /// Load identity files from the given base directory.
    /// Falls back to inline prompts for any missing file.
    pub fn load_from_dir(base_path: &Path) -> Result<Self> {
        let mut total_size: usize = 0;

        let agent = AgentIdentity {
            identity: load_file_or_fallback(
                &base_path.join("chibby/identity.md"),
                prompts::FALLBACK_IDENTITY,
                &mut total_size,
            )?,
            tools: load_file_or_fallback(
                &base_path.join("chibby/tools.md"),
                prompts::FALLBACK_TOOLS,
                &mut total_size,
            )?,
            bootstrap: load_file_or_fallback(
                &base_path.join("chibby/bootstrap.md"),
                prompts::FALLBACK_BOOTSTRAP,
                &mut total_size,
            )?,
        };

        let shared = SharedDirectives {
            security: load_file_or_fallback(
                &base_path.join("_shared/security.md"),
                prompts::FALLBACK_SECURITY,
                &mut total_size,
            )?,
            output_format: load_file_or_fallback(
                &base_path.join("_shared/output-format.md"),
                prompts::FALLBACK_OUTPUT_FORMAT,
                &mut total_size,
            )?,
            memory_instruction: load_file_or_fallback(
                &base_path.join("_shared/memory-instruction.md"),
                prompts::FALLBACK_MEMORY_INSTRUCTION,
                &mut total_size,
            )?,
            cicd_knowledge: load_file_or_fallback(
                &base_path.join("_shared/cicd-knowledge.md"),
                prompts::FALLBACK_CICD_KNOWLEDGE,
                &mut total_size,
            )?,
        };

        if total_size > MAX_TOTAL_SIZE {
            anyhow::bail!(
                "Total identity file size ({} bytes) exceeds budget ({} bytes)",
                total_size,
                MAX_TOTAL_SIZE
            );
        }

        let checksum = compute_checksum(&agent, &shared);

        Ok(Self {
            agent,
            shared,
            base_path: Some(base_path.to_path_buf()),
            checksum,
        })
    }

    /// Load from inline fallback prompts only (no files).
    pub fn load_fallback() -> Self {
        let agent = AgentIdentity {
            identity: prompts::FALLBACK_IDENTITY.to_string(),
            tools: prompts::FALLBACK_TOOLS.to_string(),
            bootstrap: prompts::FALLBACK_BOOTSTRAP.to_string(),
        };
        let shared = SharedDirectives {
            security: prompts::FALLBACK_SECURITY.to_string(),
            output_format: prompts::FALLBACK_OUTPUT_FORMAT.to_string(),
            memory_instruction: prompts::FALLBACK_MEMORY_INSTRUCTION.to_string(),
            cicd_knowledge: prompts::FALLBACK_CICD_KNOWLEDGE.to_string(),
        };
        let checksum = compute_checksum(&agent, &shared);
        Self {
            agent,
            shared,
            base_path: None,
            checksum,
        }
    }

    /// Assemble the full system prompt for the agent.
    /// Order: identity → security → cicd-knowledge → tools → output-format → memory-instruction
    /// Optionally appends bootstrap content on first run.
    pub fn assemble_prompt(&self, is_first_run: bool) -> String {
        let mut parts = Vec::with_capacity(7);

        parts.push(self.agent.identity.as_str());
        parts.push(self.shared.security.as_str());
        parts.push(self.shared.cicd_knowledge.as_str());
        parts.push(self.agent.tools.as_str());
        parts.push(self.shared.output_format.as_str());
        parts.push(self.shared.memory_instruction.as_str());

        if is_first_run {
            parts.push(self.agent.bootstrap.as_str());
        }

        parts.join("\n\n---\n\n")
    }

    /// Hot-reload identity files if they have changed (dev mode only).
    /// Returns true if files were reloaded.
    #[cfg(debug_assertions)]
    pub fn hot_reload_if_changed(&mut self) -> Result<bool> {
        let base_path = match &self.base_path {
            Some(p) => p.clone(),
            None => return Ok(false),
        };

        let reloaded = Self::load_from_dir(&base_path)?;
        if reloaded.checksum != self.checksum {
            log::info!("Identity files changed, hot-reloading");
            *self = reloaded;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

/// Resolve the identity files base path.
/// Resolution chain: source tree (dev) → executable-relative (prod) → None (fallback).
pub fn resolve_identity_path() -> Option<PathBuf> {
    // Dev mode: look in source tree relative to the crate root
    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/agent-identities");
    if dev_path.exists() {
        log::info!("Using dev identity path: {}", dev_path.display());
        return Some(dev_path);
    }

    // Prod mode: look relative to the executable (Tauri bundles resources alongside)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            // macOS: Contents/Resources/agent-identities
            let macos_resource = exe_dir
                .parent()
                .unwrap_or(exe_dir)
                .join("Resources/agent-identities");
            if macos_resource.exists() {
                log::info!("Using bundled identity path: {}", macos_resource.display());
                return Some(macos_resource);
            }

            // Linux/Windows: alongside the executable
            let sibling = exe_dir.join("agent-identities");
            if sibling.exists() {
                log::info!("Using bundled identity path: {}", sibling.display());
                return Some(sibling);
            }
        }
    }

    log::warn!("No identity files found, will use inline fallbacks");
    None
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn load_file_or_fallback(path: &Path, fallback: &str, total_size: &mut usize) -> Result<String> {
    if !path.exists() {
        log::debug!("Identity file not found, using fallback: {}", path.display());
        return Ok(fallback.to_string());
    }

    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read identity file: {}", path.display()))?;

    if content.len() > MAX_FILE_SIZE {
        anyhow::bail!(
            "Identity file {} exceeds max size ({} > {} bytes)",
            path.display(),
            content.len(),
            MAX_FILE_SIZE
        );
    }

    // Scan for injection markers
    let lower = content.to_lowercase();
    for marker in INJECTION_MARKERS {
        if lower.contains(marker) {
            anyhow::bail!(
                "Identity file {} contains suspected injection marker: '{}'",
                path.display(),
                marker
            );
        }
    }

    *total_size += content.len();
    Ok(content)
}

fn compute_checksum(agent: &AgentIdentity, shared: &SharedDirectives) -> u64 {
    let mut hasher = DefaultHasher::new();
    agent.identity.hash(&mut hasher);
    agent.tools.hash(&mut hasher);
    agent.bootstrap.hash(&mut hasher);
    shared.security.hash(&mut hasher);
    shared.output_format.hash(&mut hasher);
    shared.memory_instruction.hash(&mut hasher);
    shared.cicd_knowledge.hash(&mut hasher);
    hasher.finish()
}
