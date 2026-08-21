use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct EnvMapping {
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    pub version: u32,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub dirs: Option<ManifestDirs>,
    #[serde(default)]
    pub default_assistant: Option<String>,
    #[serde(default)]
    pub recipe: Option<String>,
    #[serde(default)]
    pub platform: Option<ManifestPlatform>,
    #[serde(default)]
    pub env_mappings: Option<Vec<EnvMapping>>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ManifestDirs {
    #[serde(default)]
    pub tools: Option<String>,
    #[serde(default)]
    pub agents: Option<String>,
    #[serde(default)]
    pub assistants: Option<String>,
    #[serde(default)]
    pub hitl: Option<String>,
    #[serde(default)]
    pub workflows: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ManifestPlatform {
    #[serde(default)]
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ManifestInfo {
    pub present: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipe: Option<String>,
}

use serde::Serialize;

pub const MANIFEST_FILE: &str = "vgen.yaml";

pub fn manifest_path(root: &Path) -> PathBuf {
    root.join(MANIFEST_FILE)
}

pub fn load(root: &Path) -> Option<Manifest> {
    let path = manifest_path(root);
    if !path.is_file() {
        return None;
    }
    let content = fs::read_to_string(&path).ok()?;
    serde_yaml::from_str(&content).ok()
}

pub fn manifest_info(root: &Path) -> ManifestInfo {
    match load(root) {
        Some(m) => ManifestInfo {
            present: true,
            name: Some(m.name),
            recipe: m.recipe,
        },
        None => ManifestInfo {
            present: false,
            name: None,
            recipe: None,
        },
    }
}

pub fn resolve_dir(
    root: &Path,
    env_var: &str,
    manifest_dir: Option<&str>,
    default_name: &str,
) -> PathBuf {
    if let Ok(v) = std::env::var(env_var) {
        if !v.is_empty() {
            return PathBuf::from(v);
        }
    }
    if let Some(rel) = manifest_dir {
        let path = PathBuf::from(rel);
        if path.is_absolute() {
            return path;
        }
        return root.join(rel);
    }
    root.join(default_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        ENV_AGENTS_DIR, ENV_ASSISTANTS_DIR, ENV_HITL_DIR, ENV_TOOLS_DIR, ENV_WORKFLOWS_DIR,
    };
    use std::sync::{Mutex, OnceLock};

    fn lock_test_env() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn manifest_paths_override_defaults() {
        let _guard = lock_test_env();
        let base = std::env::temp_dir().join(format!("vgen-manifest-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        fs::write(
            manifest_path(&base),
            "version: 1\nname: test-project\ndirs:\n  tools: my-tools\n",
        )
        .unwrap();

        std::env::remove_var(ENV_TOOLS_DIR);
        let tools = resolve_dir(&base, ENV_TOOLS_DIR, Some("my-tools"), "tools");
        assert_eq!(tools, base.join("my-tools"));

        std::env::set_var(ENV_TOOLS_DIR, "/override/tools");
        let tools = resolve_dir(&base, ENV_TOOLS_DIR, Some("my-tools"), "tools");
        assert_eq!(tools, PathBuf::from("/override/tools"));
        std::env::remove_var(ENV_TOOLS_DIR);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn env_beats_manifest_for_all_dirs() {
        let _guard = lock_test_env();
        let base =
            std::env::temp_dir().join(format!("vgen-manifest-env-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        fs::write(
            manifest_path(&base),
            "version: 1\nname: x\ndirs:\n  agents: a\n  assistants: b\n  hitl: c\n  workflows: d\n",
        )
        .unwrap();

        std::env::set_var(ENV_AGENTS_DIR, "/env/agents");
        std::env::set_var(ENV_ASSISTANTS_DIR, "/env/assistants");
        std::env::set_var(ENV_HITL_DIR, "/env/hitl");
        std::env::set_var(ENV_WORKFLOWS_DIR, "/env/workflows");

        assert_eq!(
            resolve_dir(&base, ENV_AGENTS_DIR, Some("a"), "agents"),
            PathBuf::from("/env/agents")
        );

        std::env::remove_var(ENV_AGENTS_DIR);
        std::env::remove_var(ENV_ASSISTANTS_DIR);
        std::env::remove_var(ENV_HITL_DIR);
        std::env::remove_var(ENV_WORKFLOWS_DIR);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_parse_env_mappings() {
        let _guard = lock_test_env();
        let base = std::env::temp_dir().join(format!(
            "vgen-manifest-env-mappings-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        fs::write(
            manifest_path(&base),
            "version: 1\nname: test-env-mappings\nenv_mappings:\n  - source: LOCAL_ROC_AUTH_TOKEN\n    target: ROC_AUTH_${assigned_agent}\n",
        )
        .unwrap();

        let m = load(&base).unwrap();
        assert_eq!(m.name, "test-env-mappings");
        let mappings = m.env_mappings.unwrap();
        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings[0].source, "LOCAL_ROC_AUTH_TOKEN");
        assert_eq!(mappings[0].target, "ROC_AUTH_${assigned_agent}");

        let _ = fs::remove_dir_all(&base);
    }
}
