use clap::Parser;
use vgen::api;
use vgen::cli::{
    AgentSubcommand, AssistantSubcommand, Cli, Commands, ConfigSubcommand, EnvSubcommand,
    HitlSubcommand, KitSubcommand, ToolSubcommand, WorkflowSubcommand, WorkspaceSubcommand,
};
use vgen::commands::{
    chat as chat_cmd, diff as diff_cmd, doctor as doctor_cmd, env as env_cmd,
    explain as explain_cmd, graph as graph_cmd, init as init_cmd, kit as kit_cmd,
    push_all as push_all_cmd, scaffold as scaffold_cmd, validate as validate_cmd,
    workflow as workflow_cmd, workspace as workspace_cmd,
};
use vgen::config;
use vgen::http_client;
use vgen::output::{CliContext, OutputMode};
use vgen::specs;
use vgen::store;
use vgen::sync::{run_sync_command, sync_options_from_cmd};
use std::process::ExitCode;

fn read_json_file(
    path: &std::path::Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let s = std::fs::read_to_string(path)?;
    let v: serde_json::Value = serde_json::from_str(&s)?;
    Ok(v)
}

/// Split a tool-test payload into Kriya debug `inputData` + optional `context`.
///
/// Accepts:
/// - bare input object: `{ "issueKey": "..." }`
/// - context envelope: `{ "context": { "input": {...}, ... } }`
/// - event envelope: `{ "event": { "context": { "input": {...}, ... } } }`
fn split_tool_test_payload(
    raw: serde_json::Value,
) -> (serde_json::Value, Option<serde_json::Value>) {
    let envelope = match raw.get("event") {
        Some(event) => event.clone(),
        None => raw,
    };

    if let Some(ctx_val) = envelope.get("context") {
        let mut ctx_obj = match ctx_val.as_object() {
            Some(obj) => obj.clone(),
            None => return (ctx_val.clone(), None),
        };
        let input = ctx_obj
            .remove("input")
            .unwrap_or_else(|| serde_json::json!({}));
        let context = if ctx_obj.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(ctx_obj))
        };
        return (input, context);
    }

    if let Some(input) = envelope.get("input") {
        return (input.clone(), None);
    }

    (envelope, None)
}

fn build_debug_js_body(
    code: String,
    input_data: serde_json::Value,
    context: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut body = serde_json::Map::new();
    body.insert("code".to_string(), serde_json::Value::String(code));
    body.insert("inputData".to_string(), input_data);
    if let Some(ctx) = context {
        body.insert("context".to_string(), ctx);
    }
    serde_json::Value::Object(body)
}

/// Rewrite absolute `/runtime/...` imports so FAAS `run.js` does not mangle quotes.
///
/// FAAS runtime-node replaces `from ["/']/runtime/` with `from './runtime/` but leaves the
/// original closing quote unchanged, producing broken JS like:
///   from './runtime/runtime-sdks/smriti.js"
/// which Bun reports as "Unterminated string literal". Rewriting to `./runtime/...` with a
/// matching closing quote avoids that transform entirely (CLI-only workaround; do not change FAAS).
fn rewrite_faas_runtime_imports(code: &str) -> String {
    code.replace(r#"from "/runtime/"#, r#"from "./runtime/"#)
        .replace(r#"from '/runtime/"#, r#"from './runtime/"#)
        .replace(r#"from("/runtime/"#, r#"from("./runtime/"#)
        .replace(r#"from('/runtime/"#, r#"from('./runtime/"#)
        .replace(r#"require("/runtime/"#, r#"require("./runtime/"#)
        .replace(r#"require('/runtime/"#, r#"require('./runtime/"#)
}

/// Prepend a tiny debug shim so common Kriya status APIs are no-ops under ad-hoc FAAS invoke.
///
/// Ad-hoc `/debug/execute-faas` runs handlers in the FAAS container without the full Kriya
/// chat/session pipeline, so `kriya.process.publishStatus` is often undefined. Tools that
/// call it (Teemo FAAS handlers) would otherwise fail before real logic runs.
fn inject_faas_debug_shims(code: &str) -> String {
    const SHIM: &str = r#"/* vgen tool-test FAAS debug shim */
import { kriya as __vgenKriya } from "./runtime/runtime-sdks/kriya.js";
try {
  if (__vgenKriya && typeof __vgenKriya === "object") {
    __vgenKriya.process = __vgenKriya.process || {};
    if (typeof __vgenKriya.process.publishStatus !== "function") {
      __vgenKriya.process.publishStatus = async () => {};
    }
  }
} catch (_) {}
"#;
    format!("{SHIM}\n{code}")
}

/// Build the FaaS `/invoke` ad-hoc body expected by Tantra → FAAS.
///
/// FAAS `/invoke` accepts either `(function_id, event)` or `(runtime, cmd, payload)`.
/// Local `vgen tool test` for FAAS must use the ad-hoc shape so local `handler.js`
/// runs without a prior `/deploy`. Nested `payload` matches runtime-node `run.js`:
/// `{ code, packageJson, event }` where `event.context.input` is tool arguments.
///
/// Sending `{ code, inputData, ... }` (JS debug shape) makes FAAS return a plain-text
/// 400; Tantra then fails with "Failed to parse HTTP response: error decoding response body".
fn build_debug_faas_body(
    code: String,
    package_json: serde_json::Value,
    input_data: serde_json::Value,
    context: Option<serde_json::Value>,
) -> serde_json::Value {
    let code = inject_faas_debug_shims(&rewrite_faas_runtime_imports(&code));

    let mut ctx = serde_json::Map::new();
    ctx.insert("input".to_string(), input_data);
    if let Some(extra) = context {
        if let Some(obj) = extra.as_object() {
            for (k, v) in obj {
                if k != "input" {
                    ctx.insert(k.clone(), v.clone());
                }
            }
        }
    }

    serde_json::json!({
        "runtime": "runtime-node:latest",
        "cmd": ["bun", "/sandbox/run.js"],
        // Stay under Tantra's 60s HTTP proxy timeout.
        "timeout_secs": 55,
        "include_logs": true,
        "payload": {
            "code": code,
            "packageJson": package_json,
            "event": {
                "context": serde_json::Value::Object(ctx)
            }
        }
    })
}

fn format_and_print_execution_result(res: &serde_json::Value) {
    println!("\n\x1b[1;34m=== Tool Execution Result ===\x1b[0m");

    let logs = res
        .get("logs")
        .or_else(|| res.get("data").and_then(|d| d.get("logs")))
        .or_else(|| res.get("result").and_then(|r| r.get("logs")));

    if let Some(logs_val) = logs {
        println!("\x1b[1;33m--- Logs ---\x1b[0m");
        if let Some(arr) = logs_val.as_array() {
            for log in arr {
                if let Some(s) = log.as_str() {
                    println!("\x1b[36m{}\x1b[0m", s);
                } else {
                    println!("\x1b[36m{}\x1b[0m", log);
                }
            }
        } else if let Some(s) = logs_val.as_str() {
            for line in s.lines() {
                println!("\x1b[36m{}\x1b[0m", line);
            }
        } else {
            println!("\x1b[36m{}\x1b[0m", logs_val);
        }
    }

    let duration = res
        .get("duration")
        .or_else(|| res.get("executionTime"))
        .or_else(|| res.get("data").and_then(|d| d.get("duration")))
        .or_else(|| res.get("data").and_then(|d| d.get("executionTime")));

    if let Some(d_val) = duration {
        if let Some(ms) = d_val.as_f64() {
            println!("\x1b[1;32mDuration: {:.2} ms\x1b[0m", ms);
        } else if let Some(ms) = d_val.as_i64() {
            println!("\x1b[1;32mDuration: {} ms\x1b[0m", ms);
        } else if let Some(s) = d_val.as_str() {
            println!("\x1b[1;32mDuration: {}\x1b[0m", s);
        }
    }

    let outcome = res
        .get("result")
        .or_else(|| res.get("outcome"))
        .or_else(|| res.get("data").and_then(|d| d.get("result")))
        .or_else(|| res.get("data").and_then(|d| d.get("outcome")));

    println!("\x1b[1;35m--- Outcome ---\x1b[0m");
    if let Some(out_val) = outcome {
        if let Ok(pretty) = serde_json::to_string_pretty(out_val) {
            println!("{}", pretty);
        } else {
            println!("{:?}", out_val);
        }
    } else {
        let mut clean_res = res.clone();
        if let Some(obj) = clean_res.as_object_mut() {
            obj.remove("logs");
            obj.remove("duration");
            obj.remove("executionTime");
            if let Some(data_val) = obj.get_mut("data") {
                if let Some(data_obj) = data_val.as_object_mut() {
                    data_obj.remove("logs");
                    data_obj.remove("duration");
                    data_obj.remove("executionTime");
                }
            }
        }
        if let Ok(pretty) = serde_json::to_string_pretty(&clean_res) {
            println!("{}", pretty);
        } else {
            println!("{:?}", clean_res);
        }
    }
    println!("\x1b[1;34m=============================\x1b[0m\n");
}

#[tokio::main]
async fn main() -> ExitCode {
    // Load .env from cwd so VGEN_SECRET and others are available
    dotenvy::dotenv().ok();

    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            e.print().ok();
            return ExitCode::from(2);
        }
    };

    match run(cli).await {
        Ok(code) => code,
        Err(e) => {
            eprintln!("{}", e);
            ExitCode::from(1)
        }
    }
}

async fn run(cli: Cli) -> Result<ExitCode, Box<dyn std::error::Error + Send + Sync>> {
    let json = cli.json;

    match cli.command {
        Commands::Workspace(cmd) => match cmd.subcommand {
            WorkspaceSubcommand::Info => {
                let ctx = CliContext {
                    mode: OutputMode::from_json_flag(json),
                    command: "workspace info",
                };
                return Ok(workspace_cmd::run_workspace_info(&ctx));
            }
        },
        Commands::Doctor(cmd) => {
            let ctx = CliContext {
                mode: OutputMode::from_json_flag(json),
                command: "doctor",
            };
            return Ok(doctor_cmd::run_doctor(&ctx, cmd.offline).await);
        }
        Commands::Graph(cmd) => {
            let ctx = CliContext {
                mode: OutputMode::from_json_flag(json),
                command: "graph",
            };
            return Ok(graph_cmd::run_graph(&ctx, cmd.assistant.as_deref()));
        }
        Commands::Validate(cmd) => {
            let ctx = CliContext {
                mode: OutputMode::from_json_flag(json),
                command: "validate",
            };
            return Ok(validate_cmd::run_validate(&ctx, cmd.strict, cmd.offline, cmd.remote).await);
        }
        Commands::Diff(cmd) => {
            let ctx = CliContext {
                mode: OutputMode::from_json_flag(json),
                command: "diff",
            };
            return Ok(diff_cmd::run_diff(&ctx, Some(&cmd.resource_type), &cmd.name).await);
        }
        Commands::Explain(cmd) => {
            let ctx = CliContext {
                mode: OutputMode::from_json_flag(json),
                command: "explain",
            };
            return Ok(explain_cmd::run_explain(
                &ctx,
                cmd.code.as_deref(),
                cmd.list,
                cmd.domain.as_deref(),
            ));
        }
        Commands::Init(cmd) => {
            let ctx = CliContext {
                mode: OutputMode::from_json_flag(json),
                command: "init",
            };
            return Ok(init_cmd::run_init(
                &ctx,
                cmd.name.as_deref(),
                cmd.description.as_deref(),
                cmd.force,
                cmd.no_examples,
            ));
        }
        Commands::Scaffold(cmd) => {
            let ctx = CliContext {
                mode: OutputMode::from_json_flag(json),
                command: "scaffold",
            };
            return Ok(scaffold_cmd::run_scaffold(
                &ctx,
                &cmd.recipe,
                cmd.name.as_deref(),
                cmd.force,
                cmd.with_kit,
            ));
        }
        Commands::Kit(cmd) => match cmd.subcommand {
            KitSubcommand::Update { examples, dry_run } => {
                let ctx = CliContext {
                    mode: OutputMode::from_json_flag(json),
                    command: "kit update",
                };
                return Ok(kit_cmd::run_kit_update(&ctx, examples, dry_run));
            }
        },
        Commands::PushAll(cmd) => {
            let ctx = CliContext {
                mode: OutputMode::from_json_flag(json),
                command: "push-all",
            };
            return Ok(push_all_cmd::run_push_all(
                &ctx,
                cmd.dry_run,
                cmd.yes,
                cmd.force,
                cmd.stop_on_error,
                cmd.assistant.as_deref(),
            )
            .await);
        }
        Commands::Tool(cmd) => match cmd.subcommand {
            ToolSubcommand::Create { slug, body_file } => {
                let body = read_json_file(&body_file)?;
                let id = api::create_tool(&body).await?;
                let mut s = store::IdStore::load()?;
                s.set_tool_id(slug, id.clone());
                s.save()?;
                println!("Created tool: {}", id);
            }
            ToolSubcommand::Update { slug, body_file } => {
                let mut body = read_json_file(&body_file)?;
                if body.get("id").is_none() {
                    let s = store::IdStore::load()?;
                    let id = s.get_tool_id(&slug).ok_or_else(|| {
                        format!(
                            "No tool id for slug '{}'. Create first or add id to body.",
                            slug
                        )
                    })?;
                    body["id"] = serde_json::Value::String(id.to_string());
                }
                let id = api::update_tool(&body).await?;
                let mut s = store::IdStore::load()?;
                s.set_tool_id(slug, id.clone());
                s.save()?;
                println!("Updated tool: {}", id);
            }
            ToolSubcommand::Push { name, tools_dir } => {
                let base = tools_dir.unwrap_or_else(specs::default_tools_dir);
                let tool_dir = base.join(&name);
                let (mut body, yaml_path) = specs::load_tool_from_dir(&tool_dir)?;
                let has_id = body
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| !s.is_empty())
                    .unwrap_or(false);
                let id = if has_id {
                    api::update_tool(&body).await?
                } else {
                    body.as_object_mut().and_then(|o| o.remove("id"));
                    let id = api::create_tool(&body).await?;
                    specs::write_tool_id_to_yaml(&yaml_path, &id)?;
                    id
                };
                println!("Pushed tool: {}", id);
            }
            ToolSubcommand::Pull { name, tools_dir } => {
                let base = tools_dir.unwrap_or_else(specs::default_tools_dir);
                let tool_dir = base.join(&name);
                let (id, _) = specs::get_tool_id_and_yaml_path(&tool_dir)?;
                let res = api::get_record("skillConfig", &id).await?;
                let data = res.get("data").ok_or("get-record response has no data")?;
                specs::write_tool_from_record(&tool_dir, data)?;
                println!("Pulled tool: {}", name);
            }
            ToolSubcommand::Test {
                name,
                tools_dir,
                payload,
                integration,
                session,
                skill_id,
                no_pull,
            } => {
                let base = tools_dir.unwrap_or_else(specs::default_tools_dir);
                let tool_dir = base.join(&name);

                if !no_pull {
                    // Pull latest tool from API before testing
                    let (id, _) = specs::get_tool_id_and_yaml_path(&tool_dir)?;
                    println!("Pulling latest tool '{}'...", name);
                    let res = api::get_record("skillConfig", &id).await?;
                    let data = res.get("data").ok_or("get-record response has no data")?;
                    specs::write_tool_from_record(&tool_dir, data)?;
                    println!("Pulled tool: {}", name);
                }

                let (yaml, _) = specs::load_tool_yaml(&tool_dir)?;

                let tool_type = yaml
                    .get("type")
                    .and_then(|v: &serde_json::Value| v.as_str())
                    .unwrap_or("");
                let is_faas = tool_type.eq_ignore_ascii_case("faas");
                let is_js = tool_type.eq_ignore_ascii_case("js")
                    || tool_type.eq_ignore_ascii_case("javascript");

                if !is_faas && !is_js {
                    return Err(format!(
                        "Tool '{}' is of type '{}'. Only 'JS' and 'FaaS' tools are supported for testing.",
                        name, tool_type
                    )
                    .into());
                }

                let handler_path = tool_dir.join("handler.js");
                if !handler_path.exists() {
                    return Err(format!(
                        "handler.js not found in {}. It is required for testing.",
                        tool_dir.display()
                    )
                    .into());
                }
                let handler_js_content = std::fs::read_to_string(&handler_path)?;

                let mut package_json_value = serde_json::Value::Null;
                if is_faas {
                    let package_json_path = tool_dir.join("package.json");
                    if package_json_path.exists() {
                        package_json_value = read_json_file(&package_json_path)?;
                    } else {
                        return Err(format!(
                            "package.json not found in {}. It is required for FaaS tools.",
                            tool_dir.display()
                        )
                        .into());
                    }
                }

                let payload_file_path = match payload {
                    Some(ref p) => p.clone(),
                    None => tool_dir.join("payload.json"),
                };
                if !payload_file_path.exists() {
                    return Err(format!(
                        "Payload file not found at {}. A payload is required for testing.",
                        payload_file_path.display()
                    )
                    .into());
                }
                let raw_payload = read_json_file(&payload_file_path)?;
                let (input_data, context) = split_tool_test_payload(raw_payload);

                // Only forward roc-session when the user explicitly passes --session.
                let session = session.filter(|s| !s.trim().is_empty());

                let result = if integration {
                    let Some(ref session_id) = session else {
                        return Err(
                            "--integration requires --session <real-session-id> (Kriya loads workflow state for that session). For default local handler tests, omit --integration."
                                .into(),
                        );
                    };
                    let target_skill_id = match skill_id {
                        Some(ref id) => id.clone(),
                        None => {
                            let (yaml_id, _) = specs::get_tool_id_and_yaml_path(&tool_dir)?;
                            yaml_id
                        }
                    };
                    // ExecuteSkillRequest field names (kriya): session, skill_id, input_data,
                    // context, codeOverride, packageJsonOverride, skillTypeOverride.
                    let mut req_body = serde_json::json!({
                        "session": session_id,
                        "skill_id": target_skill_id,
                        "input_data": input_data,
                        "codeOverride": handler_js_content,
                        "includeLogs": true,
                    });
                    if let Some(ctx) = context {
                        if let Some(obj) = req_body.as_object_mut() {
                            obj.insert("context".to_string(), ctx);
                        }
                    }
                    if is_faas {
                        if let Some(obj) = req_body.as_object_mut() {
                            obj.insert("packageJsonOverride".to_string(), package_json_value);
                            obj.insert(
                                "skillTypeOverride".to_string(),
                                serde_json::Value::String("FAAS".to_string()),
                            );
                        }
                    }
                    api::test_tool_execute_skill_override(&req_body, session.clone()).await?
                } else if is_js {
                    let req_body = build_debug_js_body(handler_js_content, input_data, context);
                    api::test_js_tool(&req_body, session).await?
                } else {
                    let req_body = build_debug_faas_body(
                        handler_js_content,
                        package_json_value,
                        input_data,
                        context,
                    );
                    api::test_faas_tool_local(&req_body, session).await?
                };

                format_and_print_execution_result(&result);
            }
        },
        Commands::Agent(cmd) => match cmd.subcommand {
            AgentSubcommand::Create { slug, body_file } => {
                let body = read_json_file(&body_file)?;
                let id = api::create_agent(&body).await?;
                let mut s = store::IdStore::load()?;
                s.set_agent_id(slug, id.clone());
                s.save()?;
                println!("Created agent: {}", id);
            }
            AgentSubcommand::Update {
                slug,
                body_file,
                id: override_id,
            } => {
                let document = read_json_file(&body_file)?;
                let record_id = override_id
                    .or_else(|| {
                        store::IdStore::load()
                            .ok()
                            .and_then(|s| s.get_agent_id(&slug).map(String::from))
                    })
                    .ok_or_else(|| {
                        format!("No agent id for slug '{}'. Use --id or create first.", slug)
                    })?;
                let id = api::update_agent(&record_id, &document).await?;
                let mut s = store::IdStore::load()?;
                s.set_agent_id(slug, id.clone());
                s.save()?;
                println!("Updated agent: {}", id);
            }
            AgentSubcommand::Push { name, agents_dir } => {
                let base = agents_dir.unwrap_or_else(specs::default_agents_dir);
                let (mut body, yaml_path) = specs::load_agent(&base, &name)?;
                let has_id = body
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| !s.is_empty())
                    .unwrap_or(false);
                let id = if has_id {
                    let record_id = body.get("id").and_then(|v| v.as_str()).unwrap();
                    api::update_agent(record_id, &body).await?
                } else {
                    body.as_object_mut().and_then(|o| o.remove("id"));
                    let id = api::create_agent(&body).await?;
                    specs::write_agent_id_to_yaml(&yaml_path, &id)?;
                    id
                };
                println!("Pushed agent: {}", id);
            }
            AgentSubcommand::Pull { name, agents_dir } => {
                let base = agents_dir.unwrap_or_else(specs::default_agents_dir);
                let (body, yaml_path) = specs::load_agent(&base, &name)?;
                let id = body
                    .get("id")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| {
                        format!("No id in agents/{}.yaml; push first or add id.", name)
                    })?;
                let res = api::get_record("agentConfig", id).await?;
                let data = res.get("data").ok_or("get-record response has no data")?;
                specs::write_agent_yaml_from_record(&yaml_path, data)?;
                println!("Pulled agent: {}", name);
            }
        },
        Commands::Assistant(cmd) => match cmd.subcommand {
            AssistantSubcommand::Create { slug, body_file } => {
                let payload = read_json_file(&body_file)?;
                let id = api::create_assistant(&payload).await?;
                let mut s = store::IdStore::load()?;
                s.set_assistant_id(slug, id.clone());
                s.save()?;
                println!("Created assistant: {}", id);
            }
            AssistantSubcommand::Update {
                slug,
                body_file,
                id: override_id,
            } => {
                let document = read_json_file(&body_file)?;
                let record_id = override_id
                    .or_else(|| {
                        store::IdStore::load()
                            .ok()
                            .and_then(|s| s.get_assistant_id(&slug).map(String::from))
                    })
                    .ok_or_else(|| {
                        format!(
                            "No assistant id for slug '{}'. Use --id or create first.",
                            slug
                        )
                    })?;
                let id = api::update_assistant(&record_id, &document).await?;
                let mut s = store::IdStore::load()?;
                s.set_assistant_id(slug, id.clone());
                s.save()?;
                println!("Updated assistant: {}", id);
            }
            AssistantSubcommand::Push {
                name,
                assistants_dir,
            } => {
                let base = assistants_dir.unwrap_or_else(specs::default_assistants_dir);
                let (mut body, yaml_path) = specs::load_assistant(&base, &name)?;
                let has_id = body
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| !s.is_empty())
                    .unwrap_or(false);
                let id = if has_id {
                    let record_id = body.get("id").and_then(|v| v.as_str()).unwrap();
                    api::update_assistant(record_id, &body).await?
                } else {
                    body.as_object_mut().and_then(|o| o.remove("id"));
                    let id = api::create_assistant(&body).await?;
                    specs::write_assistant_id_to_yaml(&yaml_path, &id)?;
                    id
                };
                println!("Pushed assistant: {}", id);
            }
            AssistantSubcommand::Pull {
                name,
                assistants_dir,
            } => {
                let base = assistants_dir.unwrap_or_else(specs::default_assistants_dir);
                let (body, yaml_path) = specs::load_assistant(&base, &name)?;
                let id = body
                    .get("id")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| {
                        format!("No id in assistants/{}.yaml; push first or add id.", name)
                    })?;
                let res = api::get_record("chat", id).await?;
                let data = res.get("data").ok_or("get-record response has no data")?;
                specs::write_assistant_yaml_from_record(&yaml_path, data)?;
                println!("Pulled assistant: {}", name);
            }
            AssistantSubcommand::Test {
                name,
                assistants_dir,
                new_session,
            } => {
                let base = assistants_dir.unwrap_or_else(specs::default_assistants_dir);
                let (yaml, _) = specs::load_assistant(&base, &name)?;

                let chat_id = yaml
                    .get("id")
                    .and_then(|v: &serde_json::Value| v.as_str())
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| {
                        format!(
                            "No id in assistants/{}.yaml. Please push first to get an id.",
                            name
                        )
                    })?
                    .to_string();

                let prompt_dir = base.join(&name);
                std::fs::create_dir_all(&prompt_dir)?;
                let prompt_path = prompt_dir.join("prompt.json");

                if !prompt_path.exists() {
                    return Err(format!(
                        "{} not found. Please create it with at least a \"question\" field.",
                        prompt_path.display()
                    )
                    .into());
                }

                let mut prompt = read_json_file(&prompt_path)?;
                let existing_session = prompt
                    .get("sessionId")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(String::from);

                let session_id = if new_session || existing_session.is_none() {
                    println!("Creating a new session...");
                    let new_id = api::create_assistant_session(&chat_id).await?;
                    if let Some(obj) = prompt.as_object_mut() {
                        obj.insert(
                            "sessionId".to_string(),
                            serde_json::Value::String(new_id.clone()),
                        );
                    }
                    std::fs::write(&prompt_path, serde_json::to_string_pretty(&prompt)?)?;
                    println!("Updated prompt.json with new sessionId.");
                    new_id
                } else {
                    existing_session.unwrap()
                };

                println!(
                    "Asking assistant (Chat ID: {}, Session ID: {})...",
                    chat_id, session_id
                );
                let result = api::ask_assistant(&chat_id, &session_id, &prompt).await?;
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            AssistantSubcommand::Chat {
                name,
                assistants_dir,
                new_session,
            } => {
                let ctx = CliContext {
                    mode: OutputMode::from_json_flag(json),
                    command: "assistant chat",
                };
                return Ok(
                    chat_cmd::run_assistant_chat(&ctx, &name, assistants_dir, new_session).await,
                );
            }
        },
        Commands::Hitl(cmd) => match cmd.subcommand {
            HitlSubcommand::Push { name, hitl_dir } => {
                let base = hitl_dir.unwrap_or_else(specs::default_hitl_dir);
                let hitl_dir_path = base.join(&name);
                let (mut body, meta_path) = specs::load_hitl_from_dir(&hitl_dir_path)?;
                let has_id = body
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| !s.is_empty())
                    .unwrap_or(false);
                let id = if has_id {
                    let record_id = body.get("id").and_then(|v| v.as_str()).unwrap().to_string();
                    body.as_object_mut().and_then(|o| o.remove("id"));
                    api::update_hitl(&record_id, &body).await?
                } else {
                    body.as_object_mut().and_then(|o| o.remove("id"));
                    let id = api::create_hitl(&body).await?;
                    specs::write_hitl_id_to_meta(&meta_path, &id)?;
                    id
                };
                println!("Pushed HITL: {}", id);
            }
            HitlSubcommand::Pull { name, hitl_dir } => {
                let base = hitl_dir.unwrap_or_else(specs::default_hitl_dir);
                let hitl_dir_path = base.join(&name);
                let (id, _meta_path) = specs::get_hitl_id_and_meta_path(&hitl_dir_path)?;
                let res = api::get_record("hitlConfig", &id).await?;
                let data = res.get("data").ok_or("get-record response has no data")?;
                specs::write_hitl_from_record(&hitl_dir_path, data)?;
                println!("Pulled HITL: {}", name);
            }
        },
        Commands::Config(cmd) => {
            return match cmd.subcommand {
                ConfigSubcommand::Show => {
                    let ctx = CliContext {
                        mode: OutputMode::from_json_flag(json),
                        command: "config show",
                    };
                    Ok(config::run_config_show(&ctx))
                }
                ConfigSubcommand::Validate => {
                    let ctx = CliContext {
                        mode: OutputMode::from_json_flag(json),
                        command: "config validate",
                    };
                    Ok(http_client::run_config_validate(&ctx).await)
                }
            };
        }
        Commands::Workflow(cmd) => match cmd.subcommand {
            WorkflowSubcommand::Push {
                name,
                workflows_dir,
            } => {
                let base = workflows_dir.unwrap_or_else(specs::default_workflows_dir);
                let workflow_dir = base.join(&name);

                let max_retries = 10;
                let mut retry_count = 0;
                loop {
                    let (mut bundle, version_path, format) =
                        specs::load_workflow_from_dir(&workflow_dir)?;

                    if let Some(meta) = bundle.get_mut("meta").and_then(|m| m.as_object_mut()) {
                        meta.remove("id");
                        meta.remove("_id");
                    }

                    let payload = serde_json::json!({
                        "authorBundle": bundle,
                        "sourceFormat": format.as_str(),
                    });

                    match api::create_workflow_definition(&payload).await? {
                        api::PushResult::Success(id) => {
                            if !id.is_empty() {
                                specs::write_workflow_id(&version_path, format, &id)?;
                                println!("Pushed workflow definition: {} (id: {})", name, id);
                            } else {
                                println!("Pushed workflow definition: {} successfully", name);
                            }
                            break;
                        }
                        api::PushResult::Conflict => {
                            retry_count += 1;
                            if retry_count >= max_retries {
                                return Err(format!(
                                    "Failed to push workflow: conflict (DuplicateDefinition) persisted after {} retries.",
                                    max_retries
                                ).into());
                            }
                            let next_version =
                                specs::increment_workflow_version(&version_path, format)?;
                            println!(
                                "Conflict: version already exists. Automatically bumped version to {} and retrying...",
                                next_version
                            );
                        }
                    }
                }
            }
            WorkflowSubcommand::Pull {
                name,
                version,
                workflows_dir,
            } => {
                let base = workflows_dir.unwrap_or_else(specs::default_workflows_dir);
                let workflow_dir = base.join(&name);
                let slug = specs::workflow_api_slug(&workflow_dir, &name);

                let res = api::get_workflow_definition(&slug, version).await?;
                let data = res.get("data").ok_or("get-workflow response has no data")?;
                specs::write_workflow_from_definition(&workflow_dir, data)?;
                println!("Pulled workflow: {}", slug);
            }
            WorkflowSubcommand::Validate {
                name,
                workflows_dir,
            } => {
                let ctx = CliContext {
                    mode: OutputMode::from_json_flag(json),
                    command: "workflow validate",
                };
                return Ok(workflow_cmd::run_workflow_validate(
                    &ctx,
                    &name,
                    workflows_dir,
                ));
            }
        },
        Commands::Sync(cmd) => {
            let ctx = CliContext {
                mode: OutputMode::from_json_flag(json),
                command: "sync",
            };
            let opts = sync_options_from_cmd(
                cmd.assistants_dir,
                cmd.agents_dir,
                cmd.tools_dir,
                cmd.workflows_dir,
                cmd.hitl_dir,
            );
            if ctx.mode == OutputMode::Human {
                println!("Syncing from API...");
            }
            return Ok(run_sync_command(&ctx, opts).await);
        }
        Commands::Env(cmd) => match cmd.subcommand {
            EnvSubcommand::Push {
                service_type,
                description,
            } => {
                let ctx = CliContext {
                    mode: OutputMode::from_json_flag(json),
                    command: "env push",
                };
                return Ok(env_cmd::run_env_push(&ctx, &service_type, description).await);
            }
        },
    }

    Ok(ExitCode::SUCCESS)
}
