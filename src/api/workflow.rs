use crate::config;
use crate::http_client;
use serde_json::Value;

#[derive(Debug, PartialEq, Eq)]
pub enum PushResult {
    Success(String),
    Conflict,
}

fn extract_id(body: &Value) -> Option<String> {
    // 1. Check data._id.$oid
    if let Some(oid) = body
        .get("data")
        .and_then(|d| d.get("_id"))
        .and_then(|id| id.get("$oid"))
        .and_then(|v| v.as_str())
    {
        return Some(oid.to_string());
    }
    // 2. Check data._id as string
    if let Some(id_str) = body
        .get("data")
        .and_then(|d| d.get("_id"))
        .and_then(|v| v.as_str())
    {
        return Some(id_str.to_string());
    }
    // 3. Check data.id as string
    if let Some(id_str) = body
        .get("data")
        .and_then(|d| d.get("id"))
        .and_then(|v| v.as_str())
    {
        return Some(id_str.to_string());
    }
    // 4. Check top-level id as string
    if let Some(id_str) = body.get("id").and_then(|v| v.as_str()) {
        return Some(id_str.to_string());
    }
    None
}

/// POST /workflow/definition/create
pub async fn create_workflow_definition(
    body: &Value,
) -> Result<PushResult, Box<dyn std::error::Error + Send + Sync>> {
    let slug = body
        .get("authorBundle")
        .and_then(|b| b.get("meta"))
        .and_then(|m| m.get("slug"))
        .and_then(|v| v.as_str());

    let version = body
        .get("authorBundle")
        .and_then(|b| b.get("meta"))
        .and_then(|m| m.get("version"))
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    let cfg = config::load_config().map_err(|e| e.to_string())?;
    let api_key = cfg.api_key.as_deref().ok_or("VGEN_API_KEY is not set")?;
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{}/workflow/definition/create", base);

    let client = http_client::build_client();
    let req = client
        .post(&url)
        .json(body)
        .header("Content-Type", "application/json");
    let req = http_client::add_auth_headers(req, api_key)?;
    let res = req.send().await?;

    let status = res.status();
    let res_body: Value = res.json().await?;

    if status == reqwest::StatusCode::CONFLICT {
        return Ok(PushResult::Conflict);
    }

    if !status.is_success() {
        if let Some(err_code) = res_body.get("error").and_then(|v| v.as_str()) {
            if err_code == "DuplicateDefinition" {
                return Ok(PushResult::Conflict);
            }
        }
        return Err(format!(
            "workflow/definition/create failed: {} {}",
            status,
            res_body.to_string()
        )
        .into());
    }

    let mut id = extract_id(&res_body);
    if id.is_none() {
        if let (Some(s), Some(v)) = (slug, version) {
            // Fallback: fetch the record to get its ID since create response might have omitted it
            if let Ok(get_res) = get_workflow_definition(s, Some(v)).await {
                id = extract_id(&get_res);
            }
        }
    }

    let id_str = id.unwrap_or_default();
    Ok(PushResult::Success(id_str))
}

/// POST /workflow/definition/get
pub async fn get_workflow_definition(
    slug: &str,
    version: Option<u32>,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let cfg = config::load_config().map_err(|e| e.to_string())?;
    let api_key = cfg.api_key.as_deref().ok_or("VGEN_API_KEY is not set")?;
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{}/workflow/definition/get", base);

    let mut payload = serde_json::json!({
        "slug": slug,
    });
    if let Some(v) = version {
        payload["version"] = serde_json::Value::Number(v.into());
    }

    let client = http_client::build_client();
    let req = client
        .post(&url)
        .json(&payload)
        .header("Content-Type", "application/json");
    let req = http_client::add_auth_headers(req, api_key)?;
    let res = req.send().await?;

    let status = res.status();
    let res_body: Value = res.json().await?;

    if !status.is_success() {
        return Err(format!(
            "workflow/definition/get failed: {} {}",
            status,
            res_body.to_string()
        )
        .into());
    }

    Ok(res_body)
}
