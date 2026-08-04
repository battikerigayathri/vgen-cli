use crate::graph::{EdgeKind, Graph, GraphNode, NodeKind};
use crate::specs::{
    list_agent_files, list_assistant_files, list_hitl_dirs, list_tool_dirs, load_tool_yaml,
    ResourceIndex,
};
use crate::workspace::Workspace;

use super::{
    push_finding, read_yaml, relative_path, string_field, Finding, ResourceKind, ResourceRef,
    Severity,
};

pub fn check_graph_rules(ws: &Workspace, graph: &Graph, index: &ResourceIndex) -> Vec<Finding> {
    let mut findings = Vec::new();
    check_broken_refs(ws, graph, index, &mut findings);
    check_orphans(graph, &mut findings);
    check_duplicate_ids(ws, &mut findings);
    findings
}

fn check_broken_refs(
    ws: &Workspace,
    graph: &Graph,
    index: &ResourceIndex,
    findings: &mut Vec<Finding>,
) {
    let nodes_by_id: std::collections::HashMap<&str, &GraphNode> =
        graph.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    for edge in &graph.edges {
        if edge.kind != EdgeKind::BrokenRef {
            continue;
        }
        let Some(from_node) = nodes_by_id.get(edge.from.as_str()) else {
            continue;
        };
        let ref_value = edge.ref_value.as_deref().unwrap_or(edge.to.as_str());
        match from_node.kind {
            NodeKind::Assistant => {
                if assistant_agents_contains(ws, from_node, ref_value) {
                    push_finding(
                        findings,
                        Finding {
                            code: "BROKEN_AGENT_REF".to_string(),
                            severity: Severity::Error,
                            message: format!("assistant references unknown agent id `{ref_value}`"),
                            path: Some(from_node.path.clone()),
                            resource: Some(ResourceRef {
                                kind: ResourceKind::Assistant,
                                slug: Some(from_node.slug.clone()),
                                id: if from_node.missing_id {
                                    None
                                } else {
                                    Some(from_node.id.clone())
                                },
                            }),
                        },
                    );
                }
            }
            NodeKind::Agent => {
                push_finding(
                    findings,
                    Finding {
                        code: "BROKEN_TOOL_REF".to_string(),
                        severity: Severity::Error,
                        message: format!("agent references unknown tool id `{ref_value}`"),
                        path: Some(from_node.path.clone()),
                        resource: Some(ResourceRef {
                            kind: ResourceKind::Agent,
                            slug: Some(from_node.slug.clone()),
                            id: if from_node.missing_id {
                                None
                            } else {
                                Some(from_node.id.clone())
                            },
                        }),
                    },
                );
            }
            NodeKind::Workflow => {
                let _ = index;
                if workflow_stage_hitl_slug(ws, from_node, ref_value) {
                    push_finding(
                        findings,
                        Finding {
                            code: "HITL_SLUG_UNRESOLVED".to_string(),
                            severity: Severity::Error,
                            message: format!(
                                "workflow stage references unknown hitl slug `{ref_value}`"
                            ),
                            path: Some(from_node.path.clone()),
                            resource: Some(ResourceRef {
                                kind: ResourceKind::Workflow,
                                slug: Some(from_node.slug.clone()),
                                id: if from_node.missing_id {
                                    None
                                } else {
                                    Some(from_node.id.clone())
                                },
                            }),
                        },
                    );
                } else {
                    push_finding(
                        findings,
                        Finding {
                            code: "WORKFLOW_AGENT_UNRESOLVED".to_string(),
                            severity: Severity::Error,
                            message: format!(
                                "workflow stage references unknown agent slug `{ref_value}`"
                            ),
                            path: Some(from_node.path.clone()),
                            resource: Some(ResourceRef {
                                kind: ResourceKind::Workflow,
                                slug: Some(from_node.slug.clone()),
                                id: if from_node.missing_id {
                                    None
                                } else {
                                    Some(from_node.id.clone())
                                },
                            }),
                        },
                    );
                }
            }
            _ => {}
        }
    }
}

fn assistant_agents_contains(ws: &Workspace, node: &GraphNode, agent_id: &str) -> bool {
    let path = ws.root.join(&node.path);
    let Ok(value) = read_yaml(&path) else {
        return false;
    };
    value
        .get("agents")
        .and_then(|v| v.as_array())
        .is_some_and(|agents| {
            agents
                .iter()
                .filter_map(|v| v.as_str())
                .any(|id| id == agent_id)
        })
}

fn workflow_stage_hitl_slug(ws: &Workspace, node: &GraphNode, slug: &str) -> bool {
    let workflow_dir = workflow_dir_from_node_path(ws, node);
    let Ok(flow) = read_workflow_flow(&workflow_dir) else {
        return false;
    };
    flow.get("stages")
        .and_then(|v| v.as_array())
        .is_some_and(|stages| {
            stages.iter().any(|stage| {
                stage
                    .get("hitlSlug")
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| s == slug)
            })
        })
}

fn read_workflow_flow(workflow_dir: &std::path::Path) -> Result<serde_json::Value, std::io::Error> {
    for name in ["flow.yaml", "flow.yml"] {
        let path = workflow_dir.join(name);
        if path.is_file() {
            return read_yaml(&path);
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "flow.yaml not found",
    ))
}

fn workflow_dir_from_node_path(ws: &Workspace, node: &GraphNode) -> std::path::PathBuf {
    let path = std::path::Path::new(&node.path);
    if path.components().count() > 1 {
        ws.root.join(path)
    } else {
        ws.workflows_dir.join(path.file_name().unwrap_or_default())
    }
}

fn check_orphans(graph: &Graph, findings: &mut Vec<Finding>) {
    for node in &graph.orphans {
        match node.kind {
            NodeKind::Agent => {
                push_finding(
                    findings,
                    Finding {
                        code: "ORPHAN_AGENT".to_string(),
                        severity: Severity::Warning,
                        message: format!(
                            "agent `{}` is not referenced by any assistant",
                            node.slug
                        ),
                        path: Some(node.path.clone()),
                        resource: Some(ResourceRef {
                            kind: ResourceKind::Agent,
                            slug: Some(node.slug.clone()),
                            id: if node.missing_id {
                                None
                            } else {
                                Some(node.id.clone())
                            },
                        }),
                    },
                );
            }
            NodeKind::Tool => {
                push_finding(
                    findings,
                    Finding {
                        code: "ORPHAN_TOOL".to_string(),
                        severity: Severity::Warning,
                        message: format!("tool `{}` is not referenced by any agent", node.slug),
                        path: Some(node.path.clone()),
                        resource: Some(ResourceRef {
                            kind: ResourceKind::Tool,
                            slug: Some(node.slug.clone()),
                            id: if node.missing_id {
                                None
                            } else {
                                Some(node.id.clone())
                            },
                        }),
                    },
                );
            }
            _ => {}
        }
    }
}

fn check_duplicate_ids(ws: &Workspace, findings: &mut Vec<Finding>) {
    check_duplicate_for_files(
        ws,
        findings,
        list_assistant_files(&ws.assistants_dir),
        ResourceKind::Assistant,
    );
    check_duplicate_for_files(
        ws,
        findings,
        list_agent_files(&ws.agents_dir),
        ResourceKind::Agent,
    );
    check_duplicate_for_tool_dirs(ws, findings);
    check_duplicate_for_hitl_dirs(ws, findings);
}

fn check_duplicate_for_files(
    ws: &Workspace,
    findings: &mut Vec<Finding>,
    files: Vec<(String, std::path::PathBuf)>,
    kind: ResourceKind,
) {
    let mut by_id: std::collections::HashMap<String, Vec<std::path::PathBuf>> =
        std::collections::HashMap::new();
    for (_stem, path) in files {
        let Ok(value) = read_yaml(&path) else {
            continue;
        };
        if let Some(id) = string_field(&value, "id") {
            by_id.entry(id).or_default().push(path);
        }
    }
    emit_duplicate_findings(ws, findings, kind, &by_id);
}

fn check_duplicate_for_tool_dirs(ws: &Workspace, findings: &mut Vec<Finding>) {
    let mut by_id: std::collections::HashMap<String, Vec<std::path::PathBuf>> =
        std::collections::HashMap::new();
    for tool_dir in list_tool_dirs(&ws.tools_dir) {
        let Ok((value, yaml_path)) = load_tool_yaml(&tool_dir) else {
            continue;
        };
        if let Some(id) = string_field(&value, "id") {
            by_id.entry(id).or_default().push(yaml_path);
        }
    }
    emit_duplicate_findings(ws, findings, ResourceKind::Tool, &by_id);
}

fn check_duplicate_for_hitl_dirs(ws: &Workspace, findings: &mut Vec<Finding>) {
    let mut by_id: std::collections::HashMap<String, Vec<std::path::PathBuf>> =
        std::collections::HashMap::new();
    for hitl_dir in list_hitl_dirs(&ws.hitl_dir) {
        let meta_path = if hitl_dir.join("meta.yaml").is_file() {
            hitl_dir.join("meta.yaml")
        } else {
            hitl_dir.join("meta.yml")
        };
        let Ok(value) = read_yaml(&meta_path) else {
            continue;
        };
        if let Some(id) = string_field(&value, "id") {
            by_id.entry(id).or_default().push(meta_path);
        }
    }
    emit_duplicate_findings(ws, findings, ResourceKind::Hitl, &by_id);
}

fn emit_duplicate_findings(
    ws: &Workspace,
    findings: &mut Vec<Finding>,
    kind: ResourceKind,
    by_id: &std::collections::HashMap<String, Vec<std::path::PathBuf>>,
) {
    for (id, paths) in by_id {
        if paths.len() < 2 {
            continue;
        }
        let path_strs: Vec<String> = paths.iter().map(|p| relative_path(p, &ws.root)).collect();
        push_finding(
            findings,
            Finding {
                code: "DUPLICATE_ID".to_string(),
                severity: Severity::Error,
                message: format!("duplicate id `{id}` in {:?}", path_strs),
                path: path_strs.first().cloned(),
                resource: Some(ResourceRef {
                    kind,
                    slug: None,
                    id: Some(id.clone()),
                }),
            },
        );
    }
}
