use crate::config;
use crate::http_client;
use serde_json::{json, Value};

/// POST /get-record with collectionName and recordId. Returns the full response body (callers use response["data"]).
pub async fn get_record(
    collection_name: &str,
    record_id: &str,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let cfg = config::load_config().map_err(|e| e.to_string())?;
    let api_key = cfg
        .api_key
        .as_deref()
        .ok_or("vgen_API_KEY is not set")?;
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{}/get-record", base);

    let body = json!({
        "collectionName": collection_name,
        "recordId": record_id
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
            "get-record failed: {} {}",
            status,
            res_body.to_string()
        )
        .into());
    }

    Ok(res_body)
}
