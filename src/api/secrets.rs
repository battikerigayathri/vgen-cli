use crate::config;
use crate::http_client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SecretServiceType {
    #[serde(rename = "aws_secrets_manager", alias = "AwsSecretsManager")]
    AwsSecretsManager,
    #[serde(rename = "local", alias = "Local")]
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretKeyValue {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSecretsRequest {
    #[serde(rename = "serviceType")]
    pub service_type: SecretServiceType,
    pub description: Option<String>,
    pub tags: Option<HashMap<String, String>>,
    pub secret: Vec<SecretKeyValue>,
}

/// POST /set-secrets to securely push mapped secrets to Smriti.
pub async fn push_secrets(
    request: &SetSecretsRequest,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cfg = config::load_config().map_err(|e| e.to_string())?;
    let api_key = cfg.api_key.as_deref().ok_or("VGEN_API_KEY is not set")?;
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{}/set-secrets", base);

    let client = http_client::build_client();
    let mut req = client
        .post(&url)
        .json(request)
        .header("Content-Type", "application/json");
    req = http_client::add_auth_headers(req, api_key)?;
    if let Some(roc) = &cfg.roc_session {
        req = req.header("roc-session", roc.as_str());
    }
    let res = req.send().await?;

    let status = res.status();
    if !status.is_success() {
        let body_text = res.text().await.unwrap_or_default();
        return Err(format!("push secrets failed: {} - {}", status, body_text).into());
    }

    Ok(())
}
