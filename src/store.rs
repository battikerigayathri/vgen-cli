use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

const ENV_IDS_FILE: &str = "vgen_IDS_FILE";

fn default_ids_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".vgen").join("ids.yaml"))
}

pub fn ids_file_path() -> Option<PathBuf> {
    env::var(ENV_IDS_FILE)
        .ok()
        .map(PathBuf::from)
        .or_else(default_ids_path)
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct IdStore {
    #[serde(default)]
    pub tools: HashMap<String, String>,
    #[serde(default)]
    pub agents: HashMap<String, String>,
    #[serde(default)]
    pub assistants: HashMap<String, String>,
}

impl IdStore {
    pub fn load() -> Result<IdStore, String> {
        let path = ids_file_path().ok_or("Could not resolve ids file path (no home dir)")?;
        if !path.exists() {
            return Ok(IdStore::default());
        }
        let contents = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        serde_yaml::from_str(&contents).map_err(|e| format!("Invalid YAML in {}: {}", path.display(), e))
    }

    pub fn save(&self) -> Result<(), String> {
        let path = ids_file_path().ok_or("Could not resolve ids file path (no home dir)")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
        }
        let contents = serde_yaml::to_string(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, contents).map_err(|e| format!("Failed to write {}: {}", path.display(), e))
    }

    pub fn get_tool_id(&self, slug: &str) -> Option<&str> {
        self.tools.get(slug).map(String::as_str)
    }

    pub fn get_agent_id(&self, slug: &str) -> Option<&str> {
        self.agents.get(slug).map(String::as_str)
    }

    pub fn get_assistant_id(&self, slug: &str) -> Option<&str> {
        self.assistants.get(slug).map(String::as_str)
    }

    pub fn set_tool_id(&mut self, slug: String, id: String) {
        self.tools.insert(slug, id);
    }

    pub fn set_agent_id(&mut self, slug: String, id: String) {
        self.agents.insert(slug, id);
    }

    pub fn set_assistant_id(&mut self, slug: String, id: String) {
        self.assistants.insert(slug, id);
    }
}
