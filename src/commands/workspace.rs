use crate::output::{emit_error, emit_success, CliContext, CliExitCode, OutputMode};
use crate::workspace::{Workspace, WorkspaceError};
use std::process::ExitCode;

pub fn run_workspace_info(ctx: &CliContext) -> ExitCode {
    match Workspace::detect(std::path::Path::new("")) {
        Ok(ws) => match ws.info() {
            Ok(info) => {
                match ctx.mode {
                    OutputMode::Human => print_human_workspace_info(&info),
                    OutputMode::Json => emit_success(ctx, info),
                }
                CliExitCode::Success.into()
            }
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

fn print_human_workspace_info(info: &crate::workspace::WorkspaceInfo) {
    println!("Workspace root: {}", info.root);
    println!("Detected: {}", info.detected);
    println!();
    println!("Artifact directories:");
    for kind in ["tools", "agents", "assistants", "hitl", "workflows"] {
        if let Some(dir) = info.artifact_dirs.get(kind) {
            println!(
                "  {}: {} (exists: {}, count: {})",
                kind, dir.path, dir.exists, dir.count
            );
        }
    }
    println!();
    println!(
        "Counts: {} tool(s), {} agent(s), {} assistant(s), {} hitl, {} workflow(s)",
        info.counts.tools,
        info.counts.agents,
        info.counts.assistants,
        info.counts.hitl,
        info.counts.workflows
    );
    println!();
    println!("base_url: {}", info.config.base_url);
    println!("api_key: {}", info.config.api_key.display);
    println!("roc_session: {}", info.config.roc_session.display);
    println!(
        "Config file: {}",
        info.config.config_file.as_deref().unwrap_or("none")
    );
}
