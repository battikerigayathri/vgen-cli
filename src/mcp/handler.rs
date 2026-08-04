use super::dispatch;

use crate::mcp::tools::list_tool_definitions;
use crate::output::{build_error_json, CliContext, OutputMode};
use serde_json::{json, Value};
use std::env;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

struct McpState {
    workspace_root: PathBuf,
}

pub fn run_stdio_server() -> io::Result<()> {
    let workspace_root = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let state = McpState { workspace_root };
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                write_response(
                    &mut stdout,
                    json!({
                        "jsonrpc": "2.0",
                        "id": null,
                        "error": { "code": -32700, "message": format!("Parse error: {}", e) }
                    }),
                )?;
                continue;
            }
        };
        let response = handle_request(&state, &request);
        if !response.is_null() {
            write_response(&mut stdout, response)?;
        }
    }
    Ok(())
}

fn write_response(stdout: &mut impl Write, response: Value) -> io::Result<()> {
    writeln!(stdout, "{}", serde_json::to_string(&response).unwrap())?;
    stdout.flush()
}

pub fn handle_request(state: &McpState, request: &Value) -> Value {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let params = request.get("params").cloned().unwrap_or(json!({}));

    if request.get("id").is_none() && method == "notifications/initialized" {
        return Value::Null;
    }

    let result = match method {
        "initialize" => json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "resmate-mcp", "version": env!("CARGO_PKG_VERSION") }
        }),
        "tools/list" => json!({ "tools": list_tool_definitions() }),
        "tools/call" => handle_tool_call(state, &params),
        "ping" => json!({}),
        _ => {
            return error_response(id, -32601, format!("Method not found: {}", method));
        }
    };

    if request.get("id").is_none() {
        return Value::Null;
    }

    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn handle_tool_call(state: &McpState, params: &Value) -> Value {
    let name = params
        .get("name")
        .and_then(|n| n.as_str())
        .map(String::from)
        .unwrap_or_default();
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    if let Some(root) = args.get("workspace_root").and_then(|v| v.as_str()) {
        if let Ok(abs) = PathBuf::from(root).canonicalize() {
            let _ = env::set_current_dir(&abs);
        }
    } else if let Ok(abs) = state.workspace_root.canonicalize() {
        let _ = env::set_current_dir(&abs);
    }

    let ctx = CliContext {
        mode: OutputMode::Json,
        command: "mcp",
    };

    let (envelope_json, is_error) = match name.as_str() {
        "workspace_info" => dispatch::tool_workspace_info(&ctx),
        "doctor" => {
            let offline = args
                .get("offline")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            tokio_runtime().block_on(dispatch::tool_doctor(&ctx, offline))
        }
        "graph" => {
            let assistant = args.get("assistant").and_then(|v| v.as_str());
            dispatch::tool_graph(&ctx, assistant)
        }
        "validate" => {
            let strict = args
                .get("strict")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            dispatch::tool_validate(&ctx, strict)
        }
        "workflow_validate" => {
            let wf_name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
            dispatch::tool_workflow_validate(&ctx, wf_name)
        }
        "push_all_dry_run" => {
            let assistant = args.get("assistant").and_then(|v| v.as_str());
            dispatch::tool_push_all_dry_run(&ctx, assistant)
        }
        "push_all_execute" => {
            if !args
                .get("confirm")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                return tool_result(
                    build_error_json(&ctx, "USAGE_CONFIRM_REQUIRED", "confirm must be true", None),
                    true,
                );
            }
            let assistant = args.get("assistant").and_then(|v| v.as_str());
            let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
            tokio_runtime().block_on(dispatch::tool_push_all_execute(&ctx, force, assistant))
        }
        "explain" => {
            let code = args
                .get("code")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            dispatch::tool_explain(&ctx, code)
        }
        "explain_list" => {
            let domain = args.get("domain").and_then(|v| v.as_str());
            dispatch::tool_explain_list(&ctx, domain)
        }
        "init_workspace" => {
            if !args
                .get("confirm")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                return tool_result(
                    build_error_json(&ctx, "USAGE_CONFIRM_REQUIRED", "confirm must be true", None),
                    true,
                );
            }
            let project_name = args
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("my-use-case");
            let description = args.get("description").and_then(|v| v.as_str());
            let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
            dispatch::tool_init(&ctx, project_name, description, force)
        }
        "scaffold_recipe" => {
            if !args
                .get("confirm")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                return tool_result(
                    build_error_json(&ctx, "USAGE_CONFIRM_REQUIRED", "confirm must be true", None),
                    true,
                );
            }
            let recipe = args
                .get("recipe")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let project_name = args
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("my-use-case");
            let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
            dispatch::tool_scaffold(&ctx, recipe, project_name, force)
        }
        "kit_update" => {
            if !args
                .get("confirm")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                return tool_result(
                    build_error_json(&ctx, "USAGE_CONFIRM_REQUIRED", "confirm must be true", None),
                    true,
                );
            }
            let examples = args
                .get("examples")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let dry_run = args
                .get("dry_run")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            dispatch::tool_kit_update(&ctx, examples, dry_run)
        }
        _ => (
            json!({ "ok": false, "error": { "code": "UNKNOWN_TOOL", "message": name } })
                .to_string(),
            true,
        ),
    };

    tool_result(envelope_json, is_error)
}

fn tool_result(envelope_json: String, is_error: bool) -> Value {
    json!({
        "content": [{ "type": "text", "text": envelope_json }],
        "isError": is_error
    })
}

fn error_response(id: Value, code: i32, message: String) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}

fn tokio_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_list_returns_tools() {
        let state = McpState {
            workspace_root: PathBuf::from("."),
        };
        let req = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" });
        let res = handle_request(&state, &req);
        let tools = &res["result"]["tools"];
        assert!(tools.as_array().map(|a| a.len()).unwrap_or(0) >= 8);
    }
}
