use crate::output::{emit_error, emit_success, CliContext, CliExitCode, OutputMode};
use crate::workflow_validate::{validate_workflow_dir, WorkflowValidateData};
use std::path::PathBuf;
use std::process::ExitCode;

pub fn run_workflow_validate(
    ctx: &CliContext,
    name: &str,
    workflows_dir: Option<PathBuf>,
) -> ExitCode {
    let base = workflows_dir.unwrap_or_else(crate::specs::default_workflows_dir);
    let dir = base.join(name);

    match validate_workflow_dir(&dir) {
        Ok(data) => {
            match ctx.mode {
                OutputMode::Human => print_human_success(name, &data),
                OutputMode::Json => emit_success(ctx, data),
            }
            CliExitCode::Success.into()
        }
        Err(err) => emit_error(
            ctx,
            err.code,
            err.message,
            err.details,
            CliExitCode::ValidationFailed,
        ),
    }
}

fn print_human_success(name: &str, data: &WorkflowValidateData) {
    println!("Validated workflow: {name}");
    println!("  slug: {}", data.slug);
    println!("  version: {}", data.version);
    println!("  source_format: {}", data.source_format);
    println!("  path: {}", data.path);
    for hint in &data.hints {
        println!("  Hint: {hint}");
    }
}
