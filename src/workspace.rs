use crate::config::{
    build_config_summary, ConfigSummary, ResolvedPaths, ENV_AGENTS_DIR, ENV_API_KEY,
    ENV_ASSISTANTS_DIR, ENV_BASE_URL, ENV_CONFIG_VAR, ENV_HITL_DIR, ENV_ROC_SESSION, ENV_TOOLS_DIR,
    ENV_WORKFLOWS_DIR,
};
use crate::manifest::{self, ManifestInfo};
use serde::Serialize;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Workspace {
    pub root: PathBuf,
    pub tools_dir: PathBuf,
    pub agents_dir: PathBuf,
    pub assistants_dir: PathBuf,
    pub hitl_dir: PathBuf,
    pub workflows_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ArtifactCounts {
    pub tools: usize,
    pub agents: usize,
    pub assistants: usize,
    pub hitl: usize,
    pub workflows: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DirInfo {
    pub path: String,
    pub exists: bool,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EnvVarInfo {
    pub name: String,
    pub set: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EnvSummary {
    pub tools_dir: EnvVarInfo,
    pub agents_dir: EnvVarInfo,
    pub assistants_dir: EnvVarInfo,
    pub hitl_dir: EnvVarInfo,
    pub workflows_dir: EnvVarInfo,
    pub base_url: EnvVarInfo,
    pub api_key: EnvVarInfo,
    pub roc_session: EnvVarInfo,
    pub config: EnvVarInfo,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceInfo {
    pub root: String,
    pub detected: bool,
    pub manifest: ManifestInfo,
    pub artifact_dirs: HashMap<String, DirInfo>,
    pub counts: ArtifactCounts,
    pub env: EnvSummary,
    pub config: ConfigSummary,
}

#[derive(Debug)]
pub enum WorkspaceError {
    Io(std::io::Error),
    NotFound(String),
}

impl fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkspaceError::Io(e) => write!(f, "{}", e),
            WorkspaceError::NotFound(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for WorkspaceError {}

impl From<std::io::Error> for WorkspaceError {
    fn from(value: std::io::Error) -> Self {
        WorkspaceError::Io(value)
    }
}

impl Workspace {
    pub fn detect(start: &Path) -> Result<Self, WorkspaceError> {
        let start = if start.as_os_str().is_empty() {
            std::env::current_dir()?
        } else {
            start.to_path_buf()
        };

        let root = find_workspace_root(&start)?;
        let paths = ResolvedPaths::resolve(&root);
        Ok(Self {
            root,
            tools_dir: paths.tools_dir,
            agents_dir: paths.agents_dir,
            assistants_dir: paths.assistants_dir,
            hitl_dir: paths.hitl_dir,
            workflows_dir: paths.workflows_dir,
        })
    }

    pub fn info(&self) -> Result<WorkspaceInfo, WorkspaceError> {
        let counts = ArtifactCounts {
            tools: count_tools(&self.tools_dir),
            agents: count_agents(&self.agents_dir),
            assistants: count_assistants(&self.assistants_dir),
            hitl: count_hitl(&self.hitl_dir),
            workflows: count_workflows(&self.workflows_dir),
        };

        let detected = counts.tools > 0
            || counts.agents > 0
            || counts.assistants > 0
            || counts.hitl > 0
            || counts.workflows > 0;

        let mut artifact_dirs = HashMap::new();
        artifact_dirs.insert("tools".to_string(), dir_info(&self.tools_dir, counts.tools));
        artifact_dirs.insert(
            "agents".to_string(),
            dir_info(&self.agents_dir, counts.agents),
        );
        artifact_dirs.insert(
            "assistants".to_string(),
            dir_info(&self.assistants_dir, counts.assistants),
        );
        artifact_dirs.insert("hitl".to_string(), dir_info(&self.hitl_dir, counts.hitl));
        artifact_dirs.insert(
            "workflows".to_string(),
            dir_info(&self.workflows_dir, counts.workflows),
        );

        let config = build_config_summary().map_err(|e| WorkspaceError::NotFound(e))?;

        Ok(WorkspaceInfo {
            root: self.root.display().to_string(),
            detected,
            manifest: manifest::manifest_info(&self.root),
            artifact_dirs,
            counts,
            env: build_env_summary(),
            config,
        })
    }
}

fn find_workspace_root(start: &Path) -> Result<PathBuf, WorkspaceError> {
    let mut dir = if start.is_dir() {
        start.to_path_buf()
    } else {
        start
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| start.to_path_buf())
    };

    let mut candidate = None::<PathBuf>;
    loop {
        if is_workspace_root(&dir) {
            candidate = Some(dir.clone());
        }
        if !dir.pop() {
            break;
        }
    }

    candidate.ok_or_else(|| {
        WorkspaceError::NotFound(
            "No ResMate workspace detected: no artifact directories found from current directory"
                .to_string(),
        )
    })
}

fn is_workspace_root(root: &Path) -> bool {
    if manifest::manifest_path(root).is_file() {
        return true;
    }
    [
        root.join("tools"),
        root.join("agents"),
        root.join("assistants"),
        root.join("hitl"),
        root.join("workflows"),
    ]
    .iter()
    .any(|p| p.is_dir())
}

fn dir_info(path: &Path, count: usize) -> DirInfo {
    DirInfo {
        path: path.display().to_string(),
        exists: path.is_dir(),
        count,
    }
}

fn build_env_summary() -> EnvSummary {
    EnvSummary {
        tools_dir: env_var_info(ENV_TOOLS_DIR),
        agents_dir: env_var_info(ENV_AGENTS_DIR),
        assistants_dir: env_var_info(ENV_ASSISTANTS_DIR),
        hitl_dir: env_var_info(ENV_HITL_DIR),
        workflows_dir: env_var_info(ENV_WORKFLOWS_DIR),
        base_url: env_var_info(ENV_BASE_URL),
        api_key: env_var_info(ENV_API_KEY),
        roc_session: env_var_info(ENV_ROC_SESSION),
        config: env_var_info(ENV_CONFIG_VAR),
    }
}

fn env_var_info(name: &str) -> EnvVarInfo {
    let set = std::env::var(name).map(|v| !v.is_empty()).unwrap_or(false);
    EnvVarInfo {
        name: name.to_string(),
        set,
    }
}

fn count_tools(dir: &Path) -> usize {
    count_subdirs_matching(dir, is_tool_dir)
}

fn count_agents(dir: &Path) -> usize {
    count_yaml_files(dir)
}

fn count_assistants(dir: &Path) -> usize {
    count_yaml_files(dir)
}

fn count_hitl(dir: &Path) -> usize {
    count_subdirs_matching(dir, is_hitl_dir)
}

fn count_workflows(dir: &Path) -> usize {
    count_subdirs_matching(dir, is_workflow_dir)
}

fn count_subdirs_matching(dir: &Path, predicate: fn(&Path) -> bool) -> usize {
    if !dir.is_dir() {
        return 0;
    }
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir() && predicate(&e.path()))
                .count()
        })
        .unwrap_or(0)
}

fn count_yaml_files(dir: &Path) -> usize {
    if !dir.is_dir() {
        return 0;
    }
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let path = e.path();
                    if !path.is_file() {
                        return false;
                    }
                    matches!(
                        path.extension().and_then(|s| s.to_str()),
                        Some("yaml") | Some("yml")
                    )
                })
                .count()
        })
        .unwrap_or(0)
}

fn is_tool_dir(path: &Path) -> bool {
    ["tool.yaml", "tool.yml", "config.yaml", "config.yml"]
        .iter()
        .any(|name| path.join(name).is_file())
}

fn is_hitl_dir(path: &Path) -> bool {
    path.join("config.json").is_file()
        && (path.join("meta.yaml").is_file() || path.join("meta.yml").is_file())
}

fn is_workflow_dir(path: &Path) -> bool {
    path.join("workflow.json").is_file()
        || path.join("workflow.yaml").is_file()
        || path.join("workflow.yml").is_file()
        || path.join("meta.yaml").is_file()
        || path.join("meta.yml").is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::{Mutex, OnceLock};

    fn lock_test_env() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    fn write_minimal_workspace(root: &Path) {
        fs::create_dir_all(root.join("tools/greet")).unwrap();
        fs::write(root.join("tools/greet/tool.yaml"), "name: greet\n").unwrap();
        fs::create_dir_all(root.join("agents")).unwrap();
        fs::write(root.join("agents/greet-agent.yaml"), "name: agent\n").unwrap();
    }

    #[test]
    fn detect_finds_workspace_from_subdirectory() {
        let _guard = lock_test_env();
        let base = std::env::temp_dir().join(format!("vgen-ws-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        write_minimal_workspace(&base);
        fs::create_dir_all(base.join("nested/deep")).unwrap();

        let ws = Workspace::detect(&base.join("nested/deep")).expect("detect workspace");
        assert_eq!(ws.root, base);
        let info = ws.info().expect("workspace info");
        assert!(info.detected);
        assert_eq!(info.counts.tools, 1);
        assert_eq!(info.counts.agents, 1);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn honors_tools_dir_override() {
        let _guard = lock_test_env();
        let base = std::env::temp_dir().join(format!("vgen-ws-override-{}", std::process::id()));
        let custom_tools =
            std::env::temp_dir().join(format!("vgen-custom-tools-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let _ = fs::remove_dir_all(&custom_tools);

        fs::create_dir_all(custom_tools.join("custom-tool")).unwrap();
        fs::write(custom_tools.join("custom-tool/tool.yaml"), "name: custom\n").unwrap();
        fs::create_dir_all(base.join("agents")).unwrap();
        fs::write(base.join("agents/a.yaml"), "name: a\n").unwrap();

        std::env::set_var(ENV_TOOLS_DIR, &custom_tools);

        let ws = Workspace::detect(&base).expect("detect workspace");
        assert_eq!(ws.tools_dir, custom_tools);
        let info = ws.info().expect("workspace info");
        assert_eq!(info.counts.tools, 1);
        assert!(info.env.tools_dir.set);

        std::env::remove_var(ENV_TOOLS_DIR);
        let _ = fs::remove_dir_all(&base);
        let _ = fs::remove_dir_all(&custom_tools);
    }
}
