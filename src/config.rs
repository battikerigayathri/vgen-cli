use serde::Deserialize;
use std::env;
use std::path::PathBuf;

const DEFAULT_BASE_URL: &str = "https://api-dev.ai.resmed.com";
const ENV_BASE_URL: &str = "RESMATE_BASE_URL";
const ENV_API_KEY: &str = "RESMATE_API_KEY";
const ENV_ROC_SESSION: &str = "RESMATE_ROC_SESSION";
const ENV_CONFIG: &str = "RESMATE_CONFIG";

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
    dirs::home_dir().map(|h| h.join(".resmate"))
}

fn config_file_path() -> Option<PathBuf> {
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
            let contents = std::fs::read_to_string(&path).map_err(|e| {
                format!("Failed to read config file {}: {}", path.display(), e)
            })?;
            let file: ConfigFile = serde_yaml::from_str(&contents).map_err(|e| {
                format!("Invalid YAML in config file {}: {}", path.display(), e)
            })?;
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

/// Print current config (mask api_key and roc_session).
pub fn show_config() {
    match load_config() {
        Ok(cfg) => {
            println!("base_url: {}", cfg.base_url);
            println!(
                "api_key: {}",
                cfg.api_key
                    .as_ref()
                    .map(|k| if k.is_empty() {
                        "(not set)".to_string()
                    } else {
                        format!("{}***", &k[..k.len().min(4)])
                    })
                    .unwrap_or_else(|| "(not set)".to_string())
            );
            println!(
                "roc_session: {}",
                cfg.roc_session
                    .as_ref()
                    .map(|s| if s.is_empty() {
                        "(not set)".to_string()
                    } else {
                        "(set)".to_string()
                    })
                    .unwrap_or_else(|| "(not set)".to_string())
            );
            println!(
                "\nConfig file: {}",
                config_file_path()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "none".to_string())
            );
            println!(
                "Env: {} (base URL), {} (API key), {} (roc-session)",
                ENV_BASE_URL, ENV_API_KEY, ENV_ROC_SESSION
            );
        }
        Err(e) => {
            eprintln!("Error loading config: {}", e);
            std::process::exit(1);
        }
    }
}
