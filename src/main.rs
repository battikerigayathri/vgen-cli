mod api;
mod auth;
mod config;
mod http_client;
mod specs;
mod store;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "vgen")]
#[command(about = "vgen CLI for tools, agents, assistants, and HITL", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage tools
    Tool(ToolCmd),
    /// Manage agents
    Agent(AgentCmd),
    /// Manage assistants
    Assistant(AssistantCmd),
    /// Manage HITL (Human In The Loop) configs
    Hitl(HitlCmd),
    /// Configuration and connection validation
    Config(ConfigCmd),
}

#[derive(Parser)]
struct ToolCmd {
    #[command(subcommand)]
    subcommand: ToolSubcommand,
}

#[derive(Subcommand)]
enum ToolSubcommand {
    /// Create a new tool (JSON body file)
    Create {
        /// Slug or name to store the tool id under
        #[arg(long)]
        slug: String,
        /// Path to JSON file with tool create body
        #[arg(long)]
        body_file: std::path::PathBuf,
    },
    /// Update an existing tool (id from store by slug or in body)
    Update {
        #[arg(long)]
        slug: String,
        #[arg(long)]
        body_file: std::path::PathBuf,
    },
    /// Push tool from folder (tools/<name>/ with tool.yaml, handler, package.json). Create if no id in YAML, update if id present; writes id back to YAML after create.
    Push {
        /// Tool name (folder name under tools dir)
        name: String,
        /// Directory containing tool folders (default: tools, or vgen_TOOLS_DIR)
        #[arg(long)]
        tools_dir: Option<std::path::PathBuf>,
    },
    /// Pull tool from API into tools/<name>/ (YAML, handler, package.json). Requires id in tool YAML.
    Pull {
        /// Tool name (folder name under tools dir)
        name: String,
        #[arg(long)]
        tools_dir: Option<std::path::PathBuf>,
    },
    /// Test a FaaS tool using payload.json from its folder.
    Test {
        /// Tool name (folder name under tools dir)
        name: String,
        /// Directory containing tool folders (default: tools, or vgen_TOOLS_DIR)
        #[arg(long)]
        tools_dir: Option<std::path::PathBuf>,
    },
}

#[derive(Parser)]
struct AgentCmd {
    #[command(subcommand)]
    subcommand: AgentSubcommand,
}

#[derive(Subcommand)]
enum AgentSubcommand {
    /// Create a new agent
    Create {
        #[arg(long)]
        slug: String,
        #[arg(long)]
        body_file: std::path::PathBuf,
    },
    /// Update an existing agent (record id from store by slug or --id)
    Update {
        #[arg(long)]
        slug: String,
        #[arg(long)]
        body_file: std::path::PathBuf,
        /// Record id (overrides slug lookup from store)
        #[arg(long)]
        id: Option<String>,
    },
    /// Push agent from YAML file (agents/name.yaml or .yml). Create if no id, update if id present; writes id back to YAML after create.
    Push {
        /// Agent name (filename without extension under agents dir)
        name: String,
        #[arg(long)]
        agents_dir: Option<std::path::PathBuf>,
    },
    /// Pull agent from API into agents/<name>.yaml. Requires id in agent YAML.
    Pull {
        name: String,
        #[arg(long)]
        agents_dir: Option<std::path::PathBuf>,
    },
}

#[derive(Parser)]
struct AssistantCmd {
    #[command(subcommand)]
    subcommand: AssistantSubcommand,
}

#[derive(Subcommand)]
enum AssistantSubcommand {
    /// Create a new assistant
    Create {
        #[arg(long)]
        slug: String,
        #[arg(long)]
        body_file: std::path::PathBuf,
    },
    /// Update an existing assistant (record id from store by slug or --id)
    Update {
        #[arg(long)]
        slug: String,
        #[arg(long)]
        body_file: std::path::PathBuf,
        #[arg(long)]
        id: Option<String>,
    },
    /// Push assistant from YAML file (assistants/name.yaml or .yml). Create if no id, update if id present; writes id back to YAML after create.
    Push {
        /// Assistant name (filename without extension under assistants dir)
        name: String,
        #[arg(long)]
        assistants_dir: Option<std::path::PathBuf>,
    },
    /// Pull assistant from API into assistants/<name>.yaml. Requires id in assistant YAML.
    Pull {
        name: String,
        #[arg(long)]
        assistants_dir: Option<std::path::PathBuf>,
    },
}

#[derive(Parser)]
struct HitlCmd {
    #[command(subcommand)]
    subcommand: HitlSubcommand,
}

#[derive(Subcommand)]
enum HitlSubcommand {
    /// Push HITL from folder (hitl/<name>/ with config.json and meta.yaml). Create if no id in meta, update if id present; writes id back to meta after create.
    Push {
        /// HITL name (folder name under hitl dir)
        name: String,
        /// Directory containing HITL folders (default: hitl, or vgen_HITL_DIR)
        #[arg(long)]
        hitl_dir: Option<std::path::PathBuf>,
    },
    /// Pull HITL from API into hitl/<name>/ (config.json and meta.yaml). Requires id in meta.yaml.
    Pull {
        /// HITL name (folder name under hitl dir)
        name: String,
        #[arg(long)]
        hitl_dir: Option<std::path::PathBuf>,
    },
}

#[derive(Parser)]
struct ConfigCmd {
    #[command(subcommand)]
    subcommand: ConfigSubcommand,
}

#[derive(Subcommand)]
enum ConfigSubcommand {
    /// Show current configuration (without secrets)
    Show,
    /// Validate base URL and API key with a test request
    Validate,
}

fn read_json_file(
    path: &std::path::Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let s = std::fs::read_to_string(path)?;
    let v: serde_json::Value = serde_json::from_str(&s)?;
    Ok(v)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Load .env from cwd so vgen_SECRET and others are available
    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    match cli.command {
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
                    let id = s
                        .get_tool_id(&slug)
                        .ok_or_else(|| format!("No tool id for slug '{}'. Create first or add id to body.", slug))?;
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
                let data = res
                    .get("data")
                    .ok_or("get-record response has no data")?;
                specs::write_tool_from_record(&tool_dir, data)?;
                println!("Pulled tool: {}", name);
            }
            ToolSubcommand::Test { name, tools_dir } => {
                let base = tools_dir.unwrap_or_else(specs::default_tools_dir);
                let tool_dir = base.join(&name);

                let (yaml, _) = specs::load_tool_yaml(&tool_dir)?;

                let tool_type = yaml.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if !tool_type.eq_ignore_ascii_case("faas") {
                    return Err(format!("Tool '{}' is of type '{}'. Test command only supports 'FaaS' tools.", name, tool_type).into());
                }

                let function_id = yaml.get("functionId").and_then(|v| v.as_str()).ok_or_else(|| "No functionId found in tool.yaml. Please ensure it's a valid FaaS tool.")?;

                let payload_path = tool_dir.join("payload.json");
                if !payload_path.exists() {
                    return Err(format!("payload.json not found in {}. It is required for testing.", tool_dir.display()).into());
                }

                let mut payload = read_json_file(&payload_path)?;
                if let Some(obj) = payload.as_object_mut() {
                    obj.insert("function_id".to_string(), serde_json::Value::String(function_id.to_string()));
                } else {
                    return Err("payload.json must contain a JSON object".into());
                }

                let result = api::test_faas_tool(&payload).await?;
                println!("{}", serde_json::to_string_pretty(&result)?);
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
                let record_id = override_id.or_else(|| {
                    store::IdStore::load()
                        .ok()
                        .and_then(|s| s.get_agent_id(&slug).map(String::from))
                }).ok_or_else(|| format!("No agent id for slug '{}'. Use --id or create first.", slug))?;
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
                    .ok_or_else(|| format!("No id in agents/{}.yaml; push first or add id.", name))?;
                let res = api::get_record("agentConfig", id).await?;
                let data = res
                    .get("data")
                    .ok_or("get-record response has no data")?;
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
                let record_id = override_id.or_else(|| {
                    store::IdStore::load()
                        .ok()
                        .and_then(|s| s.get_assistant_id(&slug).map(String::from))
                }).ok_or_else(|| format!("No assistant id for slug '{}'. Use --id or create first.", slug))?;
                let id = api::update_assistant(&record_id, &document).await?;
                let mut s = store::IdStore::load()?;
                s.set_assistant_id(slug, id.clone());
                s.save()?;
                println!("Updated assistant: {}", id);
            }
            AssistantSubcommand::Push { name, assistants_dir } => {
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
            AssistantSubcommand::Pull { name, assistants_dir } => {
                let base = assistants_dir.unwrap_or_else(specs::default_assistants_dir);
                let (body, yaml_path) = specs::load_assistant(&base, &name)?;
                let id = body
                    .get("id")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| format!("No id in assistants/{}.yaml; push first or add id.", name))?;
                let res = api::get_record("chat", id).await?;
                let data = res
                    .get("data")
                    .ok_or("get-record response has no data")?;
                specs::write_assistant_yaml_from_record(&yaml_path, data)?;
                println!("Pulled assistant: {}", name);
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
                let data = res
                    .get("data")
                    .ok_or("get-record response has no data")?;
                specs::write_hitl_from_record(&hitl_dir_path, data)?;
                println!("Pulled HITL: {}", name);
            }
        },
        Commands::Config(cmd) => match cmd.subcommand {
            ConfigSubcommand::Show => config::show_config(),
            ConfigSubcommand::Validate => http_client::validate_connection().await?,
        },
    }

    Ok(())
}
