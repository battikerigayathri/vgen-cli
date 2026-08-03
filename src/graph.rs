use crate::specs::{
    bundle_gates, gate_composition_edges, gate_refs_in_condition, list_agent_files,
    list_assistant_files, list_hitl_dirs, list_tool_dirs, list_workflow_dirs, load_tool_yaml,
    load_workflow_from_dir, parse_assistant_workflow_slug, stage_declaration_order,
    stage_transitions, transition_is_back_edge, transition_ref_value, ResourceIndex,
};
use crate::workspace::Workspace;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum GraphError {
    Io(std::io::Error),
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphError::Io(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for GraphError {}

impl From<std::io::Error> for GraphError {
    fn from(value: std::io::Error) -> Self {
        GraphError::Io(value)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Assistant,
    Agent,
    Tool,
    Hitl,
    Workflow,
    WorkflowStage,
    WorkflowGate,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    AssistantAgent,
    AgentTool,
    AssistantWorkflow,
    WorkflowHitl,
    WorkflowAgent,
    WorkflowStageNext,
    WorkflowStageBackTo,
    WorkflowStageTransition,
    WorkflowGateRef,
    BrokenRef,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GraphNode {
    pub id: String,
    pub slug: String,
    pub kind: NodeKind,
    pub path: String,
    pub missing_id: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Graph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub orphans: Vec<GraphNode>,
}

pub fn build_graph(ws: &Workspace) -> Result<Graph, GraphError> {
    build_graph_filtered(ws, None)
}

pub fn build_graph_filtered(
    ws: &Workspace,
    assistant_filter: Option<&str>,
) -> Result<Graph, GraphError> {
    let index = ResourceIndex::build(ws)?;
    let mut nodes_by_key: HashMap<String, GraphNode> = HashMap::new();
    let mut edges: Vec<GraphEdge> = Vec::new();

    load_assistant_nodes(ws, &mut nodes_by_key)?;
    load_agent_nodes(ws, &mut nodes_by_key)?;
    load_tool_nodes(ws, &mut nodes_by_key)?;
    load_hitl_nodes(ws, &mut nodes_by_key)?;
    load_workflow_nodes(ws, &mut nodes_by_key, &mut edges)?;

    let assistant_entries: Vec<(String, PathBuf)> = if let Some(name) = assistant_filter {
        list_assistant_files(&ws.assistants_dir)
            .into_iter()
            .filter(|(stem, path)| assistant_matches_filter(stem, path, name))
            .collect()
    } else {
        list_assistant_files(&ws.assistants_dir)
    };

    if assistant_filter.is_some() && assistant_entries.is_empty() {
        return Err(GraphError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "Assistant not found: {}",
                assistant_filter.unwrap_or_default()
            ),
        )));
    }

    for (_stem, path) in &assistant_entries {
        let value = read_yaml_file(path)?;
        let from_key = node_key_from_value(&value, path);

        if let Some(agent_ids) = value.get("agents").and_then(|v| v.as_array()) {
            for agent_ref in agent_ids {
                if let Some(agent_id) = agent_ref.as_str().map(str::trim).filter(|s| !s.is_empty())
                {
                    match index.agents_by_id.get(agent_id) {
                        Some(agent_path) => {
                            let agent_value = read_yaml_file(agent_path)?;
                            let to_key = node_key_from_value(&agent_value, agent_path);
                            edges.push(GraphEdge {
                                from: from_key.clone(),
                                to: to_key.clone(),
                                kind: EdgeKind::AssistantAgent,
                                ref_value: Some(agent_id.to_string()),
                            });
                            add_agent_edges(&index, &agent_value, &to_key, &mut edges)?;
                        }
                        None => {
                            edges.push(GraphEdge {
                                from: from_key.clone(),
                                to: agent_id.to_string(),
                                kind: EdgeKind::BrokenRef,
                                ref_value: Some(agent_id.to_string()),
                            });
                        }
                    }
                }
            }
        }

        if let Some(sys_context) = value.get("systemContext").and_then(|v| v.as_str()) {
            if let Some(workflow_slug) = parse_assistant_workflow_slug(sys_context) {
                match index.workflows_by_slug.get(&workflow_slug) {
                    Some(workflow_dir) => {
                        let (bundle, _, _) = load_workflow_from_dir(workflow_dir)
                            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                        let workflow_node = workflow_node_from_bundle(&bundle, workflow_dir, ws)?;
                        let to_key = workflow_node.id.clone();
                        nodes_by_key.entry(to_key.clone()).or_insert(workflow_node);
                        edges.push(GraphEdge {
                            from: from_key.clone(),
                            to: to_key.clone(),
                            kind: EdgeKind::AssistantWorkflow,
                            ref_value: Some(workflow_slug.clone()),
                        });
                        add_workflow_edges(&index, &bundle, &to_key, &mut edges)?;
                    }
                    None => {
                        edges.push(GraphEdge {
                            from: from_key.clone(),
                            to: workflow_slug.clone(),
                            kind: EdgeKind::BrokenRef,
                            ref_value: Some(workflow_slug),
                        });
                    }
                }
            }
        }
    }

    let reachable = compute_reachable(&nodes_by_key, &edges, &assistant_entries)?;

    let mut nodes: Vec<GraphNode> = nodes_by_key.into_values().collect();
    nodes.sort_by(|a, b| {
        (kind_sort_key(&a.kind), a.slug.as_str(), a.path.as_str()).cmp(&(
            kind_sort_key(&b.kind),
            b.slug.as_str(),
            b.path.as_str(),
        ))
    });

    let orphans: Vec<GraphNode> = nodes
        .iter()
        .filter(|n| !reachable.contains(&n.id))
        .cloned()
        .collect();

    let (nodes, edges) = if assistant_filter.is_some() {
        let node_ids: HashSet<String> = reachable.clone();
        let nodes: Vec<GraphNode> = nodes
            .into_iter()
            .filter(|n| node_ids.contains(&n.id))
            .collect();
        let edges: Vec<GraphEdge> = edges
            .into_iter()
            .filter(|e| {
                node_ids.contains(&e.from)
                    && (node_ids.contains(&e.to) || e.kind == EdgeKind::BrokenRef)
            })
            .collect();
        (nodes, edges)
    } else {
        (nodes, edges)
    };

    Ok(Graph {
        nodes,
        edges,
        orphans,
    })
}

fn kind_sort_key(kind: &NodeKind) -> u8 {
    match kind {
        NodeKind::Assistant => 0,
        NodeKind::Agent => 1,
        NodeKind::Tool => 2,
        NodeKind::Hitl => 3,
        NodeKind::Workflow => 4,
        NodeKind::WorkflowStage => 5,
        NodeKind::WorkflowGate => 6,
    }
}

fn assistant_matches_filter(stem: &str, path: &Path, filter: &str) -> bool {
    if stem == filter {
        return true;
    }
    if let Ok(value) = read_yaml_file(path) {
        if value
            .get("slug")
            .and_then(|v| v.as_str())
            .is_some_and(|s| s == filter)
        {
            return true;
        }
        if value
            .get("id")
            .and_then(|v| v.as_str())
            .is_some_and(|s| s == filter)
        {
            return true;
        }
    }
    false
}

fn load_assistant_nodes(
    ws: &Workspace,
    nodes: &mut HashMap<String, GraphNode>,
) -> Result<(), GraphError> {
    for (_stem, path) in list_assistant_files(&ws.assistants_dir) {
        let value = read_yaml_file(&path)?;
        let node = graph_node_from_value(&value, &path, ws, NodeKind::Assistant);
        nodes.insert(node.id.clone(), node);
    }
    Ok(())
}

fn load_agent_nodes(
    ws: &Workspace,
    nodes: &mut HashMap<String, GraphNode>,
) -> Result<(), GraphError> {
    for (_stem, path) in list_agent_files(&ws.agents_dir) {
        let value = read_yaml_file(&path)?;
        let node = graph_node_from_value(&value, &path, ws, NodeKind::Agent);
        nodes.insert(node.id.clone(), node);
    }
    Ok(())
}

fn load_tool_nodes(
    ws: &Workspace,
    nodes: &mut HashMap<String, GraphNode>,
) -> Result<(), GraphError> {
    for tool_dir in list_tool_dirs(&ws.tools_dir) {
        let (value, yaml_path) = load_tool_yaml(&tool_dir)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let node = graph_node_from_value(&value, &yaml_path, ws, NodeKind::Tool);
        nodes.insert(node.id.clone(), node);
    }
    Ok(())
}

fn load_hitl_nodes(
    ws: &Workspace,
    nodes: &mut HashMap<String, GraphNode>,
) -> Result<(), GraphError> {
    for hitl_dir in list_hitl_dirs(&ws.hitl_dir) {
        let meta_path = if hitl_dir.join("meta.yaml").is_file() {
            hitl_dir.join("meta.yaml")
        } else {
            hitl_dir.join("meta.yml")
        };
        let value = read_yaml_file(&meta_path)?;
        let node = graph_node_from_value(&value, &meta_path, ws, NodeKind::Hitl);
        nodes.insert(node.id.clone(), node);
    }
    Ok(())
}

fn load_workflow_nodes(
    ws: &Workspace,
    nodes: &mut HashMap<String, GraphNode>,
    edges: &mut Vec<GraphEdge>,
) -> Result<(), GraphError> {
    for workflow_dir in list_workflow_dirs(&ws.workflows_dir) {
        let (bundle, _, _) = load_workflow_from_dir(&workflow_dir)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let node = workflow_node_from_bundle(&bundle, &workflow_dir, ws)?;
        let workflow_key = node.id.clone();
        let workflow_slug = node.slug.clone();
        let workflow_path = node.path.clone();
        nodes.insert(workflow_key.clone(), node);
        add_workflow_stage_graph(
            &bundle,
            &workflow_slug,
            &workflow_key,
            &workflow_path,
            nodes,
            edges,
        );
        add_workflow_gate_graph(&bundle, &workflow_slug, &workflow_path, nodes, edges);
    }
    Ok(())
}

fn workflow_node_from_bundle(
    bundle: &Value,
    workflow_dir: &Path,
    ws: &Workspace,
) -> Result<GraphNode, GraphError> {
    let meta = bundle.get("meta").unwrap_or(bundle);
    let slug = string_field(meta, "slug")
        .or_else(|| {
            workflow_dir
                .file_name()
                .and_then(|s| s.to_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "unknown".to_string());
    let missing_id = meta
        .get("id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .map(|s| s.is_empty())
        .unwrap_or(true);
    let id = string_field(meta, "id").unwrap_or_else(|| slug.clone());
    Ok(GraphNode {
        id,
        slug,
        kind: NodeKind::Workflow,
        path: relative_path(workflow_dir, &ws.root),
        missing_id,
    })
}

fn graph_node_from_value(value: &Value, path: &Path, ws: &Workspace, kind: NodeKind) -> GraphNode {
    let slug = string_field(value, "slug")
        .or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "unknown".to_string());
    let missing_id = value
        .get("id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .map(|s| s.is_empty())
        .unwrap_or(true);
    let id = string_field(value, "id").unwrap_or_else(|| slug.clone());
    GraphNode {
        id,
        slug,
        kind,
        path: relative_path(path, &ws.root),
        missing_id,
    }
}

fn node_key_from_value(value: &Value, path: &Path) -> String {
    let slug = string_field(value, "slug")
        .or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "unknown".to_string());
    string_field(value, "id").unwrap_or(slug)
}

fn add_agent_edges(
    index: &ResourceIndex,
    agent_value: &Value,
    from_key: &str,
    edges: &mut Vec<GraphEdge>,
) -> Result<(), GraphError> {
    if let Some(skills) = agent_value.get("skills").and_then(|v| v.as_array()) {
        for skill_ref in skills {
            if let Some(tool_id) = skill_ref.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                match index.tools_by_id.get(tool_id) {
                    Some(tool_dir) => {
                        let (tool_value, yaml_path) = load_tool_yaml(tool_dir)
                            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                        let to_key = node_key_from_value(&tool_value, &yaml_path);
                        edges.push(GraphEdge {
                            from: from_key.to_string(),
                            to: to_key,
                            kind: EdgeKind::AgentTool,
                            ref_value: Some(tool_id.to_string()),
                        });
                    }
                    None => {
                        edges.push(GraphEdge {
                            from: from_key.to_string(),
                            to: tool_id.to_string(),
                            kind: EdgeKind::BrokenRef,
                            ref_value: Some(tool_id.to_string()),
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

fn required_from_stage_fields_by_stage(bundle: &Value) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    let Some(fields) = bundle
        .get("schema")
        .and_then(|schema| schema.get("fields"))
        .and_then(|fields| fields.as_array())
    else {
        return map;
    };
    for field in fields {
        let Some(key) = field.get("key").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(stage) = field
            .get("requiredFromStage")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        map.entry(stage.to_string())
            .or_default()
            .push(key.to_string());
    }
    for keys in map.values_mut() {
        keys.sort();
    }
    map
}

fn format_stage_slug(stage_id: &str, req_from_fields: &HashMap<String, Vec<String>>) -> String {
    let Some(keys) = req_from_fields.get(stage_id) else {
        return stage_id.to_string();
    };
    if keys.is_empty() {
        return stage_id.to_string();
    }
    format!("{stage_id} [reqFrom: {}]", keys.join(", "))
}

fn add_workflow_stage_graph(
    bundle: &Value,
    workflow_slug: &str,
    workflow_node_id: &str,
    workflow_path: &str,
    nodes: &mut HashMap<String, GraphNode>,
    edges: &mut Vec<GraphEdge>,
) {
    let flow = bundle.get("flow").unwrap_or(&Value::Null);
    let Some(stages) = flow.get("stages").and_then(|v| v.as_array()) else {
        return;
    };
    let req_from_fields = required_from_stage_fields_by_stage(bundle);
    let stage_order = stage_declaration_order(stages);

    for stage in stages {
        let Some(stage_id) = stage.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let stage_node_id = stage_node_key(workflow_slug, stage_id);
        nodes.entry(stage_node_id.clone()).or_insert(GraphNode {
            id: stage_node_id,
            slug: format_stage_slug(stage_id, &req_from_fields),
            kind: NodeKind::WorkflowStage,
            path: format!("{workflow_path}#stage:{stage_id}"),
            missing_id: false,
        });
    }

    if let Some(initial) = flow.get("initialStage").and_then(|v| v.as_str()) {
        edges.push(GraphEdge {
            from: workflow_node_id.to_string(),
            to: stage_node_key(workflow_slug, initial),
            kind: EdgeKind::WorkflowStageNext,
            ref_value: Some("initial".to_string()),
        });
    }

    for stage in stages {
        let Some(stage_id) = stage.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let from = stage_node_key(workflow_slug, stage_id);
        let transitions = stage_transitions(stage);

        for rule in &transitions {
            let kind = if transition_is_back_edge(stage_id, &rule.target, &stage_order) {
                EdgeKind::WorkflowStageBackTo
            } else {
                EdgeKind::WorkflowStageTransition
            };
            edges.push(GraphEdge {
                from: from.clone(),
                to: stage_node_key(workflow_slug, &rule.target),
                kind,
                ref_value: Some(transition_ref_value(rule)),
            });
        }

        if let Some(next) = stage.get("next").and_then(|v| v.as_str()) {
            edges.push(GraphEdge {
                from: from.clone(),
                to: stage_node_key(workflow_slug, next),
                kind: EdgeKind::WorkflowStageNext,
                ref_value: None,
            });
        }

        if let Some(back_to) = stage.get("backTo").and_then(|v| v.as_str()) {
            edges.push(GraphEdge {
                from,
                to: stage_node_key(workflow_slug, back_to),
                kind: EdgeKind::WorkflowStageBackTo,
                ref_value: None,
            });
        }
    }
}

fn stage_node_key(workflow_slug: &str, stage_id: &str) -> String {
    format!("{workflow_slug}::{stage_id}")
}

fn gate_node_key(workflow_slug: &str, gate_id: &str) -> String {
    format!("{workflow_slug}::gate::{gate_id}")
}

fn add_workflow_gate_graph(
    bundle: &Value,
    workflow_slug: &str,
    workflow_path: &str,
    nodes: &mut HashMap<String, GraphNode>,
    edges: &mut Vec<GraphEdge>,
) {
    let gates = bundle_gates(bundle);
    if gates.is_empty() {
        return;
    }

    for gate in &gates {
        let gate_node_id = gate_node_key(workflow_slug, &gate.id);
        nodes.entry(gate_node_id.clone()).or_insert(GraphNode {
            id: gate_node_id,
            slug: gate.name.clone(),
            kind: NodeKind::WorkflowGate,
            path: format!("{workflow_path}#gate:{}", gate.id),
            missing_id: false,
        });
    }

    for (from_gate, to_gate) in gate_composition_edges(&gates) {
        edges.push(GraphEdge {
            from: gate_node_key(workflow_slug, &from_gate),
            to: gate_node_key(workflow_slug, &to_gate),
            kind: EdgeKind::WorkflowGateRef,
            ref_value: Some(format!("gate({to_gate})")),
        });
    }

    let flow = bundle.get("flow").unwrap_or(&Value::Null);
    let Some(stages) = flow.get("stages").and_then(|v| v.as_array()) else {
        return;
    };

    for stage in stages {
        let Some(stage_id) = stage.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let from = stage_node_key(workflow_slug, stage_id);
        for rule in stage_transitions(stage) {
            for gate_ref in gate_refs_in_condition(&rule.when) {
                edges.push(GraphEdge {
                    from: from.clone(),
                    to: gate_node_key(workflow_slug, gate_ref),
                    kind: EdgeKind::WorkflowGateRef,
                    ref_value: Some(format!("gate({gate_ref})")),
                });
            }
        }
    }
}

fn add_workflow_edges(
    index: &ResourceIndex,
    bundle: &Value,
    from_key: &str,
    edges: &mut Vec<GraphEdge>,
) -> Result<(), GraphError> {
    let flow = bundle.get("flow").unwrap_or(&Value::Null);
    if let Some(stages) = flow.get("stages").and_then(|v| v.as_array()) {
        for stage in stages {
            if let Some(hitl_slug) = stage
                .get("hitlSlug")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                match index.hitl_by_slug.get(hitl_slug) {
                    Some(hitl_dir) => {
                        let meta_path = if hitl_dir.join("meta.yaml").is_file() {
                            hitl_dir.join("meta.yaml")
                        } else {
                            hitl_dir.join("meta.yml")
                        };
                        let hitl_value = read_yaml_file(&meta_path)?;
                        let to_key = node_key_from_value(&hitl_value, &meta_path);
                        edges.push(GraphEdge {
                            from: from_key.to_string(),
                            to: to_key,
                            kind: EdgeKind::WorkflowHitl,
                            ref_value: Some(hitl_slug.to_string()),
                        });
                    }
                    None => {
                        edges.push(GraphEdge {
                            from: from_key.to_string(),
                            to: hitl_slug.to_string(),
                            kind: EdgeKind::BrokenRef,
                            ref_value: Some(hitl_slug.to_string()),
                        });
                    }
                }
            }

            if let Some(agent_slug) = stage
                .get("agentSlug")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                match index.agents_by_slug.get(agent_slug) {
                    Some(agent_path) => {
                        let agent_value = read_yaml_file(agent_path)?;
                        let to_key = node_key_from_value(&agent_value, agent_path);
                        edges.push(GraphEdge {
                            from: from_key.to_string(),
                            to: to_key,
                            kind: EdgeKind::WorkflowAgent,
                            ref_value: Some(agent_slug.to_string()),
                        });
                    }
                    None => {
                        edges.push(GraphEdge {
                            from: from_key.to_string(),
                            to: agent_slug.to_string(),
                            kind: EdgeKind::BrokenRef,
                            ref_value: Some(agent_slug.to_string()),
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

fn compute_reachable(
    nodes_by_key: &HashMap<String, GraphNode>,
    edges: &[GraphEdge],
    assistant_entries: &[(String, PathBuf)],
) -> Result<HashSet<String>, GraphError> {
    let mut reachable: HashSet<String> = HashSet::new();
    let mut queue: Vec<String> = Vec::new();

    for (_stem, path) in assistant_entries {
        let value = read_yaml_file(path)?;
        let key = node_key_from_value(&value, path);
        if nodes_by_key.contains_key(&key) && reachable.insert(key.clone()) {
            queue.push(key);
        }
    }

    while let Some(from) = queue.pop() {
        for edge in edges {
            if edge.from != from || edge.kind == EdgeKind::BrokenRef {
                continue;
            }
            if nodes_by_key.contains_key(&edge.to) && reachable.insert(edge.to.clone()) {
                queue.push(edge.to.clone());
            }
        }
    }

    Ok(reachable)
}

fn relative_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn read_yaml_file(path: &Path) -> Result<Value, GraphError> {
    let content = std::fs::read_to_string(path)?;
    serde_yaml::from_str(&content)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_minimal_graph_workspace(root: &Path) {
        fs::create_dir_all(root.join("assistants")).unwrap();
        fs::create_dir_all(root.join("agents")).unwrap();
        fs::create_dir_all(root.join("tools/orphan-tool")).unwrap();
        fs::create_dir_all(root.join("tools/linked-tool")).unwrap();
        fs::create_dir_all(root.join("hitl/test-form")).unwrap();
        fs::create_dir_all(root.join("workflows/test-flow")).unwrap();

        fs::write(
            root.join("assistants/test-assistant.yaml"),
            r#"id: asst-1
slug: test-assistant
agents:
  - agent-1
systemContext: |
  workflow_definition_slug: test-flow-v1
"#,
        )
        .unwrap();
        fs::write(
            root.join("agents/test-agent.yaml"),
            r#"id: agent-1
slug: test-agent
skills:
  - tool-1
"#,
        )
        .unwrap();
        fs::write(
            root.join("tools/linked-tool/tool.yaml"),
            "id: tool-1\nname: linked\n",
        )
        .unwrap();
        fs::write(
            root.join("tools/orphan-tool/tool.yaml"),
            "id: tool-orphan\nname: orphan\n",
        )
        .unwrap();
        fs::write(root.join("hitl/test-form/config.json"), "{}").unwrap();
        fs::write(
            root.join("hitl/test-form/meta.yaml"),
            "id: hitl-1\nname: Test\nslug: test-form\n",
        )
        .unwrap();
        fs::write(
            root.join("workflows/test-flow/meta.yaml"),
            "id: \"\"\nname: Test Flow\nslug: test-flow-v1\nworkflowType: test_flow\nversion: 1\n",
        )
        .unwrap();
        fs::write(
            root.join("workflows/test-flow/flow.yaml"),
            r#"initialStage: collect
stages:
  - id: collect
    label: Collect
    kind: collect
    hitlSlug: test-form
    agentSlug: test-agent
    doneWhen: validator_pass
    next: done
  - id: done
    label: Done
    kind: terminal
    doneWhen: terminal
"#,
        )
        .unwrap();
        fs::write(root.join("workflows/test-flow/schema.yaml"), "fields: []\n").unwrap();
    }

    #[test]
    fn broken_agent_ref_emits_broken_ref_edge() {
        let base =
            std::env::temp_dir().join(format!("resmate-graph-broken-{}", uuid::Uuid::new_v4()));
        let _ = fs::remove_dir_all(&base);
        write_minimal_graph_workspace(&base);
        fs::write(
            base.join("assistants/test-assistant.yaml"),
            r#"id: asst-1
slug: test-assistant
agents:
  - missing-agent-id
"#,
        )
        .unwrap();

        let ws = Workspace::detect(&base).expect("workspace");
        let graph = build_graph(&ws).expect("graph");
        assert!(
            graph.edges.iter().any(|e| e.kind == EdgeKind::BrokenRef
                && e.ref_value.as_deref() == Some("missing-agent-id")),
            "expected broken_ref edge for missing agent"
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn empty_workflow_id_sets_missing_id_not_broken_ref() {
        let base =
            std::env::temp_dir().join(format!("resmate-graph-wf-id-{}", uuid::Uuid::new_v4()));
        let _ = fs::remove_dir_all(&base);
        write_minimal_graph_workspace(&base);

        let ws = Workspace::detect(&base).expect("workspace");
        let graph = build_graph(&ws).expect("graph");
        let workflow = graph
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Workflow)
            .expect("workflow node");
        assert!(workflow.missing_id);
        assert!(
            !graph.edges.iter().any(|e| e.kind == EdgeKind::BrokenRef),
            "empty workflow id should not create broken_ref"
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn orphan_tools_listed() {
        let base =
            std::env::temp_dir().join(format!("resmate-graph-orphan-{}", uuid::Uuid::new_v4()));
        let _ = fs::remove_dir_all(&base);
        write_minimal_graph_workspace(&base);

        let ws = Workspace::detect(&base).expect("workspace");
        let graph = build_graph(&ws).expect("graph");
        assert!(
            graph
                .orphans
                .iter()
                .any(|n| n.kind == NodeKind::Tool && n.id == "tool-orphan"),
            "expected orphan tool in orphans list"
        );
        let _ = fs::remove_dir_all(&base);
    }

    fn write_conditional_branch_graph_workspace(root: &Path) {
        fs::create_dir_all(root.join("assistants")).unwrap();
        fs::create_dir_all(root.join("hitl/classify-form")).unwrap();
        fs::create_dir_all(root.join("hitl/fast-track-review")).unwrap();
        fs::create_dir_all(root.join("hitl/standard-form")).unwrap();
        fs::create_dir_all(root.join("workflows/conditional-branch-v1")).unwrap();

        fs::write(root.join("hitl/classify-form/config.json"), "{}").unwrap();
        fs::write(
            root.join("hitl/classify-form/meta.yaml"),
            "id: hitl-classify\nname: Classify\nslug: classify-form\n",
        )
        .unwrap();
        fs::write(root.join("hitl/fast-track-review/config.json"), "{}").unwrap();
        fs::write(
            root.join("hitl/fast-track-review/meta.yaml"),
            "id: hitl-fast\nname: Fast track\nslug: fast-track-review\n",
        )
        .unwrap();
        fs::write(root.join("hitl/standard-form/config.json"), "{}").unwrap();
        fs::write(
            root.join("hitl/standard-form/meta.yaml"),
            "id: hitl-standard\nname: Standard\nslug: standard-form\n",
        )
        .unwrap();

        fs::write(
            root.join("assistants/test-assistant.yaml"),
            r#"id: asst-1
slug: test-assistant
systemContext: |
  workflow_definition_slug: conditional-branch-v1
"#,
        )
        .unwrap();
        fs::write(
            root.join("workflows/conditional-branch-v1/meta.yaml"),
            "id: \"\"\nname: Conditional Branch\nslug: conditional-branch-v1\nworkflowType: conditional_branch\nversion: 1\n",
        )
        .unwrap();
        fs::write(
            root.join("workflows/conditional-branch-v1/flow.yaml"),
            r#"initialStage: classify
stages:
  - id: classify
    label: Classify
    kind: collect
    hitlSlug: classify-form
    doneWhen: [priority]
    transitions:
      - target: fast_track
        when:
          eq:
            field: priority
            value: high
      - target: submit
        when:
          present: skipReview
    next: standard_path
  - id: fast_track
    label: Fast track
    kind: review
    hitlSlug: fast-track-review
    doneWhen: [expressConfirmed]
    next: submit
  - id: standard_path
    label: Standard path
    kind: collect
    hitlSlug: standard-form
    doneWhen: [details]
    next: submit
  - id: submit
    label: Submitted
    kind: terminal
    terminal: true
    doneWhen: terminal
"#,
        )
        .unwrap();
        fs::write(
            root.join("workflows/conditional-branch-v1/schema.yaml"),
            r#"fields:
  - key: priority
    label: Priority
    type: string
    bag: inputs
  - key: skipReview
    label: Skip review
    type: boolean
    bag: inputs
  - key: expressConfirmed
    label: Express confirmed
    type: boolean
    bag: inputs
    requiredFromStage: fast_track
  - key: details
    label: Details
    type: string
    bag: inputs
    requiredFromStage: standard_path
"#,
        )
        .unwrap();
    }

    #[test]
    fn graph_conditional_branch_renders_transition_edges() {
        let base = std::env::temp_dir().join(format!(
            "resmate-graph-conditional-{}",
            uuid::Uuid::new_v4()
        ));
        let _ = fs::remove_dir_all(&base);
        write_conditional_branch_graph_workspace(&base);

        let ws = Workspace::detect(&base).expect("workspace");
        let graph = build_graph(&ws).expect("graph");

        let transition_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::WorkflowStageTransition)
            .collect();
        assert!(
            transition_edges.len() >= 2,
            "expected >=2 transition edges, got {}",
            transition_edges.len()
        );
        assert!(
            transition_edges
                .iter()
                .any(|e| e.ref_value.as_deref() == Some("eq(priority=high)")),
            "expected condition summary label on transition edge"
        );
        assert!(
            graph
                .nodes
                .iter()
                .any(|n| n.kind == NodeKind::WorkflowStage
                    && n.id == "conditional-branch-v1::classify"),
            "expected synthetic stage node"
        );
        let fast_track = graph
            .nodes
            .iter()
            .find(|n| n.id == "conditional-branch-v1::fast_track")
            .expect("fast_track stage node");
        assert!(
            fast_track.slug.contains("[reqFrom: expressConfirmed]"),
            "expected requiredFromStage field keys on stage slug, got {}",
            fast_track.slug
        );
        let standard_path = graph
            .nodes
            .iter()
            .find(|n| n.id == "conditional-branch-v1::standard_path")
            .expect("standard_path stage node");
        assert!(
            standard_path.slug.contains("[reqFrom: details]"),
            "expected requiredFromStage field keys on stage slug, got {}",
            standard_path.slug
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn required_from_stage_fields_by_stage_groups_schema_fields() {
        let bundle = serde_json::json!({
            "schema": {
                "fields": [
                    { "key": "a", "requiredFromStage": "s1" },
                    { "key": "b", "requiredFromStage": "s2" },
                    { "key": "c", "requiredFromStage": "s1" },
                    { "key": "d" }
                ]
            }
        });
        let map = required_from_stage_fields_by_stage(&bundle);
        assert_eq!(
            map.get("s1").map(Vec::as_slice),
            Some(["a".to_string(), "c".to_string()].as_slice())
        );
        assert_eq!(
            map.get("s2").map(Vec::as_slice),
            Some(["b".to_string()].as_slice())
        );
        assert_eq!(format_stage_slug("s1", &map), "s1 [reqFrom: a, c]");
    }

    #[test]
    fn graph_linear_workflow_uses_stage_next_only() {
        let base =
            std::env::temp_dir().join(format!("resmate-graph-linear-{}", uuid::Uuid::new_v4()));
        let _ = fs::remove_dir_all(&base);
        write_minimal_graph_workspace(&base);

        let ws = Workspace::detect(&base).expect("workspace");
        let graph = build_graph(&ws).expect("graph");

        assert!(
            graph
                .edges
                .iter()
                .any(|e| e.kind == EdgeKind::WorkflowStageNext),
            "expected workflow stage next edges for linear flow"
        );
        assert!(
            !graph
                .edges
                .iter()
                .any(|e| e.kind == EdgeKind::WorkflowStageTransition),
            "linear workflow must not emit transition edges"
        );
        assert!(
            !graph
                .edges
                .iter()
                .any(|e| e.kind == EdgeKind::WorkflowStageBackTo),
            "linear workflow must not emit back_to edges"
        );

        let _ = fs::remove_dir_all(&base);
    }

    fn write_rework_loop_graph_workspace(root: &Path) {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/workflows/rework-loop-v1");
        fs::create_dir_all(root.join("assistants")).unwrap();
        fs::create_dir_all(root.join("workflows/rework-loop-v1")).unwrap();
        fs::write(
            root.join("assistants/test-assistant.yaml"),
            r#"id: asst-1
slug: test-assistant
systemContext: |
  workflow_definition_slug: rework-loop-v1
"#,
        )
        .unwrap();
        for name in ["meta.yaml", "flow.yaml", "schema.yaml"] {
            let content = fs::read_to_string(fixture.join(name)).unwrap();
            fs::write(root.join("workflows/rework-loop-v1").join(name), content).unwrap();
        }
    }

    #[test]
    fn graph_rework_loop_renders_back_edge_with_reset() {
        let base = std::env::temp_dir().join(format!(
            "resmate-graph-rework-loop-{}",
            uuid::Uuid::new_v4()
        ));
        let _ = fs::remove_dir_all(&base);
        write_rework_loop_graph_workspace(&base);

        let ws = Workspace::detect(&base).expect("workspace");
        let graph = build_graph(&ws).expect("graph");

        let back_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| {
                e.kind == EdgeKind::WorkflowStageBackTo
                    && e.from == "rework-loop-v1::review_summary"
                    && e.to == "rework-loop-v1::collect_line_items"
            })
            .collect();
        assert_eq!(
            back_edges.len(),
            1,
            "expected one back-edge transition from review_summary to collect_line_items"
        );
        assert_eq!(
            back_edges[0].ref_value.as_deref(),
            Some("eq(reviewConfirmed=false) reset:[reviewConfirmed,lineItems]")
        );

        let forward_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| {
                e.kind == EdgeKind::WorkflowStageTransition
                    && e.from == "rework-loop-v1::review_summary"
                    && e.to == "rework-loop-v1::submit"
            })
            .collect();
        assert_eq!(
            forward_edges.len(),
            1,
            "approve transition should remain a forward transition edge"
        );
        assert_eq!(
            forward_edges[0].ref_value.as_deref(),
            Some("eq(reviewConfirmed=true)")
        );

        let _ = fs::remove_dir_all(&base);
    }

    fn write_named_gates_graph_workspace(root: &Path) {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/workflows/named-gates-v1");
        fs::create_dir_all(root.join("assistants")).unwrap();
        fs::create_dir_all(root.join("workflows/named-gates-v1")).unwrap();
        fs::write(
            root.join("assistants/test-assistant.yaml"),
            r#"id: asst-1
slug: test-assistant
systemContext: |
  workflow_definition_slug: named-gates-v1
"#,
        )
        .unwrap();
        for name in ["meta.yaml", "flow.yaml", "schema.yaml", "gates.yaml"] {
            let content = fs::read_to_string(fixture.join(name)).unwrap();
            fs::write(root.join("workflows/named-gates-v1").join(name), content).unwrap();
        }
    }

    #[test]
    fn graph_named_gates_renders_gate_nodes() {
        let base = std::env::temp_dir().join(format!(
            "resmate-graph-named-gates-{}",
            uuid::Uuid::new_v4()
        ));
        let _ = fs::remove_dir_all(&base);
        write_named_gates_graph_workspace(&base);

        let ws = Workspace::detect(&base).expect("workspace");
        let graph = build_graph(&ws).expect("graph");

        assert!(
            graph.nodes.iter().any(|n| {
                n.kind == NodeKind::WorkflowGate
                    && n.id == "named-gates-v1::gate::is_high_value"
                    && n.slug == "High Value Requisition"
            }),
            "expected is_high_value gate node"
        );
        assert!(
            graph.nodes.iter().any(|n| {
                n.kind == NodeKind::WorkflowGate
                    && n.id == "named-gates-v1::gate::requires_manager_approval"
                    && n.slug == "Requires Manager Approval"
            }),
            "expected requires_manager_approval gate node"
        );

        let composition_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| {
                e.kind == EdgeKind::WorkflowGateRef
                    && e.from == "named-gates-v1::gate::requires_manager_approval"
                    && e.to == "named-gates-v1::gate::is_high_value"
            })
            .collect();
        assert_eq!(
            composition_edges.len(),
            1,
            "expected gate composition edge from requires_manager_approval to is_high_value"
        );
        assert_eq!(
            composition_edges[0].ref_value.as_deref(),
            Some("gate(is_high_value)")
        );

        assert!(
            graph.edges.iter().any(|e| {
                e.kind == EdgeKind::WorkflowGateRef
                    && e.from == "named-gates-v1::collect_request"
                    && e.to == "named-gates-v1::gate::requires_manager_approval"
                    && e.ref_value.as_deref() == Some("gate(requires_manager_approval)")
            }),
            "expected stage-to-gate edge from collect_request transition"
        );

        assert!(
            graph.edges.iter().any(|e| {
                e.kind == EdgeKind::WorkflowStageTransition
                    && e.from == "named-gates-v1::collect_request"
                    && e.to == "named-gates-v1::await_approval"
                    && e.ref_value.as_deref() == Some("gate(requires_manager_approval)")
            }),
            "transition edge should retain gate(id) label via condition_summary"
        );

        let _ = fs::remove_dir_all(&base);
    }
}
