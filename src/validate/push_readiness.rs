use crate::graph::{Graph, NodeKind};
use crate::specs::{list_agent_files, load_tool_yaml, ResourceIndex};
use crate::workspace::Workspace;

use super::{
    push_finding, read_yaml, relative_path, string_field, Finding, ResourceKind, ResourceRef,
    Severity,
};

pub fn check_push_readiness(ws: &Workspace, graph: &Graph, index: &ResourceIndex) -> Vec<Finding> {
    let mut findings = Vec::new();
    for node in &graph.nodes {
        if !node.missing_id
            || node.kind == NodeKind::WorkflowStage
            || node.kind == NodeKind::WorkflowGate
        {
            continue;
        }
        push_finding(
            &mut findings,
            Finding {
                code: "MISSING_ID".to_string(),
                severity: Severity::Info,
                message: format!(
                    "{} `{}` has no platform id; push will create a new resource",
                    kind_label(&node.kind),
                    node.slug
                ),
                path: Some(node.path.clone()),
                resource: Some(ResourceRef {
                    kind: graph_kind_to_resource(&node.kind),
                    slug: Some(node.slug.clone()),
                    id: None,
                }),
            },
        );
    }

    check_missing_id_blocking(ws, index, &mut findings);
    findings
}

fn kind_label(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Assistant => "assistant",
        NodeKind::Agent => "agent",
        NodeKind::Tool => "tool",
        NodeKind::Hitl => "HITL",
        NodeKind::Workflow => "workflow",
        NodeKind::WorkflowStage => "workflow stage",
        NodeKind::WorkflowGate => "workflow gate",
    }
}

fn graph_kind_to_resource(kind: &NodeKind) -> ResourceKind {
    match kind {
        NodeKind::Assistant => ResourceKind::Assistant,
        NodeKind::Agent => ResourceKind::Agent,
        NodeKind::Tool => ResourceKind::Tool,
        NodeKind::Hitl => ResourceKind::Hitl,
        NodeKind::Workflow => ResourceKind::Workflow,
        NodeKind::WorkflowStage | NodeKind::WorkflowGate => ResourceKind::Workflow,
    }
}

fn check_missing_id_blocking(ws: &Workspace, index: &ResourceIndex, findings: &mut Vec<Finding>) {
    for (_stem, agent_path) in list_agent_files(&ws.agents_dir) {
        let Ok(agent_value) = read_yaml(&agent_path) else {
            continue;
        };
        let agent_has_id = string_field(&agent_value, "id").is_some();
        if !agent_has_id {
            continue;
        }
        let Some(skills) = agent_value.get("skills").and_then(|v| v.as_array()) else {
            continue;
        };
        for skill_ref in skills {
            let Some(tool_id) = skill_ref.as_str().map(str::trim).filter(|s| !s.is_empty()) else {
                continue;
            };
            let Some(tool_dir) = index.tools_by_id.get(tool_id) else {
                continue;
            };
            let Ok((tool_value, yaml_path)) = load_tool_yaml(tool_dir) else {
                continue;
            };
            if string_field(&tool_value, "id").is_some() {
                continue;
            }
            let slug = tool_dir
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            push_finding(
                findings,
                Finding {
                    code: "MISSING_ID_BLOCKING".to_string(),
                    severity: Severity::Warning,
                    message: format!(
                        "tool `{slug}` has no id but is referenced by agent with id; push tool before agent"
                    ),
                    path: Some(relative_path(&yaml_path, &ws.root)),
                    resource: Some(ResourceRef {
                        kind: ResourceKind::Tool,
                        slug: Some(slug),
                        id: None,
                    }),
                },
            );
        }
    }
}
