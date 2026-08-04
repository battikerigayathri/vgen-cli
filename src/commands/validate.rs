use crate::output::{emit_error, emit_success, CliContext, CliExitCode, OutputMode};
use crate::validate::{
    run_workspace_validation_async, Severity, ValidateError, ValidateOptions, ValidationReport,
};
use crate::workspace::{Workspace, WorkspaceError};
use std::process::ExitCode;

pub async fn run_validate(ctx: &CliContext, strict: bool, offline: bool, remote: bool) -> ExitCode {
    match Workspace::detect(std::path::Path::new("")) {
        Ok(ws) => {
            match run_workspace_validation_async(
                &ws,
                ValidateOptions {
                    strict,
                    offline,
                    remote,
                },
            )
            .await
            {
                Ok(report) => finish_validate(ctx, &report, strict),
                Err(ValidateError::Graph(e)) => emit_error(
                    ctx,
                    "GRAPH_BUILD_FAILED",
                    e,
                    None,
                    CliExitCode::RuntimeError,
                ),
            }
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
    }
}

fn finish_validate(ctx: &CliContext, report: &ValidationReport, strict: bool) -> ExitCode {
    match ctx.mode {
        OutputMode::Human => print_human_report(report),
        OutputMode::Json => emit_success(ctx, report),
    }
    exit_for_report(report, strict).into()
}

fn exit_for_report(report: &ValidationReport, strict: bool) -> CliExitCode {
    if report.summary.error_count > 0 {
        return CliExitCode::ValidationFailed;
    }
    if strict && report.summary.warning_count > 0 {
        return CliExitCode::ValidationFailed;
    }
    CliExitCode::Success
}

fn print_human_report(report: &ValidationReport) {
    println!("Workspace validation");
    println!();
    println!(
        "Summary: {} error(s), {} warning(s), {} info",
        report.summary.error_count, report.summary.warning_count, report.summary.info_count
    );
    if report.findings.is_empty() {
        println!();
        println!("No findings.");
        return;
    }
    println!();
    for finding in &report.findings {
        let severity = match finding.severity {
            Severity::Error => "ERROR",
            Severity::Warning => "WARN",
            Severity::Info => "INFO",
        };
        let path = finding
            .path
            .as_deref()
            .map(|p| format!(" ({p})"))
            .unwrap_or_default();
        println!("[{severity}] {}: {}{}", finding.code, finding.message, path);
    }
}
