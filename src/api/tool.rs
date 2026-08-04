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
    let api_key = cfg.api_key.as_deref().ok_or("RESMATE_API_KEY is not set")?;
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{}/tool/create", base);

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
        return Err(format!("tool/create failed: {} {}", status, body.to_string()).into());
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
        .or_else(|| {
            body.get("id")
                .and_then(|v| v.as_i64())
                .map(|n| n.to_string())
        });

    let cfg = config::load_config().map_err(|e| e.to_string())?;
    let api_key = cfg.api_key.as_deref().ok_or("RESMATE_API_KEY is not set")?;
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{}/tool/update", base);

    let client = http_client::build_client();
    let req = client
        .post(&url)
        .json(body)
        .header("Content-Type", "application/json");
    let req = http_client::add_auth_headers(req, api_key)?;
    let res = req.send().await?;

    let status = res.status();
    let res_body: Value = res.json().await?;

    if !status.is_success() {
        return Err(format!("tool/update failed: {} {}", status, res_body.to_string()).into());
    }

    extract_id_from_response(&res_body)
        .or(request_id)
        .ok_or_else(|| "Response did not contain id".into())
}

/// POST /execute-function with JSON body to test a FaaS tool.
pub async fn test_faas_tool(
    payload: &Value,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let cfg = config::load_config().map_err(|e| e.to_string())?;
    let api_key = cfg.api_key.as_deref().ok_or("RESMATE_API_KEY is not set")?;
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{}/execute-function", base);

    let client = http_client::build_client();
    let req = client
        .post(&url)
        .json(payload)
        .header("Content-Type", "application/json");
    let req = http_client::add_auth_headers(req, api_key)?;
    let res = req.send().await?;

    let status = res.status();
    let res_body: Value = res.json().await?;

    if !status.is_success() {
        return Err(format!(
            "execute-function failed: {} {}",
            status,
            res_body.to_string()
        )
        .into());
    }

    Ok(res_body)
}

/// Attach `roc-session` only when a non-empty real session id is provided.
///
/// Matches push/pull/get-record: API key + JWT only by default. Never send an empty
/// `roc-session` header — Tantra POST logs `session=none` when the header is absent,
/// and empty-string sessions skip ROC GraphQL (`get_roc_session_push_to_redis`).
fn with_optional_roc_session(
    req: reqwest::RequestBuilder,
    session_id: Option<String>,
) -> reqwest::RequestBuilder {
    match session_id {
        Some(s) if !s.trim().is_empty() => req.header("roc-session", s.trim()),
        _ => req,
    }
}

/// POST /debug/execute-javascript with JSON body to test a JS tool.
/// Auth is API key + JWT only. Sets `roc-session` only when `session_id` is Some(non-empty).
pub async fn test_js_tool(
    payload: &Value,
    session_id: Option<String>,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let cfg = config::load_config().map_err(|e| e.to_string())?;
    let api_key = cfg.api_key.as_deref().ok_or("RESMATE_API_KEY is not set")?;
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{}/debug/execute-javascript", base);

    let client = http_client::build_client();
    let req = client
        .post(&url)
        .json(payload)
        .header("Content-Type", "application/json")
        .header("x-debug-tool-test", "true");
    let req = with_optional_roc_session(req, session_id);
    let req = http_client::add_auth_headers(req, api_key)?;
    let res = req.send().await?;

    let status = res.status();
    let res_body: Value = res.json().await?;

    if !status.is_success() {
        return Err(format!(
            "debug/execute-javascript failed: {} {}",
            status,
            res_body.to_string()
        )
        .into());
    }

    Ok(res_body)
}

/// POST /debug/execute-faas with JSON body to test a FaaS tool.
///
/// Body must match FAAS `/invoke` ad-hoc contract (`runtime` + `cmd` + `payload`),
/// not the Kriya JS debug shape. Uses a longer client timeout because FaaS may
/// `bun install` dependencies before running the handler (Tantra proxy is ~60s).
///
/// Headers match get-record/push: API key + JWT; `roc-session` only if explicitly set.
pub async fn test_faas_tool_local(
    payload: &Value,
    session_id: Option<String>,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let cfg = config::load_config().map_err(|e| e.to_string())?;
    let api_key = cfg.api_key.as_deref().ok_or("RESMATE_API_KEY is not set")?;
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{}/debug/execute-faas", base);

    // Slightly above Tantra's 60s proxy timeout so we surface gateway errors.
    let client = http_client::build_long_timeout_client(std::time::Duration::from_secs(90));
    let req = client
        .post(&url)
        .json(payload)
        .header("Content-Type", "application/json")
        .header("x-debug-tool-test", "true");
    let req = with_optional_roc_session(req, session_id);
    let req = http_client::add_auth_headers(req, api_key)?;
    let res = req.send().await?;

    let status = res.status();
    let res_body: Value = res.json().await?;

    if !status.is_success() {
        return Err(format!(
            "debug/execute-faas failed: {} {}",
            status,
            res_body.to_string()
        )
        .into());
    }

    Ok(res_body)
}

/// POST /debug/execute-skill with JSON body to test an integration tool skill.
pub async fn test_tool_execute_skill_override(
    payload: &Value,
    session_id: Option<String>,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let cfg = config::load_config().map_err(|e| e.to_string())?;
    let api_key = cfg.api_key.as_deref().ok_or("RESMATE_API_KEY is not set")?;
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{}/debug/execute-skill", base);

    let client = http_client::build_client();
    let req = client
        .post(&url)
        .json(payload)
        .header("Content-Type", "application/json")
        .header("x-debug-tool-test", "true");
    let req = with_optional_roc_session(req, session_id);
    let req = http_client::add_auth_headers(req, api_key)?;
    let res = req.send().await?;

    let status = res.status();
    let res_body: Value = res.json().await?;

    if !status.is_success() {
        return Err(format!(
            "debug/execute-skill failed: {} {}",
            status,
            res_body.to_string()
        )
        .into());
    }

    Ok(res_body)
}
