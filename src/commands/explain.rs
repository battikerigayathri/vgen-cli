use crate::errors_registry::{list_all, list_by_domain, lookup, ErrorCodeDoc};
use crate::output::{emit_error, emit_success, CliContext, CliExitCode, OutputMode};
use serde::Serialize;
use std::process::ExitCode;

#[derive(Debug, Serialize)]
pub struct ExplainListData {
    pub codes: Vec<ErrorCodeDoc>,
}

pub fn run_explain(
    ctx: &CliContext,
    code: Option<&str>,
    list: bool,
    domain: Option<&str>,
) -> ExitCode {
    if list {
        return run_list(ctx, domain);
    }

    let Some(code) = code else {
        return emit_error(
            ctx,
            "USAGE_ERROR",
            "Provide an error code or use --list",
            None,
            CliExitCode::UsageError,
        );
    };

    match lookup(code) {
        Some(doc) => {
            match ctx.mode {
                OutputMode::Human => print_human_doc(&doc),
                OutputMode::Json => emit_success(ctx, doc),
            }
            CliExitCode::Success.into()
        }
        None => emit_error(
            ctx,
            "UNKNOWN_ERROR_CODE",
            format!("Unknown error code: {}", code),
            Some(serde_json::json!({ "requested": code })),
            CliExitCode::UsageError,
        ),
    }
}

fn run_list(ctx: &CliContext, domain: Option<&str>) -> ExitCode {
    let codes = match domain {
        Some(d) => list_by_domain(d),
        None => list_all(),
    };

    match ctx.mode {
        OutputMode::Human => {
            if codes.is_empty() {
                println!("No error codes found.");
            } else {
                for doc in &codes {
                    println!("{} [{}] — {}", doc.code, doc.domain, doc.description);
                }
            }
        }
        OutputMode::Json => emit_success(ctx, ExplainListData { codes }),
    }
    CliExitCode::Success.into()
}

fn print_human_doc(doc: &ErrorCodeDoc) {
    println!("Code: {}", doc.code);
    println!("Domain: {}", doc.domain);
    if let Some(sev) = &doc.severity {
        println!("Severity: {}", sev);
    }
    if let Some(exit) = doc.exit_code {
        println!("Exit code: {}", exit);
    }
    println!();
    println!("Description:");
    println!("  {}", doc.description);
    if !doc.remediation.is_empty() {
        println!();
        println!("Remediation:");
        for hint in &doc.remediation {
            println!("  - {}", hint);
        }
    }
}
