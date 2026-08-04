use crate::api::create_assistant_session;
use crate::auth;
use crate::config;
use crate::http_client;
use crate::output::{emit_error, CliContext, CliExitCode};
use crate::specs;
use crate::workspace::Workspace;
use chrono::Utc;
use futures_util::StreamExt;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::{HeaderName, HeaderValue};
use tokio_tungstenite::tungstenite::Message;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TantraStreamEvent {
    Status {
        content: String,
    },
    Content {
        content: String,
    },
    Warning {
        content: String,
        #[serde(rename = "missingRequired")]
        missing_required: Option<Vec<String>>,
        #[serde(rename = "completenessPct")]
        completeness_pct: Option<u8>,
    },
    /// Prajna publishes `{"type":"complete","content":...}`; content is optional.
    Complete {
        #[serde(default)]
        content: Option<String>,
    },
    /// Prajna publishes `{"type":"error","content":...}` (not `message`).
    Error {
        #[serde(alias = "message")]
        content: String,
    },
}

/// Extract printable assistant text from a Tantra `/ask` JSON body.
/// Platform responses use `data.agentResponse` (UI contract); older docs used `response`.
pub fn extract_ask_response_text(http_res: &Value) -> String {
    let candidates = [
        http_res.get("data").and_then(|d| d.get("agentResponse")),
        http_res.get("agentResponse"),
        http_res.get("response"),
        http_res.get("data").and_then(|d| d.get("response")),
    ];

    for candidate in candidates.into_iter().flatten() {
        if let Some(s) = candidate.as_str() {
            return s.to_string();
        }
        if candidate.is_object() || candidate.is_array() {
            return serde_json::to_string_pretty(candidate).unwrap_or_default();
        }
    }
    String::new()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionLog {
    pub session_id: String,
    pub assistant_id: String,
    pub assistant_slug: String,
    pub started_at: String,
    pub turns: Vec<SessionTurn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTurn {
    pub turn_index: usize,
    pub timestamp: String,
    pub prompt: String,
    pub response: String,
    pub latency_ms: u64,
    pub metrics: Option<SessionMetrics>,
    pub transition: Option<SessionTransition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetrics {
    pub tokens_used: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub prompt_tokens: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTransition {
    pub from_mode: String,
    pub to_mode: String,
    pub active_workflow_id: Option<String>,
    pub active_workflow_type: Option<String>,
    pub stage: Option<String>,
    pub stage_label: Option<String>,
}

pub async fn run_assistant_chat(
    ctx: &CliContext,
    name: &str,
    assistants_dir: Option<PathBuf>,
    new_session: bool,
) -> ExitCode {
    let ws = match Workspace::detect(Path::new("")) {
        Ok(w) => w,
        Err(e) => {
            return emit_error(
                ctx,
                "WORKSPACE_DETECT_FAILED",
                format!("Failed to detect workspace: {}", e),
                None,
                CliExitCode::RuntimeError,
            );
        }
    };

    let base = assistants_dir.unwrap_or_else(specs::default_assistants_dir);
    let (yaml, _) = match specs::load_assistant(&base, name) {
        Ok(y) => y,
        Err(e) => {
            return emit_error(
                ctx,
                "ASSISTANT_LOAD_FAILED",
                format!("Failed to load assistant '{}': {}", name, e),
                None,
                CliExitCode::RuntimeError,
            );
        }
    };

    let chat_id = match yaml
        .get("id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        Some(id) => id.to_string(),
        None => {
            return emit_error(
                ctx,
                "ASSISTANT_MISSING_ID",
                format!(
                    "No id in assistants/{}.yaml. Please push first to get an id.",
                    name
                ),
                None,
                CliExitCode::RuntimeError,
            );
        }
    };

    let cfg = match config::load_config() {
        Ok(c) => c,
        Err(e) => {
            return emit_error(
                ctx,
                "CONFIG_LOAD_FAILED",
                format!("Failed to load config: {}", e),
                None,
                CliExitCode::RuntimeError,
            );
        }
    };

    let api_key = match cfg.api_key.as_deref() {
        Some(key) => key,
        None => {
            return emit_error(
                ctx,
                "CONFIG_MISSING_API_KEY",
                "RESMATE_API_KEY is not set.",
                None,
                CliExitCode::RuntimeError,
            );
        }
    };

    let base_url = cfg.base_url.trim_end_matches('/').to_string();

    // Check prompt.json for existing sessionId
    let prompt_dir = base.join(name);
    let prompt_path = prompt_dir.join("prompt.json");
    let mut prompt_json = if prompt_path.exists() {
        std::fs::read_to_string(&prompt_path)
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .unwrap_or_else(|| json!({}))
    } else {
        json!({})
    };

    let existing_session = prompt_json
        .get("sessionId")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let session_id = if new_session || existing_session.is_none() {
        println!("Creating a new session...");
        let new_id = match create_assistant_session(&chat_id).await {
            Ok(id) => id,
            Err(e) => {
                return emit_error(
                    ctx,
                    "SESSION_CREATION_FAILED",
                    format!("Failed to create assistant session: {}", e),
                    None,
                    CliExitCode::RuntimeError,
                );
            }
        };
        if let Some(obj) = prompt_json.as_object_mut() {
            obj.insert("sessionId".to_string(), Value::String(new_id.clone()));
        }
        std::fs::create_dir_all(&prompt_dir).ok();
        if let Ok(serialized) = serde_json::to_string_pretty(&prompt_json) {
            std::fs::write(&prompt_path, serialized).ok();
        }
        new_id
    } else {
        existing_session.unwrap()
    };

    println!("================================================================================");
    println!("💬 Connected to Assistant: {} (ID: {})", name, chat_id);
    println!("Session ID: {}", session_id);
    println!("Type /help for a list of commands, /status to view workflow pipeline.");
    println!("================================================================================");

    // Initialize rustyline editor
    let mut rl = match DefaultEditor::new() {
        Ok(r) => r,
        Err(e) => {
            return emit_error(
                ctx,
                "REPL_INIT_FAILED",
                format!("Failed to initialize interactive shell: {}", e),
                None,
                CliExitCode::RuntimeError,
            );
        }
    };

    // Load history
    let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let resmate_dir = home_dir.join(".resmate");
    std::fs::create_dir_all(&resmate_dir).ok();
    let history_path = resmate_dir.join("repl_history.txt");
    rl.load_history(&history_path).ok();

    let long_client = http_client::build_long_timeout_client(Duration::from_secs(600));
    let mut turn_index = 1;

    loop {
        let readline = rl.readline("resmate> ");
        match readline {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                rl.add_history_entry(trimmed).ok();

                if trimmed == "/quit" || trimmed == "/exit" {
                    println!("Goodbye!");
                    break;
                }

                if trimmed == "/help" {
                    println!("\nAvailable REPL Commands:");
                    println!(
                        "  /status - Fetch and display active workflow stage and completeness"
                    );
                    println!("  /help   - Show this help message");
                    println!("  /quit   - Exit the chat session");
                    println!("  /exit   - Exit the chat session\n");
                    continue;
                }

                if trimmed == "/status" {
                    if let Err(e) =
                        fetch_and_render_status(&long_client, &base_url, api_key, &session_id).await
                    {
                        println!("❌ Error fetching status: {}", e);
                    }
                    continue;
                }

                // Prepare /ask payload
                let payload = json!({
                    "question": trimmed,
                    "sessionId": session_id,
                });

                // Map HTTP URL to WebSocket URL
                let ws_url = map_http_to_ws(&base_url, &session_id);

                // Spawn WebSocket stream listener (Tantra authz requires x-api-key + JWT).
                let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();
                let ws_url_clone = ws_url.clone();
                let api_key_ws = api_key.to_string();
                let roc_session_ws = cfg.roc_session.clone();
                let content_printed = Arc::new(AtomicBool::new(false));
                let content_printed_clone = content_printed.clone();

                let ws_handle = tokio::spawn(async move {
                    let mut request = match ws_url_clone.into_client_request() {
                        Ok(r) => r,
                        Err(e) => {
                            eprintln!(
                                "ℹ️ Live stream unavailable (bad URL: {}); will print the HTTP response when ready.",
                                e
                            );
                            return;
                        }
                    };
                    let headers = request.headers_mut();
                    match HeaderValue::from_str(&api_key_ws) {
                        Ok(v) => {
                            headers.insert(HeaderName::from_static("x-api-key"), v);
                        }
                        Err(_) => {
                            eprintln!(
                                "ℹ️ Live stream unavailable (invalid API key header); will print the HTTP response when ready."
                            );
                            return;
                        }
                    }
                    // Tantra authz rejects tokens with eat > now+120 (TooLongAccess).
                    match auth::build_jwt(60) {
                        Ok(jwt) => match HeaderValue::from_str(&format!("Bearer {}", jwt)) {
                            Ok(v) => {
                                headers.insert(HeaderName::from_static("authorization"), v);
                            }
                            Err(_) => {
                                eprintln!(
                                    "ℹ️ Live stream unavailable (invalid Authorization header); will print the HTTP response when ready."
                                );
                                return;
                            }
                        },
                        Err(e) => {
                            eprintln!(
                                "ℹ️ Live stream unavailable (JWT: {}); will print the HTTP response when ready.",
                                e
                            );
                            return;
                        }
                    }
                    if let Some(roc) = roc_session_ws.as_deref().filter(|s| !s.is_empty()) {
                        if let Ok(v) = HeaderValue::from_str(roc) {
                            headers.insert(HeaderName::from_static("roc-session"), v);
                        }
                    }

                    let (ws_stream, _) = match connect_async(request).await {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!(
                                "ℹ️ Live stream unavailable ({}); will print the HTTP response when ready.",
                                e
                            );
                            return;
                        }
                    };
                    let (_, mut read) = ws_stream.split();
                    let mut last_was_status = false;

                    loop {
                        tokio::select! {
                            _ = &mut stop_rx => {
                                if last_was_status {
                                    print!("\r\x1b[K");
                                    std::io::stdout().flush().ok();
                                }
                                break;
                            }
                            msg = read.next() => {
                                match msg {
                                    Some(Ok(Message::Text(text))) => {
                                        if let Ok(event) = serde_json::from_str::<TantraStreamEvent>(&text) {
                                            match event {
                                                TantraStreamEvent::Status { content } => {
                                                    print!("\r\x1b[K📋 {}", content);
                                                    std::io::stdout().flush().ok();
                                                    last_was_status = true;
                                                }
                                                TantraStreamEvent::Content { content } => {
                                                    if last_was_status {
                                                        print!("\r\x1b[K");
                                                        std::io::stdout().flush().ok();
                                                        last_was_status = false;
                                                    }
                                                    print!("{}", content);
                                                    std::io::stdout().flush().ok();
                                                    content_printed_clone.store(true, Ordering::SeqCst);
                                                }
                                                TantraStreamEvent::Warning { content, missing_required, completeness_pct } => {
                                                    if last_was_status {
                                                        println!("\r\x1b[K");
                                                        last_was_status = false;
                                                    }
                                                    println!("\x1b[33m⚠️ Warning: {}\x1b[0m", content);
                                                    if let Some(mr) = missing_required {
                                                        println!("\x1b[33mMissing required fields: {:?}\x1b[0m", mr);
                                                    }
                                                    if let Some(cp) = completeness_pct {
                                                        println!("\x1b[33mCompleteness: {}%\x1b[0m", cp);
                                                    }
                                                }
                                                TantraStreamEvent::Complete { .. } => {
                                                    if last_was_status {
                                                        print!("\r\x1b[K");
                                                        std::io::stdout().flush().ok();
                                                    } else {
                                                        println!();
                                                    }
                                                    break;
                                                }
                                                TantraStreamEvent::Error { content } => {
                                                    if last_was_status {
                                                        print!("\r\x1b[K");
                                                        std::io::stdout().flush().ok();
                                                    } else {
                                                        println!();
                                                    }
                                                    println!("\x1b[31m❌ Error: {}\x1b[0m", content);
                                                    content_printed_clone.store(true, Ordering::SeqCst);
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                    Some(Err(_)) | None => {
                                        if last_was_status {
                                            print!("\r\x1b[K");
                                            std::io::stdout().flush().ok();
                                        }
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                });

                // Make blocking HTTP /ask request
                let start_time = Instant::now();
                let ask_url = format!("{}/chat/{}/session/{}/ask", base_url, chat_id, session_id);
                let mut req = long_client
                    .post(&ask_url)
                    .json(&payload)
                    .header("Content-Type", "application/json");
                req = match http_client::add_auth_headers(req, api_key) {
                    Ok(r) => r,
                    Err(e) => {
                        println!("❌ Auth error: {}", e);
                        let _ = stop_tx.send(());
                        let _ = ws_handle.await;
                        continue;
                    }
                };
                if let Some(roc) = &cfg.roc_session {
                    req = req.header("roc-session", roc.as_str());
                }

                let http_res = match req.send().await {
                    Ok(res) => {
                        let status = res.status();
                        if status.is_success() {
                            res.json::<Value>().await.unwrap_or(json!({}))
                        } else {
                            let body_text = res.text().await.unwrap_or_default();
                            json!({ "error": format!("HTTP error {}: {}", status, body_text) })
                        }
                    }
                    Err(e) => {
                        json!({ "error": format!("Request failed: {}", e) })
                    }
                };
                let latency_ms = start_time.elapsed().as_millis() as u64;

                // Stop WebSocket stream listener
                let _ = stop_tx.send(());
                let _ = ws_handle.await;

                if let Some(err) = http_res.get("error").and_then(|v| v.as_str()) {
                    println!("\x1b[31m❌ Error: {}\x1b[0m", err);
                    continue;
                }

                // Extract response text (platform uses data.agentResponse)
                let response_text = extract_ask_response_text(&http_res);

                // If WebSocket didn't print any content, print response text now
                if !content_printed.load(Ordering::SeqCst) {
                    if response_text.is_empty() {
                        println!(
                            "\x1b[33m⚠️ Ask completed but no agentResponse/response text was found in the HTTP body.\x1b[0m"
                        );
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&http_res).unwrap_or_default()
                        );
                    } else {
                        println!("{}", response_text);
                    }
                } else {
                    println!();
                }

                // Extract metrics
                let metrics_val = http_res
                    .get("metrics")
                    .or_else(|| http_res.get("data").and_then(|d| d.get("metrics")));

                let tokens_used =
                    metrics_val.and_then(|m| m.get("tokensUsed").and_then(|v| v.as_u64()));
                let completion_tokens =
                    metrics_val.and_then(|m| m.get("completionTokens").and_then(|v| v.as_u64()));
                let prompt_tokens =
                    metrics_val.and_then(|m| m.get("promptTokens").and_then(|v| v.as_u64()));

                // Extract transition
                let transition_val = http_res
                    .get("transition")
                    .or_else(|| http_res.get("data").and_then(|d| d.get("transition")));

                let mut transition_log = None;

                if let Some(transition) = transition_val {
                    let from_mode = transition
                        .get("fromMode")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let to_mode = transition
                        .get("toMode")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let wf_id = transition
                        .get("activeWorkflowId")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let wf_type = transition
                        .get("activeWorkflowType")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let stage = transition
                        .get("stage")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let stage_label = transition
                        .get("stageLabel")
                        .and_then(|v| v.as_str())
                        .map(String::from);

                    transition_log = Some(SessionTransition {
                        from_mode: from_mode.to_string(),
                        to_mode: to_mode.to_string(),
                        active_workflow_id: wf_id.clone(),
                        active_workflow_type: wf_type.clone(),
                        stage: stage.clone(),
                        stage_label: stage_label.clone(),
                    });

                    if to_mode == "ActivePlaybook" && from_mode == "Ambient" {
                        if let Some(ref id) = wf_id {
                            println!("================================================================================");
                            println!("🚀 [WORKFLOW BOUND] Transitioned to Active Playbook Mode!");
                            println!("--------------------------------------------------------------------------------");
                            println!("Instance ID : {}", id);
                            println!(
                                "Active Stage: {} ({})",
                                stage.unwrap_or_default(),
                                stage_label.unwrap_or_default()
                            );

                            if let Some(active_playbook) = transition.get("activePlaybook") {
                                if let Some(constraints) = active_playbook
                                    .get("constraints")
                                    .and_then(|c| c.as_array())
                                {
                                    println!("Constraints :");
                                    for constraint in constraints {
                                        if let Some(c_str) = constraint.as_str() {
                                            println!("  - {}", c_str);
                                        }
                                    }
                                }
                                if let Some(policy) =
                                    active_playbook.get("replyPolicy").and_then(|p| p.as_str())
                                {
                                    println!("Reply Policy: {}", policy);
                                }
                            }
                            println!("================================================================================\n");
                        }
                    }
                }

                // Log session turn
                let turn = SessionTurn {
                    turn_index,
                    timestamp: Utc::now().to_rfc3339(),
                    prompt: trimmed.to_string(),
                    response: response_text,
                    latency_ms,
                    metrics: Some(SessionMetrics {
                        tokens_used,
                        completion_tokens,
                        prompt_tokens,
                    }),
                    transition: transition_log,
                };

                log_session_turn(&ws.root, &session_id, &chat_id, name, turn);
                turn_index += 1;
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }

    // Save history
    rl.save_history(&history_path).ok();
    CliExitCode::Success.into()
}

fn map_http_to_ws(base_url: &str, session_id: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let ws_base = if base.starts_with("https://") {
        base.replace("https://", "wss://")
    } else if base.starts_with("http://") {
        base.replace("http://", "ws://")
    } else {
        format!("ws://{}", base)
    };
    format!("{}/stream/prajna:stream:{}", ws_base, session_id)
}

pub fn render_ascii_progress(
    stages: &[String],
    current_stage: &str,
    completeness_pct: u8,
) -> String {
    let bar_width = 20;
    let filled_chars = ((completeness_pct as f32 / 100.0) * bar_width as f32).round() as usize;
    let empty_chars = bar_width - filled_chars;

    let bar = format!(
        "[{}{}] {}%",
        "█".repeat(filled_chars),
        "░".repeat(empty_chars),
        completeness_pct
    );

    let mut flow_line = String::new();
    for (i, stage) in stages.iter().enumerate() {
        if stage == current_stage {
            flow_line.push_str(&format!("(\x1b[32m●\x1b[0m) {} [ACTIVE]", stage));
        } else {
            flow_line.push_str(&format!("( ) {}", stage));
        }
        if i < stages.len() - 1 {
            flow_line.push_str(" ──> ");
        }
    }

    format!("Progress: {}\nPipeline: {}", bar, flow_line)
}

async fn fetch_and_render_status(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    session_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let url = format!(
        "{}/debug/session/{}/workflow-state",
        base_url.trim_end_matches('/'),
        session_id
    );
    let mut req = client.get(&url);
    req = http_client::add_auth_headers(req, api_key)?;
    let res = req.send().await?;
    let status = res.status();
    let body: serde_json::Value = res.json().await?;

    if !status.is_success() {
        println!("❌ Failed to fetch workflow state: {} {}", status, body);
        return Ok(());
    }

    let mode = body.get("mode").and_then(|v| v.as_str()).unwrap_or("");
    if mode == "Ambient" {
        println!("ℹ️ Session is operating in Ambient Mode (no business workflow bound).");
        return Ok(());
    }

    let current_stage = body
        .get("currentStage")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let stage_label = body
        .get("stageLabel")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let completeness_pct = body
        .get("completenessPct")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u8;
    let stages: Vec<String> = body
        .get("stages")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    println!("\nStage Pipeline:");
    let progress_bar = render_ascii_progress(&stages, current_stage, completeness_pct);
    println!("{}", progress_bar);
    println!("Active Stage: {} ({})", current_stage, stage_label);

    if let Some(missing) = body.get("missingRequired").and_then(|v| v.as_array()) {
        let missing_fields: Vec<&str> = missing.iter().filter_map(|v| v.as_str()).collect();
        if !missing_fields.is_empty() {
            println!("⚠️ Missing Required Fields: {}", missing_fields.join(", "));
        }
    }

    if let Some(active_playbook) = body.get("activePlaybook") {
        if let Some(constraints) = active_playbook
            .get("constraints")
            .and_then(|c| c.as_array())
        {
            let constraints_list: Vec<&str> =
                constraints.iter().filter_map(|v| v.as_str()).collect();
            if !constraints_list.is_empty() {
                println!("Playbook Constraints:");
                for c in constraints_list {
                    println!("  - {}", c);
                }
            }
        }
    }
    println!();

    Ok(())
}

fn log_session_turn(
    ws_root: &Path,
    session_id: &str,
    assistant_id: &str,
    assistant_slug: &str,
    turn: SessionTurn,
) {
    let log_dir = ws_root.join(".resmate/debug/sessions");
    std::fs::create_dir_all(&log_dir).ok();
    let log_path = log_dir.join(format!("{}.json", session_id));

    let mut log = if log_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&log_path) {
            serde_json::from_str::<SessionLog>(&content).unwrap_or_else(|_| SessionLog {
                session_id: session_id.to_string(),
                assistant_id: assistant_id.to_string(),
                assistant_slug: assistant_slug.to_string(),
                started_at: Utc::now().to_rfc3339(),
                turns: Vec::new(),
            })
        } else {
            SessionLog {
                session_id: session_id.to_string(),
                assistant_id: assistant_id.to_string(),
                assistant_slug: assistant_slug.to_string(),
                started_at: Utc::now().to_rfc3339(),
                turns: Vec::new(),
            }
        }
    } else {
        SessionLog {
            session_id: session_id.to_string(),
            assistant_id: assistant_id.to_string(),
            assistant_slug: assistant_slug.to_string(),
            started_at: Utc::now().to_rfc3339(),
            turns: Vec::new(),
        }
    };

    log.turns.push(turn);
    if let Ok(serialized) = serde_json::to_string_pretty(&log) {
        std::fs::write(&log_path, serialized).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_ask_response_prefers_agent_response() {
        let body = json!({
            "data": {
                "agentResponse": "Hello from Teemo",
                "requiresAgentExecution": false
            }
        });
        assert_eq!(extract_ask_response_text(&body), "Hello from Teemo");
    }

    #[test]
    fn extract_ask_response_falls_back_to_response() {
        let body = json!({ "data": { "response": "legacy" } });
        assert_eq!(extract_ask_response_text(&body), "legacy");
    }

    #[test]
    fn stream_error_accepts_content_field() {
        let event: TantraStreamEvent =
            serde_json::from_str(r#"{"type":"error","content":"blocked"}"#).unwrap();
        match event {
            TantraStreamEvent::Error { content } => assert_eq!(content, "blocked"),
            other => panic!("unexpected: {:?}", other),
        }
    }

    #[test]
    fn stream_complete_accepts_content_field() {
        let event: TantraStreamEvent =
            serde_json::from_str(r#"{"type":"complete","content":"HITL_REQUIRED"}"#).unwrap();
        match event {
            TantraStreamEvent::Complete { content } => {
                assert_eq!(content.as_deref(), Some("HITL_REQUIRED"));
            }
            other => panic!("unexpected: {:?}", other),
        }
    }
}
