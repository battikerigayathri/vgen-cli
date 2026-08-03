use crate::diff::check_step_remote_drift;
use crate::graph::{build_graph_filtered, GraphError, GraphNode, NodeKind};
use crate::remote_cache::RemoteCache;
use crate::validate::{ResourceKind, Severity, ValidationReport};
use crate::workspace::Workspace;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PushAction {
    Create,
    Update,
    Skip,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResourceType {
    Hitl,
    Workflow,
    Tool,
    Agent,
    Assistant,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PushStep {
    pub order: u32,
    pub resource_type: ResourceType,
    pub name: String,
    pub path: String,
    pub action: PushAction,
    pub has_id: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blockers: Vec<String>,
    #[serde(default)]
    pub remote_drift: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drift_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PushPlan {
    pub steps: Vec<PushStep>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<PushStep>,
}

pub fn build_push_plan(
    ws: &Workspace,
    report: &ValidationReport,
    assistant_filter: Option<&str>,
) -> Result<PushPlan, GraphError> {
    let graph = build_graph_filtered(ws, assistant_filter)?;
    let orphan_ids: HashSet<&str> = graph.orphans.iter().map(|n| n.id.as_str()).collect();

    let mut reachable: Vec<&GraphNode> = graph
        .nodes
        .iter()
        .filter(|n| {
            !orphan_ids.contains(n.id.as_str())
                && n.kind != NodeKind::WorkflowStage
                && n.kind != NodeKind::WorkflowGate
        })
        .collect();

    reachable.sort_by(|a, b| {
        (
            phase_order(&a.kind),
            push_name_from_node(a).as_str(),
            a.path.as_str(),
        )
            .cmp(&(
                phase_order(&b.kind),
                push_name_from_node(b).as_str(),
                b.path.as_str(),
            ))
    });

    let steps: Vec<PushStep> = reachable
        .iter()
        .enumerate()
        .map(|(idx, node)| {
            node_to_step(
                node,
                idx as u32 + 1,
                report,
                PushAction::from_missing_id(node),
            )
        })
        .collect();

    let skipped: Vec<PushStep> = graph
        .orphans
        .iter()
        .filter(|n| n.kind != NodeKind::WorkflowStage && n.kind != NodeKind::WorkflowGate)
        .enumerate()
        .map(|(idx, node)| node_to_step(node, idx as u32 + 1, report, PushAction::Skip))
        .collect();

    Ok(PushPlan { steps, skipped })
}

pub async fn enrich_push_plan_remote_drift(ws: &Workspace, plan: &mut PushPlan) {
    let Ok(cfg) = crate::config::load_config() else {
        return;
    };
    if cfg.api_key.as_deref().unwrap_or("").is_empty() {
        return;
    }

    let mut cache = RemoteCache::new();
    for step in &mut plan.steps {
        if !step.has_id || step.action == PushAction::Skip {
            continue;
        }
        let (drift, fields) =
            check_step_remote_drift(ws, step.resource_type, &step.name, step.has_id, &mut cache)
                .await;
        step.remote_drift = drift;
        step.drift_fields = fields;
    }
}

fn node_to_step(
    node: &GraphNode,
    order: u32,
    report: &ValidationReport,
    action: PushAction,
) -> PushStep {
    let has_id = !node.missing_id;
    let id = if has_id { Some(node.id.clone()) } else { None };
    PushStep {
        order,
        resource_type: node_kind_to_resource_type(&node.kind),
        name: push_name_from_node(node),
        path: node.path.clone(),
        action,
        has_id,
        id,
        blockers: blockers_for_node(node, report),
        remote_drift: false,
        drift_fields: Vec::new(),
    }
}

impl PushAction {
    fn from_missing_id(node: &GraphNode) -> Self {
        if node.missing_id {
            PushAction::Create
        } else {
            PushAction::Update
        }
    }
}

fn phase_order(kind: &NodeKind) -> u8 {
    match kind {
        NodeKind::Hitl => 0,
        NodeKind::Workflow => 1,
        NodeKind::Tool => 2,
        NodeKind::Agent => 3,
        NodeKind::Assistant => 4,
        NodeKind::WorkflowStage => 5,
        NodeKind::WorkflowGate => 6,
    }
}

fn node_kind_to_resource_type(kind: &NodeKind) -> ResourceType {
    match kind {
        NodeKind::Hitl => ResourceType::Hitl,
        NodeKind::Workflow => ResourceType::Workflow,
        NodeKind::Tool => ResourceType::Tool,
        NodeKind::Agent => ResourceType::Agent,
        NodeKind::Assistant => ResourceType::Assistant,
        NodeKind::WorkflowStage | NodeKind::WorkflowGate => ResourceType::Workflow,
    }
}

fn push_name_from_node(node: &GraphNode) -> String {
    let path = Path::new(&node.path);
    match node.kind {
        NodeKind::Workflow => path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&node.slug)
            .to_string(),
        NodeKind::Hitl | NodeKind::Tool => path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or(&node.slug)
            .to_string(),
        NodeKind::Agent | NodeKind::Assistant => path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&node.slug)
            .to_string(),
        NodeKind::WorkflowStage | NodeKind::WorkflowGate => node.slug.clone(),
    }
}

fn blockers_for_node(node: &GraphNode, report: &ValidationReport) -> Vec<String> {
    let mut codes: Vec<String> = report
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .filter(|f| finding_matches_node(f, node))
        .map(|f| f.code.clone())
        .collect();
    codes.sort();
    codes.dedup();
    codes
}

fn finding_matches_node(finding: &crate::validate::Finding, node: &GraphNode) -> bool {
    if let Some(resource) = &finding.resource {
        if resource_kind_matches_node(&resource.kind, &node.kind) {
            if let Some(slug) = &resource.slug {
                if slug == &node.slug {
                    return true;
                }
            }
            if let Some(id) = &resource.id {
                if id == &node.id {
                    return true;
                }
            }
        }
    }
    if let Some(path) = &finding.path {
        if path == &node.path {
            return true;
        }
        if node.path.starts_with(path) || path.starts_with(&node.path) {
            return true;
        }
        let name = push_name_from_node(node);
        if path.contains(&name) {
            return true;
        }
    }
    false
}

fn resource_kind_matches_node(kind: &ResourceKind, node_kind: &NodeKind) -> bool {
    matches!(
        (kind, node_kind),
        (ResourceKind::Assistant, NodeKind::Assistant)
            | (ResourceKind::Agent, NodeKind::Agent)
            | (ResourceKind::Tool, NodeKind::Tool)
            | (ResourceKind::Hitl, NodeKind::Hitl)
            | (ResourceKind::Workflow, NodeKind::Workflow)
    )
}
