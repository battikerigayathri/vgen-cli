use crate::PlannedTask;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillRef {
    pub agent_slug: String,
    pub skill_key: String,
}

impl SkillRef {
    pub fn tool_id(&self) -> String {
        format!("{}__{}", self.agent_slug, self.skill_key)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRecord {
    pub skill_ref: SkillRef,
    pub tool_id: String,
    pub mongo_skill_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_version: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SkillResolveError {
    #[error("agent '{agent_slug}' not in catalog")]
    AgentNotInCatalog { agent_slug: String },
    #[error("assigned agent '{assigned_agent}' could not be resolved")]
    AgentUnresolved { assigned_agent: String },
    #[error("skill '{skill_key}' not found on agent '{agent_slug}'")]
    SkillNotInCatalog {
        agent_slug: String,
        skill_key: String,
    },
    #[error("invalid tool_id '{tool_id}': {reason}")]
    InvalidToolId { tool_id: String, reason: String },
    #[error("invalid arguments for tool '{tool_id}': {detail}")]
    ArgumentsInvalid { tool_id: String, detail: String },
}

impl SkillResolveError {
    pub fn as_log_str(&self) -> String {
        self.to_string()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillCatalog {
    #[serde(default)]
    pub agents: Vec<SkillCatalogAgent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillCatalogAgent {
    pub slug: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_context: Option<String>,
    pub tools: Vec<SkillRecord>,
}

#[derive(Debug, Clone)]
struct AgentIndex {
    by_key: HashMap<String, SkillRecord>,
    by_key_lower: HashMap<String, SkillRecord>,
    by_mongo_id: HashMap<String, SkillRecord>,
}

#[derive(Debug, Clone, Default)]
struct CatalogIndex {
    by_slug: HashMap<String, AgentIndex>,
    by_name: HashMap<String, String>,
    by_name_lower: HashMap<String, String>,
}

fn extract_mongo_skill_id(skill: &Value) -> Option<String> {
    skill
        .get("_id")
        .and_then(|id| id.get("$oid").and_then(|v| v.as_str()).map(String::from))
        .or_else(|| skill.get("id").and_then(|v| v.as_str()).map(String::from))
        .or_else(|| skill.get("_id").and_then(|v| v.as_str()).map(String::from))
}

fn skill_parameters(skill: &Value) -> Option<Value> {
    let raw = skill
        .get("arguments")
        .or_else(|| skill.get("parameters"))
        .or_else(|| skill.get("inputArguments"));
    normalize_skill_parameters(raw)
}

fn is_flat_faas_argument_map(obj: &serde_json::Map<String, Value>) -> bool {
    !obj.is_empty()
        && obj.values().all(|v| {
            v.as_object()
                .and_then(|o| o.get("type"))
                .is_some_and(|t| t.is_string())
        })
}

fn flat_map_to_json_schema(map: &serde_json::Map<String, Value>) -> Value {
    use serde_json::Map;
    let mut properties = Map::new();
    let mut required = Vec::new();
    for (key, field_def) in map {
        let Some(mut field_obj) = field_def.as_object().cloned() else {
            continue;
        };
        let is_required = field_obj
            .remove("required")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if is_required {
            required.push(Value::String(key.clone()));
        }
        properties.insert(
            key.clone(),
            normalize_strict_json_schema(Value::Object(field_obj)),
        );
    }
    let mut schema = json!({
        "type": "object",
        "properties": properties,
        "additionalProperties": false
    });
    if !required.is_empty() {
        schema
            .as_object_mut()
            .unwrap()
            .insert("required".into(), Value::Array(required));
    }
    schema
}

fn array_to_json_schema(items: &[Value]) -> Value {
    use serde_json::Map;
    let mut properties = Map::new();
    let mut required = Vec::new();
    for item in items {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let Some(name) = obj.get("name").and_then(|n| n.as_str()) else {
            continue;
        };
        let mut prop = obj.clone();
        prop.remove("name");
        let is_required = prop
            .remove("required")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if is_required {
            required.push(Value::String(name.to_string()));
        }
        properties.insert(
            name.to_string(),
            normalize_strict_json_schema(Value::Object(prop)),
        );
    }
    let mut schema = json!({
        "type": "object",
        "properties": properties,
        "additionalProperties": false
    });
    if !required.is_empty() {
        schema
            .as_object_mut()
            .unwrap()
            .insert("required".into(), Value::Array(required));
    }
    schema
}

/// Convert Mongo/FaaS skill `arguments` into JSON Schema for validation and LLM prompts.
///
/// Accepts standard JSON Schema (`type: object`, `properties`, `required`), flat FaaS maps
/// (`{ field: { type, required, description } }`), and optional array form
/// (`[{ name, type, required }]`).
pub fn normalize_skill_parameters(raw: Option<&Value>) -> Option<Value> {
    let raw = raw?;
    if raw.is_null() {
        return None;
    }
    if let Some(arr) = raw.as_array() {
        return Some(normalize_strict_json_schema(array_to_json_schema(arr)));
    }
    let Some(obj) = raw.as_object() else {
        return Some(normalize_strict_json_schema(raw.clone()));
    };
    if obj.get("properties").is_some() {
        let mut schema = raw.clone();
        if schema.get("type").is_none() {
            schema
                .as_object_mut()
                .unwrap()
                .insert("type".into(), json!("object"));
        }
        return Some(normalize_strict_json_schema(schema));
    }
    if is_flat_faas_argument_map(obj) {
        return Some(normalize_strict_json_schema(flat_map_to_json_schema(obj)));
    }
    Some(normalize_strict_json_schema(raw.clone()))
}

/// Normalized parameter schema for a catalog record (handles persisted raw FaaS shapes).
pub fn effective_parameters(record: &SkillRecord) -> Option<Value> {
    normalize_skill_parameters(record.parameters.as_ref())
}

fn merge_parameter_properties(into: &mut serde_json::Map<String, Value>, params: Option<&Value>) {
    let Some(params) = normalize_skill_parameters(params) else {
        return;
    };
    let Some(props) = params.get("properties").and_then(|p| p.as_object()) else {
        return;
    };
    for (key, prop_schema) in props {
        into.entry(key.clone())
            .or_insert_with(|| normalize_strict_json_schema(prop_schema.clone()));
    }
}

/// Ensure nested object nodes satisfy OpenAI strict-mode requirements.
fn normalize_strict_json_schema(schema: Value) -> Value {
    let Some(mut obj) = schema.as_object().cloned() else {
        return schema;
    };
    match obj.get("type").and_then(|t| t.as_str()) {
        Some("object") => {
            if !obj.contains_key("additionalProperties") {
                obj.insert("additionalProperties".into(), json!(false));
            }
            if let Some(props) = obj.get_mut("properties").and_then(|p| p.as_object_mut()) {
                for v in props.values_mut() {
                    *v = normalize_strict_json_schema(v.clone());
                }
            }
        }
        Some("array") => {
            if let Some(items) = obj.get("items").cloned() {
                obj.insert("items".into(), normalize_strict_json_schema(items));
            }
            if let Some(prefix) = obj.get("prefixItems").and_then(|p| p.as_array()).cloned() {
                let normalized: Vec<Value> = prefix
                    .into_iter()
                    .map(normalize_strict_json_schema)
                    .collect();
                obj.insert("prefixItems".into(), Value::Array(normalized));
            }
        }
        _ => {}
    }
    Value::Object(obj)
}

/// OpenAI strict structured output requires `required` to list **every** key in `properties`.
/// Semantic optional/required for runtime validation stays in the normalized parameter schema;
/// this helper is only for LLM response_format object nodes.
pub fn apply_openai_strict_object_required(schema: &mut Value) {
    let Some(obj) = schema.as_object_mut() else {
        return;
    };
    if obj.get("type").and_then(|t| t.as_str()) != Some("object") {
        return;
    }
    if !obj.contains_key("additionalProperties") {
        obj.insert("additionalProperties".into(), json!(false));
    }
    let required: Vec<Value> = obj
        .get("properties")
        .and_then(|p| p.as_object())
        .map(|props| props.keys().map(|k| json!(k)).collect())
        .unwrap_or_default();
    obj.insert("required".into(), Value::Array(required));
}

/// Normalize task/skill input to a JSON object for argument validation.
pub fn normalize_argument_value(input: &Value) -> Value {
    if input.is_object() {
        return input.clone();
    }
    if let Some(s) = input.as_str()
        && let Ok(v) = serde_json::from_str::<Value>(s)
        && v.is_object()
    {
        return v;
    }
    json!({})
}

impl SkillCatalog {
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }

    pub fn from_agents(agents: &[Value]) -> Self {
        let mut catalog_agents = Vec::new();

        for agent in agents {
            let Some(slug) = agent.get("slug").and_then(|s| s.as_str()) else {
                continue;
            };
            let name = agent
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or(slug)
                .to_string();

            let skills_json = agent
                .get("skills")
                .and_then(|s| s.as_array())
                .cloned()
                .unwrap_or_default();

            let mut tools = Vec::new();
            for skill in skills_json {
                let Some(skill_key) = skill.get("name").and_then(|n| n.as_str()) else {
                    continue;
                };
                let mongo_skill_id =
                    extract_mongo_skill_id(&skill).unwrap_or_else(|| skill_key.to_string());
                let skill_ref = SkillRef {
                    agent_slug: slug.to_string(),
                    skill_key: skill_key.to_string(),
                };
                let tool_id = skill_ref.tool_id();
                tools.push(SkillRecord {
                    skill_ref: skill_ref.clone(),
                    tool_id,
                    mongo_skill_id,
                    skill_version: skill
                        .get("version")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    name: skill_key.to_string(),
                    skill_type: skill.get("type").and_then(|v| v.as_str()).map(String::from),
                    description: skill
                        .get("description")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    parameters: skill_parameters(&skill),
                });
            }

            catalog_agents.push(SkillCatalogAgent {
                slug: slug.to_string(),
                name,
                description: agent
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                agent_context: agent
                    .get("agentContext")
                    .or_else(|| agent.get("agent_context"))
                    .and_then(|v| v.as_str())
                    .map(String::from),
                tools,
            });
        }

        Self {
            agents: catalog_agents,
        }
    }

    /// Build catalog including only agents whose slug is in `allowed_slugs`.
    pub fn from_agents_filtered(agents: &[Value], allowed_slugs: &HashSet<String>) -> Self {
        if allowed_slugs.is_empty() {
            return Self::from_agents(agents);
        }
        let filtered: Vec<Value> = agents
            .iter()
            .filter(|agent| {
                agent
                    .get("slug")
                    .and_then(|s| s.as_str())
                    .is_some_and(|slug| allowed_slugs.contains(slug))
            })
            .cloned()
            .collect();
        Self::from_agents(&filtered)
    }

    /// Prefer persisted catalog; else filtered rebuild from workflow authz fields.
    pub fn from_workflow_state(state: &crate::WorkflowState) -> Self {
        if !state.skill_catalog.is_empty() {
            return state.skill_catalog.clone();
        }
        if let Some(agents) = &state.kriya_agents {
            let allowed = state.active_agent_set();
            if !allowed.is_empty() {
                return Self::from_agents_filtered(agents, &allowed);
            }
            return Self::from_agents(agents);
        }
        Self::default()
    }

    fn build_index(&self) -> CatalogIndex {
        let mut index = CatalogIndex::default();

        for agent in &self.agents {
            let mut by_key = HashMap::new();
            let mut by_key_lower = HashMap::new();
            let mut by_mongo_id = HashMap::new();

            for tool in &agent.tools {
                by_key.insert(tool.name.clone(), tool.clone());
                by_key_lower
                    .entry(tool.name.to_ascii_lowercase())
                    .or_insert_with(|| tool.clone());
                by_mongo_id
                    .entry(tool.mongo_skill_id.clone())
                    .or_insert_with(|| tool.clone());
            }

            index.by_slug.insert(
                agent.slug.clone(),
                AgentIndex {
                    by_key,
                    by_key_lower,
                    by_mongo_id,
                },
            );
            index
                .by_name
                .entry(agent.name.clone())
                .or_insert_with(|| agent.slug.clone());
            index
                .by_name_lower
                .entry(agent.name.to_ascii_lowercase())
                .or_insert_with(|| agent.slug.clone());
        }

        index
    }

    fn resolve_agent_slug(&self, assigned_agent: &str) -> Result<String, SkillResolveError> {
        let index = self.build_index();
        let trimmed = assigned_agent.trim();

        if index.by_slug.contains_key(trimmed) {
            return Ok(trimmed.to_string());
        }
        if let Some(slug) = index.by_name.get(trimmed) {
            return Ok(slug.clone());
        }
        let lower = trimmed.to_ascii_lowercase();
        if let Some(slug) = index.by_name_lower.get(&lower) {
            return Ok(slug.clone());
        }

        Err(SkillResolveError::AgentUnresolved {
            assigned_agent: assigned_agent.to_string(),
        })
    }

    pub fn resolve(
        &self,
        agent_slug: &str,
        inbound_key: &str,
    ) -> Result<SkillRecord, SkillResolveError> {
        let index = self.build_index();
        let agent =
            index
                .by_slug
                .get(agent_slug)
                .ok_or_else(|| SkillResolveError::AgentNotInCatalog {
                    agent_slug: agent_slug.to_string(),
                })?;

        let key = inbound_key.trim();
        if key.is_empty() {
            return Err(SkillResolveError::SkillNotInCatalog {
                agent_slug: agent_slug.to_string(),
                skill_key: inbound_key.to_string(),
            });
        }

        if let Some(record) = agent.by_key.get(key) {
            return Ok(record.clone());
        }
        if let Some(record) = agent.by_key_lower.get(&key.to_ascii_lowercase()) {
            return Ok(record.clone());
        }
        if let Some(record) = agent.by_mongo_id.get(key) {
            return Ok(record.clone());
        }

        Err(SkillResolveError::SkillNotInCatalog {
            agent_slug: agent_slug.to_string(),
            skill_key: inbound_key.to_string(),
        })
    }

    pub fn resolve_tool_id(&self, tool_id: &str) -> Result<SkillRecord, SkillResolveError> {
        let Some((agent_slug, skill_key)) = tool_id.split_once("__") else {
            return Err(SkillResolveError::InvalidToolId {
                tool_id: tool_id.to_string(),
                reason: "missing '__' delimiter".to_string(),
            });
        };
        if agent_slug.is_empty() || skill_key.is_empty() {
            return Err(SkillResolveError::InvalidToolId {
                tool_id: tool_id.to_string(),
                reason: "empty agent_slug or skill_key".to_string(),
            });
        }
        self.resolve(agent_slug, skill_key)
    }

    pub fn resolve_assigned(
        &self,
        assigned_agent: &str,
        inbound_key: &str,
    ) -> Result<SkillRecord, SkillResolveError> {
        let agent_slug = self.resolve_agent_slug(assigned_agent)?;
        self.resolve(&agent_slug, inbound_key)
    }

    /// Resolve display name or slug to catalog agent slug (orchestrator v4 validation).
    pub fn resolve_assigned_agent_slug(
        &self,
        assigned_agent: &str,
    ) -> Result<String, SkillResolveError> {
        self.resolve_agent_slug(assigned_agent)
    }

    /// Catalog scoped to a single agent (agent planner tool selection).
    pub fn for_agent_slug(&self, agent_slug: &str) -> Self {
        let filtered: Vec<SkillCatalogAgent> = self
            .agents
            .iter()
            .filter(|a| a.slug == agent_slug)
            .cloned()
            .collect();
        Self { agents: filtered }
    }

    fn agent_display_name(&self, agent_slug: &str) -> Option<String> {
        self.agents
            .iter()
            .find(|a| a.slug == agent_slug)
            .map(|a| a.name.clone())
    }

    /// Public display name for an agent slug (Kriya blob lookup).
    pub fn agent_name_for_slug(&self, agent_slug: &str) -> Option<String> {
        self.agent_display_name(agent_slug)
    }

    /// Resolve by skill key when agent hint is missing (catalog is already session-scoped).
    pub fn resolve_skill_key(&self, inbound_key: &str) -> Result<SkillRecord, SkillResolveError> {
        let key = inbound_key.trim();
        if key.is_empty() {
            return Err(SkillResolveError::SkillNotInCatalog {
                agent_slug: String::new(),
                skill_key: inbound_key.to_string(),
            });
        }
        let mut matches = Vec::new();
        for agent in &self.agents {
            if let Ok(record) = self.resolve(&agent.slug, key) {
                matches.push(record);
            }
        }
        match matches.len() {
            0 => Err(SkillResolveError::SkillNotInCatalog {
                agent_slug: String::new(),
                skill_key: key.to_string(),
            }),
            1 => Ok(matches.into_iter().next().unwrap()),
            _ => Err(SkillResolveError::InvalidToolId {
                tool_id: key.to_string(),
                reason: "skill key matches multiple agents in catalog".to_string(),
            }),
        }
    }

    /// Best-effort resolve: tool_id → assigned_agent+skill → skill key scan (only when agent hint absent).
    pub fn resolve_for_execution(
        &self,
        assigned_agent: &str,
        inbound_key: &str,
        tool_id: Option<&str>,
    ) -> Result<SkillRecord, SkillResolveError> {
        if let Some(tid) = tool_id.filter(|s| !s.is_empty()) {
            return self.resolve_tool_id(tid);
        }
        if !assigned_agent.trim().is_empty() {
            return self.resolve_assigned(assigned_agent, inbound_key);
        }
        self.resolve_skill_key(inbound_key)
    }

    pub fn normalize_task_from_tool_id(
        &self,
        task: &mut PlannedTask,
    ) -> Result<SkillRecord, SkillResolveError> {
        let tool_id = task
            .tool_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| SkillResolveError::InvalidToolId {
                tool_id: String::new(),
                reason: "tool_id is required for normalization".to_string(),
            })?;
        let record = self.resolve_tool_id(tool_id)?;
        let agent_name = self
            .agent_display_name(&record.skill_ref.agent_slug)
            .unwrap_or_else(|| record.skill_ref.agent_slug.clone());
        task.assigned_agent = agent_name;
        task.skill_id = Some(record.skill_ref.skill_key.clone());
        Ok(record)
    }

    pub fn normalize_tasks_from_tool_ids(
        &self,
        tasks: &mut [PlannedTask],
    ) -> Result<(), SkillResolveError> {
        for task in tasks {
            if task.tool_id.as_deref().filter(|s| !s.is_empty()).is_some() {
                self.normalize_task_from_tool_id(task)?;
            }
        }
        Ok(())
    }

    pub fn validate_task(&self, task: &PlannedTask) -> Result<SkillRecord, SkillResolveError> {
        if let Some(tool_id) = task.tool_id.as_deref().filter(|s| !s.is_empty()) {
            let record = self.resolve_tool_id(tool_id)?;
            if !task.assigned_agent.is_empty() {
                let expected_name = self
                    .agent_display_name(&record.skill_ref.agent_slug)
                    .unwrap_or_else(|| record.skill_ref.agent_slug.clone());
                if task.assigned_agent != expected_name {
                    return Err(SkillResolveError::InvalidToolId {
                        tool_id: tool_id.to_string(),
                        reason: format!(
                            "assigned_agent '{}' conflicts with tool_id agent '{}'",
                            task.assigned_agent, expected_name
                        ),
                    });
                }
            }
            if let Some(skill_id) = task.skill_id.as_deref().filter(|s| !s.is_empty())
                && skill_id != record.skill_ref.skill_key
                && skill_id != record.mongo_skill_id
            {
                return Err(SkillResolveError::InvalidToolId {
                    tool_id: tool_id.to_string(),
                    reason: format!(
                        "skill_id '{skill_id}' conflicts with tool_id skill '{}'",
                        record.skill_ref.skill_key
                    ),
                });
            }
            return Ok(record);
        }

        let Some(skill_id) = task.skill_id.as_deref().filter(|s| !s.is_empty()) else {
            return Err(SkillResolveError::SkillNotInCatalog {
                agent_slug: task.assigned_agent.clone(),
                skill_key: String::new(),
            });
        };
        self.resolve_assigned(&task.assigned_agent, skill_id)
    }

    pub fn validate_tasks(&self, tasks: &[PlannedTask]) -> Result<(), SkillResolveError> {
        for task in tasks {
            if task.skill_id.as_deref().filter(|s| !s.is_empty()).is_some() {
                self.validate_task(task)?;
            }
        }
        Ok(())
    }

    /// Require every planned task to have a resolvable `tool_id` or `skill_id`.
    pub fn validate_plan_has_executable_refs(
        &self,
        tasks: &[PlannedTask],
    ) -> Result<(), SkillResolveError> {
        for task in tasks {
            let has_tool = task.tool_id.as_deref().filter(|s| !s.is_empty()).is_some();
            let has_skill = task.skill_id.as_deref().filter(|s| !s.is_empty()).is_some();
            if !has_tool && !has_skill {
                return Err(SkillResolveError::SkillNotInCatalog {
                    agent_slug: task.assigned_agent.clone(),
                    skill_key: String::new(),
                });
            }
            self.validate_task(task)?;
        }
        Ok(())
    }

    pub fn tool_ids(&self) -> Vec<String> {
        self.agents
            .iter()
            .flat_map(|a| a.tools.iter().map(|t| t.tool_id.clone()))
            .collect()
    }

    /// JSON Schema for LLM `input_arguments`.
    ///
    /// OpenAI strict structured output requires `additionalProperties: false` on every
    /// object — a bare `{}`-only schema cannot accept tool args. We merge explicit
    /// `properties` from each catalog tool's parameter schema so the LLM can supply
    /// known keys; runtime `validate_arguments` still enforces per-tool `required`.
    pub fn input_arguments_schema(&self) -> Value {
        use serde_json::Map;
        let mut merged = Map::new();
        for agent in &self.agents {
            for tool in &agent.tools {
                merge_parameter_properties(&mut merged, effective_parameters(tool).as_ref());
            }
        }
        let mut schema = json!({
            "type": "object",
            "description": "Arguments for the selected tool; use keys from that tool's parameter schema in the tools appendix",
            "properties": merged,
            "additionalProperties": false
        });
        apply_openai_strict_object_required(&mut schema);
        schema
    }

    /// Lightweight v1 argument validation against catalog `parameters.required`.
    pub fn validate_arguments(
        record: &SkillRecord,
        input: &Value,
    ) -> Result<(), SkillResolveError> {
        let Some(params) = effective_parameters(record) else {
            return Ok(());
        };
        let Some(required) = params.get("required").and_then(|r| r.as_array()) else {
            return Ok(());
        };
        if required.is_empty() {
            return Ok(());
        }
        let args = normalize_argument_value(input);
        for key in required {
            let Some(key_str) = key.as_str() else {
                continue;
            };
            let present = args.get(key_str).map(|v| !v.is_null()).unwrap_or(false);
            if !present {
                return Err(SkillResolveError::ArgumentsInvalid {
                    tool_id: record.tool_id.clone(),
                    detail: format!("missing required argument '{key_str}'"),
                });
            }
        }
        Ok(())
    }

    /// Per-tool JSON Schema for argument resolver LLM (Phase 7.5).
    pub fn input_arguments_schema_for_tool(record: &SkillRecord) -> Value {
        use serde_json::Map;
        let mut properties = Map::new();
        let params = effective_parameters(record);
        merge_parameter_properties(&mut properties, params.as_ref());
        if properties.is_empty()
            && let Some(params) = params.as_ref()
            && params.get("type").and_then(|t| t.as_str()) == Some("object")
        {
            let mut schema = normalize_strict_json_schema(params.clone());
            apply_openai_strict_object_required(&mut schema);
            return schema;
        }
        let mut schema = json!({
            "type": "object",
            "description": format!(
                "Arguments for tool {}; use concrete values from context, not placeholders",
                record.tool_id
            ),
            "properties": properties,
            "additionalProperties": false
        });
        apply_openai_strict_object_required(&mut schema);
        schema
    }

    /// Required parameter keys for a tool (from Mongo `arguments.required`).
    pub fn required_argument_keys(record: &SkillRecord) -> Vec<String> {
        let Some(params) = effective_parameters(record) else {
            return Vec::new();
        };
        params
            .get("required")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|k| k.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn parameter_property_keys(record: &SkillRecord) -> Vec<String> {
        let Some(params) = effective_parameters(record) else {
            return Vec::new();
        };
        params
            .get("properties")
            .and_then(|p| p.as_object())
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Human-readable parameter block for argument resolver prompt.
    pub fn format_tool_parameters_appendix(record: &SkillRecord) -> String {
        let mut lines = vec![
            format!("Tool: {}", record.tool_id),
            format!("Name: {}", record.name),
        ];
        if let Some(desc) = record.description.as_deref().filter(|d| !d.is_empty()) {
            lines.push(format!("Description: {desc}"));
        }
        let required = Self::required_argument_keys(record);
        if required.is_empty() {
            lines.push("Required arguments: (none)".to_string());
        } else {
            lines.push(format!("Required arguments: {}", required.join(", ")));
        }
        if let Some(params) = effective_parameters(record) {
            lines.push(format!("Parameter schema: {params}"));
        }
        lines.join("\n")
    }

    /// Whether argument resolution is needed before dispatch.
    pub fn arguments_need_resolution(record: &SkillRecord, input: &Value) -> bool {
        let args = normalize_argument_value(input);
        let empty = args.as_object().is_none_or(|o| o.is_empty());
        if empty {
            let has_required = !Self::required_argument_keys(record).is_empty();
            let has_properties = !Self::parameter_property_keys(record).is_empty();
            if has_required || has_properties {
                return true;
            }
        }
        Self::validate_arguments(record, input).is_err()
    }

    pub fn tools_appendix(&self) -> String {
        self.tools_appendix_truncated(usize::MAX)
    }

    /// Tools appendix truncated to `max_chars` (D7 token budget).
    pub fn tools_appendix_truncated(&self, max_chars: usize) -> String {
        let full = self.tools_appendix_inner();
        if max_chars == usize::MAX || full.len() <= max_chars {
            return full;
        }
        let mut truncated = full;
        truncated.truncate(max_chars);
        if let Some(last_newline) = truncated.rfind('\n') {
            truncated.truncate(last_newline);
        }
        truncated.push_str("\n\n[tools appendix truncated for token budget]");
        truncated
    }

    /// Agents appendix for orchestrator v4 (no tool list).
    pub fn agents_appendix(&self) -> String {
        self.agents_appendix_truncated(usize::MAX)
    }

    /// Agents appendix truncated to `max_chars`.
    pub fn agents_appendix_truncated(&self, max_chars: usize) -> String {
        let full = self.agents_appendix_inner();
        if max_chars == usize::MAX || full.len() <= max_chars {
            return full;
        }
        let mut truncated = full;
        truncated.truncate(max_chars);
        if let Some(last_newline) = truncated.rfind('\n') {
            truncated.truncate(last_newline);
        }
        truncated.push_str("\n\n[agents appendix truncated for token budget]");
        truncated
    }

    /// Capabilities appendix for assistant planner v4 — skill names/descriptions, no tool_id.
    pub fn agent_capabilities_appendix_truncated(&self, max_chars: usize) -> String {
        let full = self.agent_capabilities_appendix_inner();
        if max_chars == usize::MAX || full.len() <= max_chars {
            return full;
        }
        let mut truncated = full;
        truncated.truncate(max_chars);
        if let Some(last_newline) = truncated.rfind('\n') {
            truncated.truncate(last_newline);
        }
        truncated.push_str("\n\n[capabilities appendix truncated for token budget]");
        truncated
    }

    fn agent_capabilities_appendix_inner(&self) -> String {
        let mut lines = vec![
            "Available agents and capabilities (assign by agent name; do NOT select tool_id):"
                .to_string(),
            String::new(),
        ];
        for agent in &self.agents {
            lines.push(format!("Agent: {} (slug: {})", agent.name, agent.slug));
            if let Some(desc) = agent.description.as_deref().filter(|d| !d.is_empty()) {
                lines.push(format!("  Description: {desc}"));
            }
            if let Some(ctx) = agent.agent_context.as_deref().filter(|c| !c.is_empty()) {
                lines.push(format!("  Agent context: {ctx}"));
            }
            if agent.tools.is_empty() {
                lines.push("  Capabilities: (none listed)".to_string());
            } else {
                lines.push("  Capabilities:".to_string());
                for tool in &agent.tools {
                    let skill_name = tool.skill_ref.skill_key.as_str();
                    let desc = tool.description.as_deref().unwrap_or("");
                    lines.push(format!("    - {skill_name}: {desc}"));
                }
            }
            lines.push(String::new());
        }
        lines.join("\n")
    }

    fn agents_appendix_inner(&self) -> String {
        let mut lines = vec![
            "Available agents (assign jobs by agent name; do NOT select tools):".to_string(),
            String::new(),
        ];
        for agent in &self.agents {
            lines.push(format!("Agent: {} (slug: {})", agent.name, agent.slug));
            if let Some(desc) = agent.description.as_deref().filter(|d| !d.is_empty()) {
                lines.push(format!("  Description: {desc}"));
            }
            if let Some(ctx) = agent.agent_context.as_deref().filter(|c| !c.is_empty()) {
                lines.push(format!("  Agent context: {ctx}"));
            }
            lines.push(format!("  Skill count: {}", agent.tools.len()));
            lines.push(String::new());
        }
        lines.join("\n")
    }

    fn tools_appendix_inner(&self) -> String {
        let mut lines = vec![
            "Available tools (select ONLY by exact tool_id):".to_string(),
            String::new(),
        ];

        for agent in &self.agents {
            lines.push(format!("Agent: {} ({})", agent.slug, agent.name));
            for tool in &agent.tools {
                let type_label = tool.skill_type.as_deref().unwrap_or("JS");
                let desc = tool.description.as_deref().unwrap_or("");
                lines.push(format!("  - {} [{}]: {}", tool.tool_id, type_label, desc));
                if let Some(params) = effective_parameters(tool) {
                    lines.push(format!("    parameters: {}", params));
                }
            }
            lines.push(String::new());
        }

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PlannedTask, WORKFLOW_STATE_VERSION, WorkflowState};
    use serde_json::json;

    fn two_agent_fixture() -> Vec<Value> {
        vec![
            json!({
                "name": "API Code Generator",
                "slug": "api-gen",
                "skills": [{
                    "name": "APICodeGenerator",
                    "_id": { "$oid": "mongo-api-1" },
                    "type": "JS",
                    "description": "Generates API code",
                    "version": "1.0.0"
                }]
            }),
            json!({
                "name": "Data Analyzer",
                "slug": "data-analyzer",
                "skills": [
                    {
                        "name": "fetch_data",
                        "_id": { "$oid": "mongo-fetch-1" },
                        "type": "JS"
                    },
                    {
                        "name": "write_report",
                        "id": "mongo-report-1",
                        "type": "JS"
                    }
                ]
            }),
        ]
    }

    #[test]
    fn skill_ref_tool_id() {
        let sr = SkillRef {
            agent_slug: "api-gen".into(),
            skill_key: "APICodeGenerator".into(),
        };
        assert_eq!(sr.tool_id(), "api-gen__APICodeGenerator");
    }

    #[test]
    fn types_serde_round_trip() {
        let catalog = SkillCatalog::from_agents(&two_agent_fixture());
        let json = serde_json::to_string(&catalog).unwrap();
        let back: SkillCatalog = serde_json::from_str(&json).unwrap();
        assert_eq!(back.agents.len(), 2);
    }

    #[test]
    fn from_agents_multi_agent() {
        let catalog = SkillCatalog::from_agents(&two_agent_fixture());
        assert_eq!(catalog.agents.len(), 2);
        assert_eq!(catalog.tool_ids().len(), 3);
    }

    #[test]
    fn resolve_by_exact_name() {
        let catalog = SkillCatalog::from_agents(&two_agent_fixture());
        let record = catalog.resolve("data-analyzer", "fetch_data").unwrap();
        assert_eq!(record.name, "fetch_data");
    }

    #[test]
    fn resolve_case_insensitive() {
        let catalog = SkillCatalog::from_agents(&two_agent_fixture());
        let record = catalog.resolve("data-analyzer", "FETCH_DATA").unwrap();
        assert_eq!(record.name, "fetch_data");
    }

    #[test]
    fn resolve_by_mongo_id() {
        let catalog = SkillCatalog::from_agents(&two_agent_fixture());
        let record = catalog.resolve("data-analyzer", "mongo-fetch-1").unwrap();
        assert_eq!(record.name, "fetch_data");
    }

    #[test]
    fn resolve_by_mongo_id_oid_shape() {
        let catalog = SkillCatalog::from_agents(&two_agent_fixture());
        let record = catalog.resolve("api-gen", "mongo-api-1").unwrap();
        assert_eq!(record.name, "APICodeGenerator");
    }

    #[test]
    fn resolve_wrong_agent_blocked() {
        let catalog = SkillCatalog::from_agents(&two_agent_fixture());
        let err = catalog.resolve("api-gen", "fetch_data").unwrap_err();
        assert!(matches!(err, SkillResolveError::SkillNotInCatalog { .. }));
    }

    #[test]
    fn resolve_tool_id_round_trip() {
        let catalog = SkillCatalog::from_agents(&two_agent_fixture());
        let record = catalog
            .resolve_tool_id("api-gen__APICodeGenerator")
            .unwrap();
        assert_eq!(record.mongo_skill_id, "mongo-api-1");
    }

    #[test]
    fn resolve_tool_id_malformed() {
        let catalog = SkillCatalog::from_agents(&two_agent_fixture());
        let err = catalog.resolve_tool_id("no-delimiter").unwrap_err();
        assert!(matches!(err, SkillResolveError::InvalidToolId { .. }));
    }

    #[test]
    fn resolve_assigned_by_name() {
        let catalog = SkillCatalog::from_agents(&two_agent_fixture());
        let record = catalog
            .resolve_assigned("Data Analyzer", "fetch_data")
            .unwrap();
        assert_eq!(record.skill_ref.agent_slug, "data-analyzer");
    }

    #[test]
    fn validate_tasks_first_error() {
        let catalog = SkillCatalog::from_agents(&two_agent_fixture());
        let tasks = vec![
            PlannedTask {
                step: "1".into(),
                description: "d".into(),
                assigned_agent: "Data Analyzer".into(),
                reason: "r".into(),
                tool_id: None,
                skill_id: Some("fetch_data".into()),
                input_arguments: None,
            },
            PlannedTask {
                step: "2".into(),
                description: "d".into(),
                assigned_agent: "Data Analyzer".into(),
                reason: "r".into(),
                tool_id: None,
                skill_id: Some("nonexistent".into()),
                input_arguments: None,
            },
        ];
        assert!(catalog.validate_tasks(&tasks).is_err());
    }

    #[test]
    fn tools_appendix_non_empty() {
        let catalog = SkillCatalog::from_agents(&two_agent_fixture());
        let appendix = catalog.tools_appendix();
        assert!(appendix.contains("api-gen"));
        assert!(appendix.contains("api-gen__APICodeGenerator"));
    }

    #[test]
    fn agent_capabilities_appendix_lists_skills_without_tool_id() {
        let catalog = SkillCatalog::from_agents(&two_agent_fixture());
        let appendix = catalog.agent_capabilities_appendix_truncated(usize::MAX);
        assert!(appendix.contains("fetch_data"));
        assert!(appendix.contains("Capabilities:"));
        assert!(!appendix.contains("data-analyzer__"));
    }

    #[test]
    fn for_agent_slug_excludes_other_agents() {
        let catalog = SkillCatalog::from_agents(&two_agent_fixture());
        let scoped = catalog.for_agent_slug("data-analyzer");
        let ids = scoped.tool_ids();
        assert!(ids.iter().all(|id| id.starts_with("data-analyzer__")));
        assert!(!ids.iter().any(|id| id.starts_with("api-gen__")));
    }

    #[test]
    fn workflow_state_deser_without_catalog() {
        let v1 = json!({
            "version": WORKFLOW_STATE_VERSION,
            "sessionId": "s1",
            "correlationId": "c1",
            "phase": "executing",
            "userMessage": "hi",
            "agents": [],
            "history": [],
            "results": [],
            "usage": {}
        });
        let state: WorkflowState = serde_json::from_value(v1).unwrap();
        assert!(state.skill_catalog.is_empty());
    }

    #[test]
    fn workflow_state_with_catalog_round_trip() {
        let mut state = WorkflowState::new("s".into(), "c".into(), "q".into());
        state.skill_catalog = SkillCatalog::from_agents(&two_agent_fixture());
        let serialized = serde_json::to_string(&state).unwrap();
        let back: WorkflowState = serde_json::from_str(&serialized).unwrap();
        assert_eq!(back.skill_catalog.agents.len(), 2);
    }

    #[test]
    fn validate_task_mongo_id_in_plan() {
        let catalog = SkillCatalog::from_agents(&two_agent_fixture());
        let task = PlannedTask {
            step: "1".into(),
            description: "d".into(),
            assigned_agent: "API Code Generator".into(),
            reason: "r".into(),
            tool_id: None,
            skill_id: Some("mongo-api-1".into()),
            input_arguments: None,
        };
        assert!(catalog.validate_task(&task).is_ok());
    }

    #[test]
    fn is_empty_default() {
        assert!(SkillCatalog::default().is_empty());
    }

    #[test]
    fn normalize_task_from_tool_id_sets_fields() {
        let catalog = SkillCatalog::from_agents(&two_agent_fixture());
        let mut task = PlannedTask {
            step: "1".into(),
            description: "d".into(),
            assigned_agent: String::new(),
            reason: "r".into(),
            tool_id: Some("api-gen__APICodeGenerator".into()),
            skill_id: None,
            input_arguments: None,
        };
        let record = catalog.normalize_task_from_tool_id(&mut task).unwrap();
        assert_eq!(record.name, "APICodeGenerator");
        assert_eq!(task.assigned_agent, "API Code Generator");
        assert_eq!(task.skill_id.as_deref(), Some("APICodeGenerator"));
    }

    #[test]
    fn normalize_legacy_task_unchanged() {
        let catalog = SkillCatalog::from_agents(&two_agent_fixture());
        let mut task = PlannedTask {
            step: "1".into(),
            description: "d".into(),
            assigned_agent: "Data Analyzer".into(),
            reason: "r".into(),
            tool_id: None,
            skill_id: Some("fetch_data".into()),
            input_arguments: None,
        };
        catalog
            .normalize_tasks_from_tool_ids(std::slice::from_mut(&mut task))
            .unwrap();
        assert_eq!(task.assigned_agent, "Data Analyzer");
        assert_eq!(task.skill_id.as_deref(), Some("fetch_data"));
    }

    #[test]
    fn validate_task_with_tool_id() {
        let catalog = SkillCatalog::from_agents(&two_agent_fixture());
        let task = PlannedTask {
            step: "1".into(),
            description: "d".into(),
            assigned_agent: String::new(),
            reason: "r".into(),
            tool_id: Some("data-analyzer__fetch_data".into()),
            skill_id: None,
            input_arguments: None,
        };
        assert!(catalog.validate_task(&task).is_ok());
    }

    #[test]
    fn validate_task_rejects_conflicting_tool_id_and_skill_id() {
        let catalog = SkillCatalog::from_agents(&two_agent_fixture());
        let task = PlannedTask {
            step: "1".into(),
            description: "d".into(),
            assigned_agent: String::new(),
            reason: "r".into(),
            tool_id: Some("api-gen__APICodeGenerator".into()),
            skill_id: Some("fetch_data".into()),
            input_arguments: None,
        };
        assert!(catalog.validate_task(&task).is_err());
    }

    #[test]
    fn input_arguments_schema_openai_strict_compliant() {
        let catalog = SkillCatalog::from_agents(&two_agent_fixture());
        let schema = catalog.input_arguments_schema();
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], false);
    }

    #[test]
    fn input_arguments_schema_merges_tool_properties() {
        let mut agents = two_agent_fixture();
        agents[0]["skills"][0]["arguments"] = json!({
            "type": "object",
            "properties": {
                "endpoint": { "type": "string" },
                "method": { "type": "string" }
            }
        });
        let catalog = SkillCatalog::from_agents(&agents);
        let schema = catalog.input_arguments_schema();
        assert_eq!(schema["additionalProperties"], false);
        assert!(schema["properties"].get("endpoint").is_some());
        assert!(schema["properties"].get("method").is_some());
    }

    #[test]
    fn from_agents_filtered_excludes_other_assistant_tools() {
        let pool = two_agent_fixture();
        let mut allowed = HashSet::new();
        allowed.insert("api-gen".to_string());
        let catalog = SkillCatalog::from_agents_filtered(&pool, &allowed);
        assert_eq!(catalog.agents.len(), 1);
        assert_eq!(
            catalog.tool_ids(),
            vec!["api-gen__APICodeGenerator".to_string()]
        );
        assert!(catalog.resolve("data-analyzer", "fetch_data").is_err());
    }

    #[test]
    fn from_workflow_state_uses_persisted_catalog() {
        let mut state = WorkflowState::new("s".into(), "c".into(), "q".into());
        state.active_agent_slugs = vec!["api-gen".into()];
        state.kriya_agents = Some(two_agent_fixture());
        state.skill_catalog =
            SkillCatalog::from_agents_filtered(state.kriya_agents.as_ref().unwrap(), &{
                let mut s = HashSet::new();
                s.insert("api-gen".into());
                s
            });
        let catalog = SkillCatalog::from_workflow_state(&state);
        assert_eq!(catalog.tool_ids().len(), 1);
    }

    #[test]
    fn tools_appendix_truncated_under_cap() {
        let mut agents = two_agent_fixture();
        for agent in agents.iter_mut() {
            if let Some(skills) = agent.get_mut("skills").and_then(|s| s.as_array_mut()) {
                for skill in skills.iter_mut() {
                    skill
                        .as_object_mut()
                        .unwrap()
                        .insert("description".into(), json!("x".repeat(5000)));
                }
            }
        }
        let catalog = SkillCatalog::from_agents(&agents);
        let appendix = catalog.tools_appendix_truncated(500);
        assert!(appendix.len() <= 600);
        assert!(appendix.contains("truncated"));
    }

    #[test]
    fn validate_arguments_rejects_missing_required() {
        let catalog = SkillCatalog::from_agents(&two_agent_fixture());
        let record = catalog.resolve("api-gen", "APICodeGenerator").unwrap();
        let mut record_with_params = record.clone();
        record_with_params.parameters = Some(json!({
            "type": "object",
            "required": ["endpoint"],
            "properties": { "endpoint": { "type": "string" } }
        }));
        let err = SkillCatalog::validate_arguments(&record_with_params, &json!({})).unwrap_err();
        assert!(matches!(err, SkillResolveError::ArgumentsInvalid { .. }));
    }

    #[test]
    fn validate_arguments_accepts_required_present() {
        let catalog = SkillCatalog::from_agents(&two_agent_fixture());
        let record = catalog.resolve("api-gen", "APICodeGenerator").unwrap();
        let mut record_with_params = record.clone();
        record_with_params.parameters = Some(json!({
            "type": "object",
            "required": ["endpoint"],
            "properties": { "endpoint": { "type": "string" } }
        }));
        assert!(
            SkillCatalog::validate_arguments(&record_with_params, &json!({ "endpoint": "/api" }))
                .is_ok()
        );
    }

    fn record_with_endpoint_param() -> SkillRecord {
        SkillRecord {
            skill_ref: SkillRef {
                agent_slug: "api-gen".into(),
                skill_key: "APICodeGenerator".into(),
            },
            tool_id: "api-gen__APICodeGenerator".into(),
            mongo_skill_id: "mongo-1".into(),
            skill_version: None,
            name: "APICodeGenerator".into(),
            skill_type: Some("JS".into()),
            description: Some("Generates API code".into()),
            parameters: Some(json!({
                "type": "object",
                "properties": { "endpoint": { "type": "string" } },
                "required": ["endpoint"]
            })),
        }
    }

    #[test]
    fn input_arguments_schema_for_tool_strict_compliance() {
        let record = record_with_endpoint_param();
        let schema = SkillCatalog::input_arguments_schema_for_tool(&record);
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], false);
        assert!(schema["properties"].get("endpoint").is_some());
        let required = schema["required"].as_array().expect("required");
        assert!(required.iter().any(|k| k == "endpoint"));
    }

    #[test]
    fn input_arguments_schema_for_tool_no_parameters() {
        let mut record = record_with_endpoint_param();
        record.parameters = None;
        let schema = SkillCatalog::input_arguments_schema_for_tool(&record);
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], false);
    }

    #[test]
    fn required_argument_keys_reads_schema() {
        let record = record_with_endpoint_param();
        assert_eq!(
            SkillCatalog::required_argument_keys(&record),
            vec!["endpoint".to_string()]
        );
    }

    #[test]
    fn arguments_need_resolution_when_empty_and_required() {
        let record = record_with_endpoint_param();
        assert!(SkillCatalog::arguments_need_resolution(&record, &json!({})));
        assert!(!SkillCatalog::arguments_need_resolution(
            &record,
            &json!({ "endpoint": "https://x.com" })
        ));
    }

    fn flat_faas_arguments() -> Value {
        json!({
            "inputFormat": {
                "type": "string",
                "required": true,
                "description": "Input data format"
            },
            "outputFormat": {
                "type": "string",
                "required": true,
                "description": "Output data format"
            }
        })
    }

    fn record_with_flat_faas_params(raw: bool) -> SkillRecord {
        let params = flat_faas_arguments();
        SkillRecord {
            skill_ref: SkillRef {
                agent_slug: "transformer".into(),
                skill_key: "data_transform".into(),
            },
            tool_id: "transformer__data_transform".into(),
            mongo_skill_id: "mongo-transform-1".into(),
            skill_version: None,
            name: "data_transform".into(),
            skill_type: Some("FaaS".into()),
            description: Some("Transforms data".into()),
            parameters: if raw {
                Some(params)
            } else {
                normalize_skill_parameters(Some(&params))
            },
        }
    }

    #[test]
    fn flat_faas_format_extracts_required_keys() {
        let record = record_with_flat_faas_params(true);
        assert_eq!(
            SkillCatalog::required_argument_keys(&record),
            vec!["inputFormat".to_string(), "outputFormat".to_string()]
        );
        assert!(SkillCatalog::arguments_need_resolution(&record, &json!({})));
    }

    #[test]
    fn from_agents_with_flat_faas_arguments() {
        let agents = vec![json!({
            "name": "Data Transformer",
            "slug": "transformer",
            "skills": [{
                "name": "data_transform",
                "_id": { "$oid": "mongo-transform-1" },
                "type": "FaaS",
                "description": "Transforms data",
                "arguments": flat_faas_arguments()
            }]
        })];
        let catalog = SkillCatalog::from_agents(&agents);
        let record = catalog.resolve("transformer", "data_transform").unwrap();
        assert_eq!(
            SkillCatalog::required_argument_keys(&record),
            vec!["inputFormat".to_string(), "outputFormat".to_string()]
        );
        let schema = SkillCatalog::input_arguments_schema_for_tool(&record);
        assert!(schema["properties"].get("inputFormat").is_some());
        assert!(schema["properties"].get("outputFormat").is_some());
    }

    #[test]
    fn validate_arguments_rejects_missing_required_flat_faas() {
        let record = record_with_flat_faas_params(true);
        let err = SkillCatalog::validate_arguments(&record, &json!({ "inputFormat": "json" }))
            .unwrap_err();
        assert!(matches!(err, SkillResolveError::ArgumentsInvalid { .. }));
        assert!(err.to_string().contains("outputFormat"));
    }

    #[test]
    fn normalize_skill_parameters_array_format() {
        let raw = json!([
            { "name": "endpoint", "type": "string", "required": true },
            { "name": "method", "type": "string", "required": false }
        ]);
        let normalized = normalize_skill_parameters(Some(&raw)).unwrap();
        assert_eq!(normalized["type"], "object");
        assert!(normalized["properties"].get("endpoint").is_some());
        assert_eq!(
            SkillCatalog::required_argument_keys(&SkillRecord {
                skill_ref: SkillRef {
                    agent_slug: "a".into(),
                    skill_key: "b".into(),
                },
                tool_id: "a__b".into(),
                mongo_skill_id: "m".into(),
                skill_version: None,
                name: "b".into(),
                skill_type: None,
                description: None,
                parameters: Some(raw),
            }),
            vec!["endpoint".to_string()]
        );
    }

    #[test]
    fn input_arguments_schema_for_tool_openai_strict_lists_all_properties() {
        let record = SkillRecord {
            skill_ref: SkillRef {
                agent_slug: "web-search-agent".into(),
                skill_key: "Web Search".into(),
            },
            tool_id: "web-search-agent__Web Search".into(),
            mongo_skill_id: "m".into(),
            skill_version: None,
            name: "Web Search".into(),
            skill_type: Some("FaaS".into()),
            description: Some("Search the web".into()),
            parameters: Some(json!({
                "query": {
                    "type": "string",
                    "required": true,
                    "description": "Search query"
                }
            })),
        };
        let schema = SkillCatalog::input_arguments_schema_for_tool(&record);
        assert!(schema["properties"].get("query").is_some());
        let required = schema["required"].as_array().expect("required");
        assert_eq!(required.len(), 1);
        assert_eq!(required[0], "query");
    }

    #[test]
    fn flat_faas_normalizes_array_field_with_items() {
        let raw = json!({
            "file_ids": {
                "description": "Array of image file IDs used for analysis.",
                "items": { "type": "string" },
                "required": true,
                "type": "array"
            },
            "question": {
                "description": "Natural language question about the image(s).",
                "required": true,
                "type": "string"
            }
        });
        let normalized = normalize_skill_parameters(Some(&raw)).unwrap();
        let file_ids = &normalized["properties"]["file_ids"];
        assert_eq!(file_ids["type"], "array");
        assert_eq!(file_ids["items"]["type"], "string");
        let record = SkillRecord {
            skill_ref: SkillRef {
                agent_slug: "vision".into(),
                skill_key: "analyze".into(),
            },
            tool_id: "vision__analyze".into(),
            mongo_skill_id: "m".into(),
            skill_version: None,
            name: "analyze".into(),
            skill_type: Some("FaaS".into()),
            description: None,
            parameters: Some(raw.clone()),
        };
        assert_eq!(
            SkillCatalog::required_argument_keys(&record),
            vec!["file_ids".to_string(), "question".to_string()]
        );
        let schema = SkillCatalog::input_arguments_schema_for_tool(&record);
        assert_eq!(schema["properties"]["file_ids"]["type"], "array");
        let required = schema["required"].as_array().expect("required");
        assert!(required.iter().any(|k| k == "file_ids"));
        assert!(required.iter().any(|k| k == "question"));
    }
}
