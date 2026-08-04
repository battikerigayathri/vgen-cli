use crate::config;
use crate::http_client;
use serde_json::{json, Value};

fn extract_id_from_response(body: &Value) -> Option<String> {
    // Prefer data._id.$oid (Mongo-style response)
    body.get("data")
        .and_then(|d| d.get("_id"))
        .and_then(|id| id.get("$oid"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| body.get("id").and_then(|v| v.as_str()).map(String::from))
        .or_else(|| {
            body.get("data")
                .and_then(|d| d.get("id"))
                .and_then(|v| v.as_str())
                .map(String::from)
        })
}

/// POST /agent/create with JSON body. Returns the created agent id.
pub async fn create_agent(
    body: &Value,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let cfg = config::load_config().map_err(|e| e.to_string())?;
    let api_key = cfg.api_key.as_deref().ok_or("RESMATE_API_KEY is not set")?;
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{}/agent/create", base);

    let client = http_client::build_client();
    let req = client
        .post(&url)
        .json(body)
        .header("Content-Type", "application/json");
    let req = http_client::add_auth_headers(req, api_key)?;
    let res = req.send().await?;

    let status = res.status();
    let body: Value = res.json().await?;

    if !status.is_success() {
        return Err(format!("agent/create failed: {} {}", status, body.to_string()).into());
    }

    extract_id_from_response(&body).ok_or_else(|| "Response did not contain id".into())
}

/// POST /update-record with collectionName: agentConfig, recordId, document. Returns the agent id.
pub async fn update_agent(
    record_id: &str,
    document: &Value,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let cfg = config::load_config().map_err(|e| e.to_string())?;
    let api_key = cfg.api_key.as_deref().ok_or("RESMATE_API_KEY is not set")?;
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{}/update-record", base);

    let body = json!({
        "collectionName": "agentConfig",
        "recordId": record_id,
        "document": document
    });

    let client = http_client::build_client();
    let req = client
        .post(&url)
        .json(&body)
        .header("Content-Type", "application/json");
    let req = http_client::add_auth_headers(req, api_key)?;
    let res = req.send().await?;

    let status = res.status();
    let res_body: Value = res.json().await?;

    if !status.is_success() {
        return Err(format!(
            "update-record (agent) failed: {} {}",
            status,
            res_body.to_string()
        )
        .into());
    }

    Ok(extract_id_from_response(&res_body).unwrap_or_else(|| record_id.to_string()))
}
