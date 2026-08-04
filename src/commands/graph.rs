use crate::graph::{build_graph_filtered, GraphError};
use crate::output::{emit_error, emit_success, CliContext, CliExitCode, OutputMode};
use crate::workspace::{Workspace, WorkspaceError};
use std::process::ExitCode;

pub fn run_graph(ctx: &CliContext, assistant: Option<&str>) -> ExitCode {
    match Workspace::detect(std::path::Path::new("")) {
        Ok(ws) => match build_graph_filtered(&ws, assistant) {
            Ok(graph) => {
                match ctx.mode {
                    OutputMode::Human => print_human_graph(&graph),
                    OutputMode::Json => emit_success(ctx, graph),
                }
                CliExitCode::Success.into()
            }
            Err(GraphError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
                emit_error(ctx, "GRAPH_BUILD_FAILED", e, None, CliExitCode::UsageError)
            }
            Err(GraphError::Io(e)) => emit_error(
                ctx,
                "GRAPH_BUILD_FAILED",
                e,
                None,
                CliExitCode::RuntimeError,
            ),
        },
        Err(WorkspaceError::NotFound(msg)) => emit_error(
            ctx,
            "WORKSPACE_NOT_FOUND",
            msg,
            None,
            CliExitCode::RuntimeError,
        ),
        Err(WorkspaceError::Io(e)) => emit_error(
            ctx,
            "WORKSPACE_NOT_FOUND",
            e,
            None,
            CliExitCode::RuntimeError,
        ),
    }
}

fn print_human_graph(graph: &crate::graph::Graph) {
    println!("Dependency graph");
    println!();
    println!("Nodes ({}):", graph.nodes.len());
    for node in &graph.nodes {
        let missing = if node.missing_id { " missing_id" } else { "" };
        println!(
            "  [{:?}] {} ({}) — {}{}",
            node.kind, node.slug, node.id, node.path, missing
        );
    }
    println!();
    println!("Edges ({}):", graph.edges.len());
    for edge in &graph.edges {
        let marker = if edge.kind == crate::graph::EdgeKind::BrokenRef {
            " [BROKEN]"
        } else {
            ""
        };
        let label = edge
            .ref_value
            .as_deref()
            .map(|value| format!(" [{value}]"))
            .unwrap_or_default();
        println!(
            "  {} -> {} ({:?}){}{}",
            edge.from, edge.to, edge.kind, label, marker
        );
    }
    if !graph.orphans.is_empty() {
        println!();
        println!("Orphans ({}):", graph.orphans.len());
        for node in &graph.orphans {
            println!("  [{:?}] {} ({})", node.kind, node.slug, node.id);
        }
    }
}
