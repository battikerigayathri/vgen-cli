//! Integration tests for dependency graph against the pr-agent-v2 workspace.

use resmate::graph::{build_graph, build_graph_filtered, EdgeKind, NodeKind};
use resmate::workspace::Workspace;
use std::collections::HashSet;
use std::path::PathBuf;

fn pr_agent_v2_root() -> PathBuf {
    PathBuf::from("/Users/roshankgujarathi/Workspace/ResMed/pr-agent-v2")
}

#[test]
fn pr_agent_v2_graph_has_all_node_kinds_and_no_broken_refs() {
    let root = pr_agent_v2_root();
    if !root.is_dir() {
        eprintln!("Skipping: pr-agent-v2 not found at {}", root.display());
        return;
    }

    let ws = Workspace::detect(&root).expect("detect pr-agent-v2");
    let graph = build_graph(&ws).expect("build graph");

    let kinds: HashSet<String> = graph
        .nodes
        .iter()
        .map(|n| format!("{:?}", n.kind).to_lowercase())
        .collect();
    for expected in ["assistant", "agent", "tool", "hitl", "workflow"] {
        assert!(
            kinds.contains(expected),
            "missing node kind {expected}; got {:?}",
            kinds
        );
    }

    let broken = graph
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::BrokenRef)
        .count();
    assert_eq!(broken, 0, "expected zero broken_ref edges");

    let workflow = graph
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Workflow)
        .expect("workflow node");
    // workflow.missing_id can be true or false depending on whether it's pushed
    assert_eq!(workflow.slug, "oracle-purchase-requisition-v1");
}

#[test]
fn pr_agent_v2_oracle_assistant_subtree_is_connected() {
    let root = pr_agent_v2_root();
    if !root.is_dir() {
        eprintln!("Skipping: pr-agent-v2 not found at {}", root.display());
        return;
    }

    let ws = Workspace::detect(&root).expect("detect pr-agent-v2");
    let graph = build_graph_filtered(&ws, Some("oracle-pr-assistant")).expect("filtered graph");

    assert!(
        graph.nodes.len() >= 7,
        "expected at least 7 nodes in oracle-pr subtree; got {}",
        graph.nodes.len()
    );
    assert!(
        !graph.edges.iter().any(|e| e.kind == EdgeKind::BrokenRef),
        "filtered graph should have no broken refs"
    );

    let assistant = graph
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Assistant)
        .expect("assistant node");
    assert_eq!(assistant.slug, "oracle-pr-assistant");

    let reachable_ids: HashSet<&str> = graph.nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(reachable_ids.len() >= 7);

    let tool_count = graph
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Tool)
        .count();
    assert!(tool_count >= 3);
}

#[test]
fn pr_agent_v2_has_no_orphan_tools_when_all_linked() {
    let root = pr_agent_v2_root();
    if !root.is_dir() {
        eprintln!("Skipping: pr-agent-v2 not found at {}", root.display());
        return;
    }

    let ws = Workspace::detect(&root).expect("detect pr-agent-v2");
    let graph = build_graph(&ws).expect("build graph");

    let orphan_tools: Vec<_> = graph
        .orphans
        .iter()
        .filter(|n| n.kind == NodeKind::Tool)
        .collect();
    assert!(
        orphan_tools.is_empty(),
        "all pr-agent-v2 tools are referenced by oracle-pr-agent; orphans: {:?}",
        orphan_tools
    );
}
