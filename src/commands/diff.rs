use crate::diff::{parse_resource_type, DiffError, DiffReport};
use crate::output::{emit_error, emit_success, CliContext, CliExitCode, OutputMode};
use crate::workspace::{Workspace, WorkspaceError};
use std::process::ExitCode;

pub async fn run_diff(ctx: &CliContext, resource_type: Option<&str>, name: &str) -> ExitCode {
    let Some(raw_type) = resource_type else {
        return emit_error(
            ctx,
            "USAGE_ERROR",
            "resource type required: tool|agent|assistant|hitl|workflow",
            None,
            CliExitCode::UsageError,
        );
    };
    let Some(resource_type) = parse_resource_type(raw_type) else {
        return emit_error(
            ctx,
            "USAGE_ERROR",
            format!("unknown resource type `{raw_type}`"),
            None,
            CliExitCode::UsageError,
        );
    };

    match Workspace::detect(std::path::Path::new("")) {
        Ok(ws) => match crate::diff::diff_resource(&ws, resource_type, name).await {
            Ok(report) => finish_diff(ctx, &report),
            Err(DiffError::NotFound(msg)) => emit_error(
                ctx,
                "RESOURCE_NOT_FOUND",
                msg,
                None,
                CliExitCode::UsageError,
            ),
            Err(DiffError::NoId(msg)) => {
                emit_error(ctx, "MISSING_ID", msg, None, CliExitCode::UsageError)
            }
            Err(DiffError::Remote(msg)) => emit_error(
                ctx,
                "REMOTE_LOOKUP_FAILED",
                msg,
                None,
                CliExitCode::RuntimeError,
            ),
            Err(DiffError::Io(e)) => emit_error(
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

fn finish_diff(ctx: &CliContext, report: &DiffReport) -> ExitCode {
    match ctx.mode {
        OutputMode::Human => print_human_diff(report),
        OutputMode::Json => emit_success(ctx, report),
    }
    CliExitCode::Success.into()
}

fn print_human_diff(report: &DiffReport) {
    println!(
        "Diff {:?} `{}` — changes: {}",
        report.resource_type, report.name, report.has_changes
    );
    if report.remote_missing {
        if let Some(msg) = &report.message {
            println!("Remote: {msg}");
        }
        return;
    }
    if report.hunks.is_empty() {
        println!("No field differences.");
        return;
    }
    for hunk in &report.hunks {
        println!("  {}:", hunk.field);
        println!("    local:  {}", value_line(&hunk.local));
        println!("    remote: {}", value_line(&hunk.remote));
    }
}

fn value_line(value: &Option<serde_json::Value>) -> String {
    match value {
        Some(v) => v.to_string(),
        None => "(missing)".to_string(),
    }
}
