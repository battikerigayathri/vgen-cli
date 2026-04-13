use crate::config;
use crate::http_client;
use serde_json::{json, Value};

fn extract_id_from_response(body: &Value) -> Option<String> {
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

/// POST /create-record with collectionName: hitlConfig, payload. Returns the created HITL record id.
pub async fn create_hitl(
    payload: &Value,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let cfg = config::load_config().map_err(|e| e.to_string())?;
    let api_key = cfg
        .api_key
        .as_deref()
        .ok_or("RESMATE_API_KEY is not set")?;
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{}/create-record", base);

    let body = json!({
        "collectionName": "hitlConfig",
        "payload": payload
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
            "create-record (HITL) failed: {} {}",
            status,
            res_body.to_string()
        )
        .into());
    }

    extract_id_from_response(&res_body).ok_or_else(|| "Response did not contain id".into())
}

/// POST /update-record with collectionName: hitlConfig, recordId, document. Returns the HITL record id.
pub async fn update_hitl(
    record_id: &str,
    document: &Value,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let cfg = config::load_config().map_err(|e| e.to_string())?;
    let api_key = cfg
        .api_key
        .as_deref()
        .ok_or("RESMATE_API_KEY is not set")?;
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{}/update-record", base);

    let body = json!({
        "collectionName": "hitlConfig",
        "recordId": record_id,
        "document": document
    });

    let client = http_client::build_client();
    let req = client.post(&url).json(&body).header("Content-Type", "application/json");
    let req = http_client::add_auth_headers(req, api_key)?;
    let res = req.send().await?;

    let status = res.status();
    let res_body: Value = res.json().await?;

    if !status.is_success() {
        return Err(format!(
            "update-record (HITL) failed: {} {}",
            status,
            res_body.to_string()
        )
        .into());
    }

    Ok(extract_id_from_response(&res_body).unwrap_or_else(|| record_id.to_string()))
}
