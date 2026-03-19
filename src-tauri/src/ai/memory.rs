use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Memory types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    ProjectPattern,
    FailurePattern,
    UserPreference,
    EnvironmentFact,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub key: String,
    pub value: String,
    pub memory_type: MemoryType,
    pub created_at: DateTime<Utc>,
    pub project_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Memory store — file-based JSON persistence
// ---------------------------------------------------------------------------

pub struct MemoryStore {
    base_dir: PathBuf,
}

impl MemoryStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            base_dir: data_dir.join("agent-memory"),
        }
    }

    /// Load all memories for a specific project.
    pub fn load_project_memories(&self, project_id: &str) -> Result<Vec<MemoryEntry>> {
        let path = self.project_memory_path(project_id);
        load_memories_from_file(&path)
    }

    /// Load global memories (cross-project).
    pub fn load_global_memories(&self) -> Result<Vec<MemoryEntry>> {
        let path = self.base_dir.join("global/learned-patterns.json");
        load_memories_from_file(&path)
    }

    /// Load all memories relevant to a project (project-specific + global).
    pub fn load_all_for_project(&self, project_id: &str) -> Result<Vec<MemoryEntry>> {
        let mut memories = self.load_global_memories().unwrap_or_default();
        memories.extend(self.load_project_memories(project_id).unwrap_or_default());
        Ok(memories)
    }

    /// Save a memory entry. Routes to project or global store based on project_id.
    pub fn save_memory(&self, entry: &MemoryEntry) -> Result<()> {
        let path = match &entry.project_id {
            Some(pid) => self.project_memory_path(pid),
            None => self.base_dir.join("global/learned-patterns.json"),
        };

        let mut memories = load_memories_from_file(&path).unwrap_or_default();

        // Upsert: replace existing entry with same key, or append
        if let Some(existing) = memories.iter_mut().find(|m| m.key == entry.key) {
            existing.value = entry.value.clone();
            existing.memory_type = entry.memory_type.clone();
            existing.created_at = entry.created_at;
        } else {
            memories.push(entry.clone());
        }

        save_memories_to_file(&path, &memories)
    }

    /// Delete a memory entry by key.
    pub fn delete_memory(&self, key: &str, project_id: Option<&str>) -> Result<()> {
        let path = match project_id {
            Some(pid) => self.project_memory_path(pid),
            None => self.base_dir.join("global/learned-patterns.json"),
        };

        let mut memories = load_memories_from_file(&path).unwrap_or_default();
        memories.retain(|m| m.key != key);
        save_memories_to_file(&path, &memories)
    }

    /// Get all memories (project + global) for listing.
    pub fn list_memories(&self, project_id: Option<&str>) -> Result<Vec<MemoryEntry>> {
        match project_id {
            Some(pid) => self.load_all_for_project(pid),
            None => self.load_global_memories(),
        }
    }

    fn project_memory_path(&self, project_id: &str) -> PathBuf {
        // Sanitize project_id for filesystem safety
        let safe_id: String = project_id
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        self.base_dir.join(format!("projects/{}/memory.json", safe_id))
    }
}

// ---------------------------------------------------------------------------
// Memory extraction from agent responses
// ---------------------------------------------------------------------------

/// Extract `[REMEMBER: key | value]` markers from an agent response.
pub fn extract_memories(response: &str, project_id: Option<&str>) -> Vec<MemoryEntry> {
    let re = Regex::new(r"\[REMEMBER:\s*([a-z0-9_]{1,64})\s*\|\s*(.{1,512}?)\s*\]").unwrap();
    let mut memories = Vec::new();
    let mut count = 0;

    for cap in re.captures_iter(response) {
        if count >= 5 {
            break; // Max 5 per response
        }

        let key = cap[1].to_string();
        let value = cap[2].to_string();

        // Classify the memory type based on key patterns
        let memory_type = classify_memory_type(&key, &value);

        memories.push(MemoryEntry {
            key,
            value,
            memory_type,
            created_at: Utc::now(),
            project_id: project_id.map(|s| s.to_string()),
        });

        count += 1;
    }

    memories
}

fn classify_memory_type(key: &str, _value: &str) -> MemoryType {
    if key.contains("fail") || key.contains("error") || key.contains("flaky") {
        MemoryType::FailurePattern
    } else if key.contains("prefer") || key.contains("style") || key.contains("user") {
        MemoryType::UserPreference
    } else if key.contains("env") || key.contains("server") || key.contains("host") {
        MemoryType::EnvironmentFact
    } else {
        MemoryType::ProjectPattern
    }
}

// ---------------------------------------------------------------------------
// File I/O helpers
// ---------------------------------------------------------------------------

fn load_memories_from_file(path: &Path) -> Result<Vec<MemoryEntry>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read memory file: {}", path.display()))?;

    let memories: Vec<MemoryEntry> = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse memory file: {}", path.display()))?;

    Ok(memories)
}

fn save_memories_to_file(path: &Path, memories: &[MemoryEntry]) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create memory directory: {}", parent.display()))?;
    }

    let content = serde_json::to_string_pretty(memories)
        .context("Failed to serialize memories")?;

    std::fs::write(path, content)
        .with_context(|| format!("Failed to write memory file: {}", path.display()))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_memories_basic() {
        let response = "The build failed because of a missing dependency.\n\
            [REMEMBER: package_manager | yarn]\n\
            [REMEMBER: node_version | 20]\n\
            You should run `yarn install` first.";

        let memories = extract_memories(response, Some("proj-1"));
        assert_eq!(memories.len(), 2);
        assert_eq!(memories[0].key, "package_manager");
        assert_eq!(memories[0].value, "yarn");
        assert_eq!(memories[1].key, "node_version");
        assert_eq!(memories[1].value, "20");
    }

    #[test]
    fn test_extract_memories_max_five() {
        let response = "[REMEMBER: a | 1]\n\
            [REMEMBER: b | 2]\n\
            [REMEMBER: c | 3]\n\
            [REMEMBER: d | 4]\n\
            [REMEMBER: e | 5]\n\
            [REMEMBER: f | 6]\n\
            [REMEMBER: g | 7]";

        let memories = extract_memories(response, None);
        assert_eq!(memories.len(), 5);
    }

    #[test]
    fn test_extract_memories_invalid_key() {
        let response = "[REMEMBER: INVALID_KEY | value]\n\
            [REMEMBER: valid_key | value]";

        let memories = extract_memories(response, None);
        // INVALID_KEY has uppercase, regex requires lowercase
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].key, "valid_key");
    }

    #[test]
    fn test_classify_memory_type() {
        assert_eq!(
            classify_memory_type("failure_pattern", ""),
            MemoryType::FailurePattern
        );
        assert_eq!(
            classify_memory_type("user_preference", ""),
            MemoryType::UserPreference
        );
        assert_eq!(
            classify_memory_type("server_host", ""),
            MemoryType::EnvironmentFact
        );
        assert_eq!(
            classify_memory_type("package_manager", ""),
            MemoryType::ProjectPattern
        );
    }
}
