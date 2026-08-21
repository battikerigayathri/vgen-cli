use crate::manifest::{self, Manifest};
use crate::output::{emit_error, emit_success, CliContext, CliExitCode, OutputMode};
use serde::Serialize;
use std::process::ExitCode;

use serde::Deserialize;
use std::env;
use std::path::{Path, PathBuf};

const DEFAULT_BASE_URL: &str = "https://api-dev.ai.resmed.com";
pub const ENV_BASE_URL: &str = "VGEN_BASE_URL";
pub const ENV_API_KEY: &str = "VGEN_API_KEY";
pub const ENV_ROC_SESSION: &str = "VGEN_ROC_SESSION";
pub const ENV_TOOLS_DIR: &str = "VGEN_TOOLS_DIR";
pub const ENV_AGENTS_DIR: &str = "VGEN_AGENTS_DIR";
pub const ENV_ASSISTANTS_DIR: &str = "VGEN_ASSISTANTS_DIR";
pub const ENV_HITL_DIR: &str = "VGEN_HITL_DIR";
pub const ENV_WORKFLOWS_DIR: &str = "VGEN_WORKFLOWS_DIR";
const ENV_CONFIG: &str = "VGEN_CONFIG";

pub const ENV_CONFIG_VAR: &str = ENV_CONFIG;

/// Artifact directory paths resolved from workspace root and `VGEN_*_DIR` overrides.
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedPaths {
    pub tools_dir: PathBuf,
    pub agents_dir: PathBuf,
    pub assistants_dir: PathBuf,
    pub hitl_dir: PathBuf,
    pub workflows_dir: PathBuf,
}

impl ResolvedPaths {
    pub fn resolve(root: &Path) -> Self {
        Self::resolve_with_manifest(root, manifest::load(root).as_ref())
    }

    pub fn resolve_with_manifest(root: &Path, manifest: Option<&Manifest>) -> Self {
        let dirs = manifest.and_then(|m| m.dirs.as_ref());
        Self {
            tools_dir: manifest::resolve_dir(
                root,
                ENV_TOOLS_DIR,
                dirs.and_then(|d| d.tools.as_deref()),
                "tools",
            ),
            agents_dir: manifest::resolve_dir(
                root,
                ENV_AGENTS_DIR,
                dirs.and_then(|d| d.agents.as_deref()),
                "agents",
            ),
            assistants_dir: manifest::resolve_dir(
                root,
                ENV_ASSISTANTS_DIR,
                dirs.and_then(|d| d.assistants.as_deref()),
                "assistants",
            ),
            hitl_dir: manifest::resolve_dir(
                root,
                ENV_HITL_DIR,
                dirs.and_then(|d| d.hitl.as_deref()),
                "hitl",
            ),
            workflows_dir: manifest::resolve_dir(
                root,
                ENV_WORKFLOWS_DIR,
                dirs.and_then(|d| d.workflows.as_deref()),
                "workflows",
            ),
        }
    }
}

fn resolve_artifact_dir(root: &Path, env_var: &str, default_name: &str) -> PathBuf {
    env::var(env_var)
        .map(PathBuf::from)
        .unwrap_or_else(|_| root.join(default_name))
}

/// Configuration loaded from env and optional config file.
#[derive(Debug, Clone)]
pub struct ResMateConfig {
    pub base_url: String,
    pub api_key: Option<String>,
    pub roc_session: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConfigFile {
    base_url: Option<String>,
    api_key: Option<String>,
    roc_session: Option<String>,
}

fn config_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".vgen"))
}

pub fn config_file_path() -> Option<PathBuf> {
    env::var(ENV_CONFIG)
        .ok()
        .map(PathBuf::from)
        .or_else(|| config_dir().map(|d| d.join("config.yaml")))
}

/// Load config from optional file and env (env overrides file).
pub fn load_config() -> Result<ResMateConfig, String> {
    let mut base_url = None::<String>;
    let mut api_key = None::<String>;
    let mut roc_session = None::<String>;

    if let Some(path) = config_file_path() {
        if path.exists() {
            let contents = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read config file {}: {}", path.display(), e))?;
            let file: ConfigFile = serde_yaml::from_str(&contents)
                .map_err(|e| format!("Invalid YAML in config file {}: {}", path.display(), e))?;
            base_url = file.base_url;
            api_key = file.api_key;
            roc_session = file.roc_session;
        }
    }

    if let Ok(v) = env::var(ENV_BASE_URL) {
        base_url = Some(v);
    }
    if let Ok(v) = env::var(ENV_API_KEY) {
        api_key = Some(v);
    }
    if let Ok(v) = env::var(ENV_ROC_SESSION) {
        roc_session = Some(v);
    }

    Ok(ResMateConfig {
        base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
        api_key,
        roc_session,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigSummary {
    pub base_url: String,
    pub api_key: RedactedField,
    pub roc_session: RedactedField,
    pub config_file: Option<String>,
    pub env_vars: EnvVarNames,
}

#[derive(Debug, Clone, Serialize)]
pub struct RedactedField {
    pub set: bool,
    pub display: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnvVarNames {
    pub base_url: String,
    pub api_key: String,
    pub roc_session: String,
}

fn redact_api_key(key: &Option<String>) -> RedactedField {
    match key {
        None => RedactedField {
            set: false,
            display: "(not set)".to_string(),
        },
        Some(k) if k.is_empty() => RedactedField {
            set: false,
            display: "(not set)".to_string(),
        },
        Some(k) => RedactedField {
            set: true,
            display: format!("{}***", &k[..k.len().min(4)]),
        },
    }
}

fn redact_roc_session(session: &Option<String>) -> RedactedField {
    match session {
        None => RedactedField {
            set: false,
            display: "(not set)".to_string(),
        },
        Some(s) if s.is_empty() => RedactedField {
            set: false,
            display: "(not set)".to_string(),
        },
        Some(_) => RedactedField {
            set: true,
            display: "(set)".to_string(),
        },
    }
}

/// Build a JSON-serializable config summary with redacted secrets.
pub fn build_config_summary() -> Result<ConfigSummary, String> {
    let cfg = load_config()?;
    Ok(ConfigSummary {
        base_url: cfg.base_url,
        api_key: redact_api_key(&cfg.api_key),
        roc_session: redact_roc_session(&cfg.roc_session),
        config_file: config_file_path().map(|p| p.display().to_string()),
        env_vars: EnvVarNames {
            base_url: ENV_BASE_URL.to_string(),
            api_key: ENV_API_KEY.to_string(),
            roc_session: ENV_ROC_SESSION.to_string(),
        },
    })
}

fn print_human_config(summary: &ConfigSummary) {
    println!("base_url: {}", summary.base_url);
    println!("api_key: {}", summary.api_key.display);
    println!("roc_session: {}", summary.roc_session.display);
    println!(
        "\nConfig file: {}",
        summary.config_file.as_deref().unwrap_or("none")
    );
    println!(
        "Env: {} (base URL), {} (API key), {} (roc-session)",
        summary.env_vars.base_url, summary.env_vars.api_key, summary.env_vars.roc_session
    );
}

/// Show config with JSON envelope support.
pub fn run_config_show(ctx: &CliContext) -> ExitCode {
    match build_config_summary() {
        Ok(summary) => {
            match ctx.mode {
                OutputMode::Human => print_human_config(&summary),
                OutputMode::Json => emit_success(ctx, summary),
            }
            CliExitCode::Success.into()
        }
        Err(e) => emit_error(
            ctx,
            "CONFIG_LOAD_FAILED",
            e,
            None,
            CliExitCode::RuntimeError,
        ),
    }
}
