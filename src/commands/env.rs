use crate::api::{push_secrets, SecretKeyValue, SecretServiceType, SetSecretsRequest};
use crate::manifest;
use crate::output::{emit_error, emit_success, CliContext, CliExitCode, OutputMode};
use crate::workspace::Workspace;
use std::path::Path;
use std::process::ExitCode;

pub async fn run_env_push(
    ctx: &CliContext,
    service_type: &str,
    description: Option<String>,
) -> ExitCode {
    let ws = match Workspace::detect(Path::new("")) {
        Ok(w) => w,
        Err(e) => {
            return emit_error(
                ctx,
                "WORKSPACE_DETECT_FAILED",
                format!("Failed to detect workspace: {}", e),
                None,
                CliExitCode::RuntimeError,
            );
        }
    };

    // Load workspace-root aware .env file
    let dotenv_path = ws.root.join(".env");
    if dotenv_path.is_file() {
        if let Err(e) = dotenvy::from_path(&dotenv_path) {
            println!(
                "⚠️ Warning: Failed to load .env file at {}: {}",
                dotenv_path.display(),
                e
            );
        }
    } else if ctx.mode == OutputMode::Human {
        println!(
            "⚠️ Warning: No .env file found at workspace root ({}). Proceeding with system environment variables.",
            ws.root.display()
        );
    }

    // Load manifest
    let manifest = match manifest::load(&ws.root) {
        Some(m) => m,
        None => {
            return emit_error(
                ctx,
                "MANIFEST_LOAD_FAILED",
                "Failed to load vgen.yaml manifest.",
                None,
                CliExitCode::RuntimeError,
            );
        }
    };

    // Scan agents/ folder for active agent slugs
    let mut agent_slugs = Vec::new();
    if ws.agents_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&ws.agents_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                        if ext == "yaml" || ext == "yml" {
                            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                                agent_slugs.push(stem.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    // Compile secrets
    let mut compiled_secrets = Vec::new();
    let mut keys_changed = Vec::new();

    if let Some(mappings) = &manifest.env_mappings {
        for mapping in mappings {
            let val = match std::env::var(&mapping.source) {
                Ok(v) => v,
                Err(_) => {
                    if ctx.mode == OutputMode::Human {
                        println!(
                            "⚠️ Warning: Environment variable {} is not set in .env or system environment.",
                            mapping.source
                        );
                    }
                    continue;
                }
            };

            if mapping.target.contains("${assigned_agent}") {
                if agent_slugs.is_empty() {
                    if ctx.mode == OutputMode::Human {
                        println!(
                            "⚠️ Warning: Target contains ${{assigned_agent}} but no active agents were found in agents/."
                        );
                    }
                } else {
                    for slug in &agent_slugs {
                        let target_key = mapping.target.replace("${assigned_agent}", slug);
                        keys_changed.push((target_key.clone(), val.len()));
                        compiled_secrets.push(SecretKeyValue {
                            key: target_key,
                            value: val.clone(),
                        });
                    }
                }
            } else {
                keys_changed.push((mapping.target.clone(), val.len()));
                compiled_secrets.push(SecretKeyValue {
                    key: mapping.target.clone(),
                    value: val,
                });
            }
        }
    }

    if compiled_secrets.is_empty() {
        if ctx.mode == OutputMode::Human {
            println!("🔒 No secrets compiled from env_mappings.");
        }
        return CliExitCode::Success.into();
    }

    // Determine service type — Smriti has no local secret storage; AWS only.
    let svc_type = match service_type.to_lowercase().as_str() {
        "aws_secrets_manager" | "awssecretsmanager" => SecretServiceType::AwsSecretsManager,
        "local" => {
            return emit_error(
                ctx,
                "SECRETS_SERVICE_TYPE_UNSUPPORTED",
                "service-type 'local' is not supported. Use aws_secrets_manager (default).",
                None,
                CliExitCode::UsageError,
            );
        }
        other => {
            return emit_error(
                ctx,
                "SECRETS_SERVICE_TYPE_UNSUPPORTED",
                format!(
                    "Unsupported service-type '{}'. Use aws_secrets_manager.",
                    other
                ),
                None,
                CliExitCode::UsageError,
            );
        }
    };

    let request = SetSecretsRequest {
        service_type: svc_type,
        description,
        tags: None,
        secret: compiled_secrets,
    };

    // Push secrets to Smriti
    if let Err(e) = push_secrets(&request).await {
        return emit_error(
            ctx,
            "SECRETS_PUSH_FAILED",
            format!("Failed to push secrets to Smriti: {}", e),
            None,
            CliExitCode::RuntimeError,
        );
    }

    match ctx.mode {
        OutputMode::Human => {
            println!("🔒 Secret mapping compiled successfully.");
            println!(
                "Pushed {} secrets to Smriti ({:?} Storage manager):",
                keys_changed.len(),
                request.service_type
            );
            for (key, len) in &keys_changed {
                println!("  - [Success] Key: {} (Length: {})", key, len);
            }
            println!("Bulk deployment finalized. Values have been encrypted and stored.");
        }
        OutputMode::Json => {
            let json_keys: Vec<String> = keys_changed.into_iter().map(|(k, _)| k).collect();
            emit_success(ctx, serde_json::json!({ "pushed": json_keys }));
        }
    }

    CliExitCode::Success.into()
}
