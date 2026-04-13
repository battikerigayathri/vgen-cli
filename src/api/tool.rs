use crate::config;
use crate::http_client;
use serde_json::Value;

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

/// POST /tool/create with JSON body. Returns the created tool id from the response.
pub async fn create_tool(body: &Value) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let cfg = config::load_config().map_err(|e| e.to_string())?;
    let api_key = cfg
        .api_key
        .as_deref()
        .ok_or("RESMATE_API_KEY is not set")?;
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{}/tool/create", base);

    let client = http_client::build_client();
    let req = client.post(&url).json(body).header("Content-Type", "application/json");
    let req = http_client::add_auth_headers(req, api_key)?;
    let res = req.send().await?;

    let status = res.status();
    let body: Value = res.json().await?;

    if !status.is_success() {
        return Err(format!(
            "tool/create failed: {} {}",
            status,
            body.to_string()
        )
        .into());
    }

    extract_id_from_response(&body).ok_or_else(|| "Response did not contain id".into())
}

/// POST /tool/update with body including id. Returns the tool id (from response or request body).
/// Update API may return only { "data": { "message": "Record updated successfully" } } with no id.
pub async fn update_tool(body: &Value) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let request_id = body
        .get("id")
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| body.get("id").and_then(|v| v.as_i64()).map(|n| n.to_string()));

    let cfg = config::load_config().map_err(|e| e.to_string())?;
    let api_key = cfg
        .api_key
        .as_deref()
        .ok_or("RESMATE_API_KEY is not set")?;
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{}/tool/update", base);

    let client = http_client::build_client();
    let req = client.post(&url).json(body).header("Content-Type", "application/json");
    let req = http_client::add_auth_headers(req, api_key)?;
    let res = req.send().await?;

    let status = res.status();
    let res_body: Value = res.json().await?;

    if !status.is_success() {
        return Err(format!(
            "tool/update failed: {} {}",
            status,
            res_body.to_string()
        )
        .into());
    }

    extract_id_from_response(&res_body)
        .or(request_id)
        .ok_or_else(|| "Response did not contain id".into())
}
