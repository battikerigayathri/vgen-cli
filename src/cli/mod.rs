use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "vgen")]
#[command(about = "ResMate CLI for tools, agents, assistants, and HITL", long_about = None)]
pub struct Cli {
    /// Emit machine-readable JSON envelope on stdout; errors on stderr in human mode only
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Workspace detection and artifact summary
    Workspace(WorkspaceCmd),
    /// Environment and connectivity diagnostics
    Doctor(DoctorCmd),
    /// Dependency link graph for assistants, agents, tools, HITL, and workflows
    Graph(GraphCmd),
    /// Aggregate workspace validation (graph, artifacts, workflows, secrets)
    Validate(ValidateCmd),
    /// Compare local artifact vs platform record
    Diff(DiffCmd),
    /// Explain a stable error/finding code
    Explain(ExplainCmd),
    /// Bootstrap a ResMate workspace (empty or allowlisted directory)
    Init(InitCmd),
    /// Scaffold a workspace from a recipe template
    Scaffold(ScaffoldCmd),
    /// Manage the authoring kit (skills, rules, docs, AGENTS.md)
    Kit(KitCmd),
    /// Plan or execute batch push in dependency order
    PushAll(PushAllCmd),
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
    /// Manage workflows
    Workflow(WorkflowCmd),
    /// Sync assistants, workflows, HITL, agents, and tools from the API
    Sync(SyncCmd),
    /// Manage environment variables and secrets
    Env(EnvCmd),
}

#[derive(Parser)]
pub struct ToolCmd {
    #[command(subcommand)]
    pub subcommand: ToolSubcommand,
}

#[derive(Subcommand)]
pub enum ToolSubcommand {
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
        /// Directory containing tool folders (default: tools, or VGEN_TOOLS_DIR)
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
    /// Test a JS or FaaS tool.
    Test {
        /// Tool name (folder name under tools dir)
        name: String,
        /// Directory containing tool folders (default: tools, or VGEN_TOOLS_DIR)
        #[arg(long)]
        tools_dir: Option<std::path::PathBuf>,
        /// Path to custom payload JSON file (defaults to <tool-dir>/payload.json)
        #[arg(long)]
        payload: Option<std::path::PathBuf>,
        /// Enable integration test mode (using /debug/execute-skill)
        #[arg(long)]
        integration: bool,
        /// Optional real session ID (sets roc-session header only when provided; omitted by default)
        #[arg(long)]
        session: Option<String>,
        /// Optional skill configuration ID (overrides YAML tool ID)
        #[arg(long)]
        skill_id: Option<String>,
        /// Skip pulling the latest tool from API before testing
        #[arg(long)]
        no_pull: bool,
    },
}

#[derive(Parser)]
pub struct AgentCmd {
    #[command(subcommand)]
    pub subcommand: AgentSubcommand,
}

#[derive(Subcommand)]
pub enum AgentSubcommand {
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
pub struct AssistantCmd {
    #[command(subcommand)]
    pub subcommand: AssistantSubcommand,
}

#[derive(Subcommand)]
pub enum AssistantSubcommand {
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
    /// Test an assistant using prompt.json from assistants/<name>/prompt.json.
    Test {
        /// Assistant name (filename without extension under assistants dir)
        name: String,
        #[arg(long)]
        assistants_dir: Option<std::path::PathBuf>,
        /// Force create a new session instead of reusing the existing one
        #[arg(long)]
        new_session: bool,
    },
    /// Start a multi-turn interactive chat session with the assistant
    Chat {
        /// Assistant name (filename without extension under assistants dir)
        name: String,
        /// Directory containing assistant folders (default: assistants, or VGEN_ASSISTANTS_DIR)
        #[arg(long)]
        assistants_dir: Option<std::path::PathBuf>,
        /// Force create a new session instead of reusing the existing one
        #[arg(long)]
        new_session: bool,
    },
}

#[derive(Parser)]
pub struct HitlCmd {
    #[command(subcommand)]
    pub subcommand: HitlSubcommand,
}

#[derive(Subcommand)]
pub enum HitlSubcommand {
    /// Push HITL from folder (hitl/<name>/ with config.json and meta.yaml). Create if no id in meta, update if id present; writes id back to meta after create.
    Push {
        /// HITL name (folder name under hitl dir)
        name: String,
        /// Directory containing HITL folders (default: hitl, or VGEN_HITL_DIR)
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
pub struct ConfigCmd {
    #[command(subcommand)]
    pub subcommand: ConfigSubcommand,
}

#[derive(Subcommand)]
pub enum ConfigSubcommand {
    /// Show current configuration (without secrets)
    Show,
    /// Validate base URL and API key with a test request
    Validate,
}

#[derive(Parser)]
pub struct WorkflowCmd {
    #[command(subcommand)]
    pub subcommand: WorkflowSubcommand,
}

#[derive(Subcommand)]
pub enum WorkflowSubcommand {
    /// Push workflow definition from folder
    Push {
        /// Workflow name (folder name under workflows dir)
        name: String,
        /// Directory containing workflow folders (default: workflows, or VGEN_WORKFLOWS_DIR)
        #[arg(long)]
        workflows_dir: Option<std::path::PathBuf>,
    },
    /// Pull workflow definition from API into workflows/<name>/
    Pull {
        /// Workflow name (folder name under workflows dir)
        name: String,
        /// Specific version to pull (optional, pulls latest version if omitted)
        #[arg(long)]
        version: Option<u32>,
        /// Directory containing workflow folders (default: workflows, or VGEN_WORKFLOWS_DIR)
        #[arg(long)]
        workflows_dir: Option<std::path::PathBuf>,
    },
    /// Validate workflow definition locally (schema + semantics, including transitions, dead-end checks, path-aware completeness via requiredFromStage, reset on rework-loop transitions — see spec-workflow.md §2.4.3; named gates via gates.yaml with composed predicates and cycle detection — see §2.4.5; no API)
    Validate {
        /// Workflow name (folder name under workflows dir)
        name: String,
        /// Directory containing workflow folders (default: workflows, or VGEN_WORKFLOWS_DIR)
        #[arg(long)]
        workflows_dir: Option<std::path::PathBuf>,
    },
}

#[derive(Parser)]
pub struct SyncCmd {
    /// Directory containing assistant YAML files (default: assistants, or VGEN_ASSISTANTS_DIR)
    #[arg(long)]
    pub assistants_dir: Option<std::path::PathBuf>,
    /// Directory containing agent YAML files (default: agents, or VGEN_AGENTS_DIR)
    #[arg(long)]
    pub agents_dir: Option<std::path::PathBuf>,
    /// Directory containing tool folders (default: tools, or VGEN_TOOLS_DIR)
    #[arg(long)]
    pub tools_dir: Option<std::path::PathBuf>,
    /// Directory containing workflow folders (default: workflows, or VGEN_WORKFLOWS_DIR)
    #[arg(long)]
    pub workflows_dir: Option<std::path::PathBuf>,
    /// Directory containing HITL folders (default: hitl, or VGEN_HITL_DIR)
    #[arg(long)]
    pub hitl_dir: Option<std::path::PathBuf>,
}

#[derive(Parser)]
pub struct ExplainCmd {
    /// Error or finding code to explain (e.g. BROKEN_AGENT_REF)
    pub code: Option<String>,
    /// List all known error codes
    #[arg(long)]
    pub list: bool,
    /// Filter --list to one domain (config, validation, workflow, push)
    #[arg(long)]
    pub domain: Option<String>,
}

#[derive(Parser)]
pub struct InitCmd {
    /// Project name for vgen.yaml
    #[arg(long)]
    pub name: Option<String>,
    /// Description for vgen.yaml (default: "ResMate use case workspace")
    #[arg(long)]
    pub description: Option<String>,
    /// Bootstrap-overwrite kit/seed files in a non-init-safe directory (never
    /// deletes live artifacts). To refresh an existing use-case repo, prefer
    /// `vgen kit update`.
    #[arg(long)]
    pub force: bool,
    /// Skip copying the examples/ reference tree
    #[arg(long)]
    pub no_examples: bool,
}

#[derive(Parser)]
pub struct ScaffoldCmd {
    /// Recipe name (oracle-pr, minimal, form-wizard)
    pub recipe: String,
    /// Project slug for vgen.yaml and artifact names
    #[arg(long)]
    pub name: Option<String>,
    /// Overwrite existing files
    #[arg(long)]
    pub force: bool,
    /// Run init (authoring kit) before scaffolding when kit is missing
    #[arg(long)]
    pub with_kit: bool,
}

#[derive(Parser)]
pub struct KitCmd {
    #[command(subcommand)]
    pub subcommand: KitSubcommand,
}

#[derive(Subcommand)]
pub enum KitSubcommand {
    /// Refresh the authoring kit files (skills, rules, docs, AGENTS.md) in an existing workspace
    #[command(alias = "refresh")]
    Update {
        /// Also refresh the examples/ reference tree
        #[arg(long)]
        examples: bool,
        /// List paths that would be written without modifying the filesystem
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Parser)]
pub struct WorkspaceCmd {
    #[command(subcommand)]
    pub subcommand: WorkspaceSubcommand,
}

#[derive(Subcommand)]
pub enum WorkspaceSubcommand {
    /// Show workspace root, artifact directories, counts, and config summary
    Info,
}

#[derive(Parser)]
pub struct DoctorCmd {
    /// Skip API connectivity check
    #[arg(long)]
    pub offline: bool,
}

#[derive(Parser)]
pub struct GraphCmd {
    /// Limit graph to the subtree reachable from one assistant (file stem, slug, or id)
    #[arg(long)]
    pub assistant: Option<String>,
}

#[derive(Parser)]
pub struct ValidateCmd {
    /// Treat warnings as validation failures (exit 3)
    #[arg(long)]
    pub strict: bool,
    /// Skip remote API checks (filesystem validation only)
    #[arg(long)]
    pub offline: bool,
    /// Verify platform IDs exist via authenticated API lookup
    #[arg(long)]
    pub remote: bool,
}

#[derive(Parser)]
pub struct DiffCmd {
    /// Resource type: tool, agent, assistant, hitl, workflow
    pub resource_type: String,
    /// Resource folder or file stem name
    pub name: String,
}

#[derive(Parser)]
pub struct PushAllCmd {
    /// Print push plan without calling API or writing files
    #[arg(long)]
    pub dry_run: bool,
    /// Skip interactive confirmation (required for execute in JSON mode)
    #[arg(long)]
    pub yes: bool,
    /// Push resources with validation blockers (risky)
    #[arg(long)]
    pub force: bool,
    /// Stop on first failed step (default true)
    #[arg(long, default_value_t = true)]
    pub stop_on_error: bool,
    /// Limit plan to the subgraph reachable from one assistant (file stem, slug, or id)
    #[arg(long)]
    pub assistant: Option<String>,
}

#[derive(Parser)]
pub struct EnvCmd {
    #[command(subcommand)]
    pub subcommand: EnvSubcommand,
}

#[derive(Subcommand)]
pub enum EnvSubcommand {
    /// Securely push mapped secrets to Smriti's bulk secrets manager
    Push {
        /// Secret service type (aws_secrets_manager)
        #[arg(long, default_value = "aws_secrets_manager")]
        service_type: String,
        /// Optional description for the pushed secrets
        #[arg(long)]
        description: Option<String>,
    },
}
