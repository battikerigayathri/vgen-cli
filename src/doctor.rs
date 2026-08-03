use crate::config::{self, ENV_API_KEY};
use crate::http_client;
use crate::workspace::{Workspace, WorkspaceError};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CheckResult {
    pub id: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DoctorReport {
    pub checks: Vec<CheckResult>,
}

impl DoctorReport {
    pub fn has_failures(&self) -> bool {
        self.checks.iter().any(|c| c.status == "fail")
    }
}

pub async fn run_checks(start: &Path, offline: bool) -> DoctorReport {
    let mut checks = Vec::new();

    let workspace = match Workspace::detect(start) {
        Ok(ws) => {
            checks.push(CheckResult {
                id: "workspace_root".to_string(),
                status: "pass".to_string(),
                message: format!("Workspace root: {}", ws.root.display()),
            });
            Some(ws)
        }
        Err(WorkspaceError::NotFound(msg)) => {
            checks.push(CheckResult {
                id: "workspace_root".to_string(),
                status: "fail".to_string(),
                message: msg,
            });
            None
        }
        Err(WorkspaceError::Io(e)) => {
            checks.push(CheckResult {
                id: "workspace_root".to_string(),
                status: "fail".to_string(),
                message: e.to_string(),
            });
            None
        }
    };

    match config::load_config() {
        Ok(cfg) => {
            checks.push(CheckResult {
                id: "config_load".to_string(),
                status: "pass".to_string(),
                message: format!("Config loaded (base_url: {})", cfg.base_url),
            });

            let api_key_ok = cfg.api_key.as_ref().map(|k| !k.is_empty()).unwrap_or(false);
            checks.push(CheckResult {
                id: "api_key_set".to_string(),
                status: if api_key_ok { "pass" } else { "fail" }.to_string(),
                message: if api_key_ok {
                    format!("{} is set", ENV_API_KEY)
                } else {
                    format!(
                        "{} is not set. Set it in the environment or in ~/.resmate/config.yaml.",
                        ENV_API_KEY
                    )
                },
            });
        }
        Err(e) => {
            checks.push(CheckResult {
                id: "config_load".to_string(),
                status: "fail".to_string(),
                message: e,
            });
            checks.push(CheckResult {
                id: "api_key_set".to_string(),
                status: "fail".to_string(),
                message: "Cannot check API key because config failed to load".to_string(),
            });
        }
    }

    let jwt_ok = std::env::var("RESMATE_SECRET")
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    checks.push(CheckResult {
        id: "jwt_secret".to_string(),
        status: if jwt_ok { "pass" } else { "warn" }.to_string(),
        message: if jwt_ok {
            "RESMATE_SECRET is set".to_string()
        } else {
            "RESMATE_SECRET is not set; JWT auth may fail.".to_string()
        },
    });

    if offline {
        checks.push(CheckResult {
            id: "connectivity".to_string(),
            status: "skip".to_string(),
            message: "Skipped (--offline)".to_string(),
        });
    } else {
        match http_client::validate_connection_result().await {
            Ok(data) => {
                checks.push(CheckResult {
                    id: "connectivity".to_string(),
                    status: "pass".to_string(),
                    message: format!("{} (HTTP {})", data.message, data.status),
                });
            }
            Err(err) => {
                checks.push(CheckResult {
                    id: "connectivity".to_string(),
                    status: "fail".to_string(),
                    message: err.message,
                });
            }
        }
    }

    if let Some(ws) = workspace {
        let artifact_dirs = [
            ("tools", &ws.tools_dir),
            ("agents", &ws.agents_dir),
            ("assistants", &ws.assistants_dir),
            ("hitl", &ws.hitl_dir),
            ("workflows", &ws.workflows_dir),
        ];

        let missing: Vec<String> = artifact_dirs
            .iter()
            .filter(|(_, path)| !path.is_dir())
            .map(|(name, path)| format!("{} ({})", name, path.display()))
            .collect();

        checks.push(CheckResult {
            id: "artifact_dirs_exist".to_string(),
            status: if missing.is_empty() {
                "pass".to_string()
            } else {
                "warn".to_string()
            },
            message: if missing.is_empty() {
                "All artifact directories exist".to_string()
            } else {
                format!("Missing artifact directories: {}", missing.join(", "))
            },
        });
    } else {
        checks.push(CheckResult {
            id: "artifact_dirs_exist".to_string(),
            status: "fail".to_string(),
            message: "Cannot check artifact directories without a workspace root".to_string(),
        });
    }

    DoctorReport { checks }
}
