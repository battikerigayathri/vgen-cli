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

/// Creates a record in the "chat" collection and returns the generated id.
pub async fn create_assistant(
    payload: &Value,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let cfg = config::load_config().map_err(|e| e.to_string())?;
    
    eprintln!("DEBUG: Loaded config - base_url: {}", cfg.base_url);

    let api_key = cfg
        .api_key
        .as_deref()
        .ok_or("VGEN_API_KEY is not set")?;

    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{}/assistant/create", base);

let body = payload.clone();

    // let body = payload.clone();
    let client = http_client::build_client();
    let mut req = client
        .post(&url)
        .json(&body)
        .header("Content-Type", "application/json");

    req = http_client::add_auth_headers(req, api_key)?;

    if let Some(session) = &cfg.roc_session {
        req = req.header("session", session.as_str());
    }

    eprintln!("DEBUG: Sending POST to {}", url);
    eprintln!("DEBUG: Body = {}", serde_json::to_string_pretty(&body).unwrap());
    eprintln!("DEBUG: About to send request...");

    let res = req.send().await?;
    eprintln!("DEBUG: Response received");
    let status = res.status();
    let res_body: Value = res.json().await?;

    if !status.is_success() {
        return Err(format!(
            "create-record failed: {} {}",
            status,
            res_body.to_string()
        )
        .into());
    }

    extract_id_from_response(&res_body)
        .ok_or_else(|| "Response did not contain id".into())
}

/// Updates a record in the "chat" collection and returns the id.
pub async fn update_assistant(
    record_id: &str,
    document: &Value,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let cfg = config::load_config().map_err(|e| e.to_string())?;

    let api_key = cfg
        .api_key
        .as_deref()
        .ok_or("VGEN_API_KEY is not set")?;

    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{}/update-record", base);

    let body = json!({
        "collectionName": "chat",
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
            "update-record failed: {} {}",
            status,
            res_body.to_string()
        )
        .into());
    }

    Ok(extract_id_from_response(&res_body)
        .unwrap_or_else(|| record_id.to_string()))
}

/// Creates a new session for an assistant
pub async fn create_assistant_session(
    chat_id: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let cfg = config::load_config().map_err(|e| e.to_string())?;

    let api_key = cfg
        .api_key
        .as_deref()
        .ok_or("VGEN_API_KEY is not set")?;

    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{}/create-record", base);

    // Using generic test user details
    let body = json!({
      "collectionName": "session",
      "payload": {
        "chatId": chat_id,
        "user": {
          "name": "Test User",
          "email": "testuser@example.com"
        },
        "status": "active",
        "sessionMetadata": {
          "device": "cli",
          "location": "Local",
          "ipAddress": "127.0.0.1"
        }
      }
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
            "session creation failed: {} {}",
            status,
            res_body.to_string()
        )
        .into());
    }

    extract_id_from_response(&res_body)
        .ok_or_else(|| "Response did not contain session id".into())
}

/// Sends a prompt to an assistant
pub async fn ask_assistant(
    chat_id: &str,
    session_id: &str,
    payload: &Value,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let cfg = config::load_config().map_err(|e| e.to_string())?;

    let api_key = cfg
        .api_key
        .as_deref()
        .ok_or("VGEN_API_KEY is not set")?;

    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{}/chat/{}/session/{}/ask", base, chat_id, session_id);

    let client = http_client::build_client();
    let mut req = client
        .post(&url)
        .json(payload)
        .header("Content-Type", "application/json");

    req = http_client::add_auth_headers(req, api_key)?;

    if let Some(roc_session) = &cfg.roc_session {
        req = req.header("roc-session", roc_session.as_str());
    }

    let res = req.send().await?;

    let status = res.status();
    let res_body: Value = res.json().await?;

    if !status.is_success() {
        return Err(format!(
            "ask assistant failed: {} {}",
            status,
            res_body.to_string()
        )
        .into());
    }

    Ok(res_body)
}