use crate::graph::GraphError;
use crate::output::{emit_error, emit_success, CliContext, CliExitCode, OutputMode};
use crate::push_executor::{
    execute_push_plan, preflight_blockers, PushExecuteOptions, PushExecutorError,
};
use crate::push_plan::{
    build_push_plan, enrich_push_plan_remote_drift, PushAction, PushPlan, PushStep,
};
use crate::validate::{run_workspace_validation, ValidateError, ValidateOptions};
use crate::workspace::{Workspace, WorkspaceError};
use std::process::ExitCode;

pub async fn run_push_all(
    ctx: &CliContext,
    dry_run: bool,
    yes: bool,
    force: bool,
    stop_on_error: bool,
    assistant: Option<&str>,
) -> ExitCode {
    if !dry_run && !yes {
        return emit_error(
            ctx,
            "USAGE_CONFIRM_REQUIRED",
            "push-all execute requires --yes (or use --dry-run)",
            None,
            CliExitCode::UsageError,
        );
    }

    match Workspace::detect(std::path::Path::new("")) {
        Ok(ws) => match run_workspace_validation(
            &ws,
            ValidateOptions {
                strict: false,
                offline: true,
                remote: false,
            },
        ) {
            Ok(report) => match build_push_plan(&ws, &report, assistant) {
                Ok(mut plan) => {
                    if dry_run {
                        enrich_push_plan_remote_drift(&ws, &mut plan).await;
                        return finish_dry_run(ctx, &plan);
                    }
                    let blockers = preflight_blockers(&plan, force);
                    if !blockers.is_empty() {
                        return emit_error(
                            ctx,
                            "PUSH_BLOCKED",
                            "Push blocked by validation blockers",
                            Some(serde_json::json!({ "blockers": blockers })),
                            CliExitCode::ValidationFailed,
                        );
                    }
                    match execute_push_plan(
                        &ws,
                        &plan,
                        PushExecuteOptions {
                            force,
                            stop_on_error,
                        },
                    )
                    .await
                    {
                        Ok(run_report) => finish_execute(ctx, &run_report),
                        Err(PushExecutorError::PreflightBlocked(blockers)) => emit_error(
                            ctx,
                            "PUSH_BLOCKED",
                            "Push blocked by validation blockers",
                            Some(serde_json::json!({ "blockers": blockers })),
                            CliExitCode::ValidationFailed,
                        ),
                    }
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
            Err(ValidateError::Graph(e)) => emit_error(
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

fn finish_dry_run(ctx: &CliContext, plan: &PushPlan) -> ExitCode {
    match ctx.mode {
        OutputMode::Human => print_human_plan(plan),
        OutputMode::Json => emit_success(ctx, plan),
    }
    CliExitCode::Success.into()
}

fn finish_execute(ctx: &CliContext, report: &crate::push_executor::PushRunReport) -> ExitCode {
    match ctx.mode {
        OutputMode::Human => {
            for step in &report.steps {
                let status = format!("{:?}", step.status).to_lowercase();
                println!(
                    "  {}. [{:?}] {} — {}",
                    step.order, step.resource_type, step.name, status
                );
            }
            if let Some(failed_at) = report.failed_at {
                eprintln!("Push stopped at step {}", failed_at);
            } else {
                println!("Push complete: {} step(s)", report.completed_count);
            }
        }
        OutputMode::Json => emit_success(ctx, report),
    }
    if report.failed_at.is_some() {
        CliExitCode::RuntimeError.into()
    } else {
        CliExitCode::Success.into()
    }
}

fn print_human_plan(plan: &PushPlan) {
    println!("Push plan (dry-run)");
    println!();
    if plan.steps.is_empty() {
        println!("No push steps.");
    } else {
        println!("Steps ({}):", plan.steps.len());
        for step in &plan.steps {
            print_step(step);
        }
    }
    if !plan.skipped.is_empty() {
        println!();
        println!("Skipped ({}):", plan.skipped.len());
        for step in &plan.skipped {
            print_step(step);
        }
    }
}

fn print_step(step: &PushStep) {
    let action = match step.action {
        PushAction::Create => "create",
        PushAction::Update => "update",
        PushAction::Skip => "skip",
    };
    let blockers = if step.blockers.is_empty() {
        String::new()
    } else {
        format!(" blockers=[{}]", step.blockers.join(", "))
    };
    let drift = if step.remote_drift {
        format!(
            " remote_drift=[{}]",
            if step.drift_fields.is_empty() {
                "changed".to_string()
            } else {
                step.drift_fields.join(", ")
            }
        )
    } else {
        String::new()
    };
    println!(
        "  {}. [{:?}] {} ({}) — {}{}{}",
        step.order, step.resource_type, step.name, action, step.path, blockers, drift
    );
}
