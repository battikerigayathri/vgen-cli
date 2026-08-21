use crate::auth;
use crate::config;
use crate::output::{emit_error, emit_success, CliContext, CliExitCode, OutputMode};
use reqwest::Client;
use reqwest::RequestBuilder;
use serde::Serialize;
use serde_json::Value;
use std::process::ExitCode;
use std::time::Duration;

/// Build a reqwest client with default timeout.
pub fn build_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("reqwest client")
}

/// Build a long-timeout reqwest client for heavy orchestration loops.
pub fn build_long_timeout_client(timeout: Duration) -> Client {
    Client::builder()
        .timeout(timeout)
        .build()
        .expect("Failed to initialize long-timeout reqwest client")
}

/// Add x-api-key and Authorization: Bearer <jwt> to a request. Generates a fresh JWT per call.
pub fn add_auth_headers(
    request: RequestBuilder,
    api_key: &str,
) -> Result<RequestBuilder, Box<dyn std::error::Error + Send + Sync>> {
    let jwt = auth::build_jwt(60).map_err(|e| e.to_string())?;
    Ok(request
        .header("x-api-key", api_key)
        .header("Authorization", format!("Bearer {}", jwt)))
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidateConnectionData {
    pub url: String,
    pub status: u16,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ValidateConnectionError {
    pub code: &'static str,
    pub message: String,
    pub details: Option<Value>,
}

/// Validate connectivity and return structured result for JSON envelope.
pub async fn validate_connection_result() -> Result<ValidateConnectionData, ValidateConnectionError>
{
    let cfg = config::load_config().map_err(|e| ValidateConnectionError {
        code: "CONFIG_LOAD_FAILED",
        message: e,
        details: None,
    })?;

    let api_key = cfg.api_key.as_deref().ok_or_else(|| ValidateConnectionError {
        code: "CONFIG_MISSING_API_KEY",
        message: "VGEN_API_KEY is not set. Set it in the environment or in ~/.vgen/config.yaml (or path in VGEN_CONFIG).".to_string(),
        details: None,
    })?;

    if api_key.is_empty() {
        return Err(ValidateConnectionError {
            code: "CONFIG_MISSING_API_KEY",
            message: "VGEN_API_KEY is empty.".to_string(),
            details: None,
        });
    }

    let client = build_client();
    let url = cfg.base_url.trim_end_matches('/').to_string();
    let req = client.get(&url);
    let req = add_auth_headers(req, api_key).map_err(|e| ValidateConnectionError {
        code: "CONNECTIVITY_FAILED",
        message: e.to_string(),
        details: None,
    })?;

    let res = req.send().await.map_err(|e| ValidateConnectionError {
        code: "CONNECTIVITY_FAILED",
        message: format!("Request failed: {}", e),
        details: Some(serde_json::json!({ "url": url })),
    })?;

    let status = res.status();
    if status.is_success() {
        return Ok(ValidateConnectionData {
            url: url.clone(),
            status: status.as_u16(),
            message: "Base URL, API key, and JWT are valid.".to_string(),
        });
    }

    if status.as_u16() == 401 {
        return Err(ValidateConnectionError {
            code: "CONNECTIVITY_FAILED",
            message: "Invalid API key, JWT, or unauthorized (401). Check VGEN_API_KEY and VGEN_SECRET.".to_string(),
            details: Some(serde_json::json!({ "url": url, "status": 401 })),
        });
    }

    let body = res.text().await.unwrap_or_default();
    Err(ValidateConnectionError {
        code: "CONNECTIVITY_FAILED",
        message: format!(
            "Validation failed: {} {}. {}",
            status.as_u16(),
            status.canonical_reason().unwrap_or(""),
            if body.is_empty() {
                "No response body."
            } else {
                body.as_str()
            }
        ),
        details: Some(serde_json::json!({
            "url": url,
            "status": status.as_u16(),
        })),
    })
}

/// Validate connection with JSON envelope support.
pub async fn run_config_validate(ctx: &CliContext) -> ExitCode {
    match validate_connection_result().await {
        Ok(data) => {
            match ctx.mode {
                OutputMode::Human => {
                    println!("OK: {} ({}). {}", data.url, data.status, data.message);
                }
                OutputMode::Json => emit_success(ctx, data),
            }
            CliExitCode::Success.into()
        }
        Err(err) => emit_error(
            ctx,
            err.code,
            err.message,
            err.details,
            CliExitCode::RuntimeError,
        ),
    }
}
