use crate::auth;
use crate::config;
use reqwest::Client;
use reqwest::RequestBuilder;
use std::time::Duration;

/// Build a reqwest client with default timeout.
pub fn build_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("reqwest client")
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

/// Validate base URL, API key, and JWT by issuing a GET to the base URL with both headers.
pub async fn validate_connection() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cfg = config::load_config().map_err(|e| format!("Config: {}", e))?;

    let api_key = cfg.api_key.as_deref().ok_or_else(|| {
        "RESMATE_API_KEY is not set. Set it in the environment or in ~/.resmate/config.yaml (or path in RESMATE_CONFIG)."
    })?;

    if api_key.is_empty() {
        return Err("RESMATE_API_KEY is empty.".into());
    }

    let client = build_client();
    let url = cfg.base_url.trim_end_matches('/').to_string();
    let req = client.get(&url);
    let req = add_auth_headers(req, api_key)?;
    let res = req.send().await?;

    let status = res.status();
    if status.is_success() {
        println!(
            "OK: {} ({}). Base URL, API key, and JWT are valid.",
            url, status
        );
        return Ok(());
    }

    if status.as_u16() == 401 {
        return Err("Invalid API key, JWT, or unauthorized (401). Check RESMATE_API_KEY and RESMATE_SECRET.".into());
    }

    let body = res.text().await.unwrap_or_default();
    Err(format!(
        "Validation failed: {} {}. {}",
        status.as_u16(),
        status.canonical_reason().unwrap_or(""),
        if body.is_empty() {
            "No response body."
        } else {
            body.as_str()
        }
    )
    .into())
}
