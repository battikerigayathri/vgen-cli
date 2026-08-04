//! In-process MCP tool dispatch — returns JSON envelope strings.

use crate::commands::explain::ExplainListData;
use crate::commands::init::init_refused_message;
use crate::commands::init::InitData;
use crate::commands::kit::KitUpdateData;
use crate::commands::scaffold::ScaffoldData;
use crate::doctor;
use crate::errors_registry::{list_all, list_by_domain, lookup};
use crate::graph::{self, GraphError};
use crate::kit::{
    copy_recipe, copy_workspace_kit, ensure_live_dirs, init_unsafe_entries, kit_is_present,
    kit_version, render_seed_files, update_kit_version_in_manifest, InitOptions,
    DEFAULT_DESCRIPTION, INIT_SAFE_TOP_LEVEL,
};
use crate::manifest::MANIFEST_FILE;
use crate::output::{build_error_json, build_success_json, CliContext};
use crate::push_executor::{execute_push_plan, preflight_blockers, PushExecuteOptions};
use crate::push_plan::build_push_plan;
use crate::scaffold::known_recipes;
use crate::validate::{run_workspace_validation, ValidateError, ValidateOptions};
use crate::workflow_validate;
use crate::workspace::{Workspace, WorkspaceError};
use serde_json::json;
use std::path::Path;

pub async fn tool_doctor(ctx: &CliContext, offline: bool) -> (String, bool) {
    let report = doctor::run_checks(Path::new(""), offline).await;
    let is_error = report.has_failures();
    (build_success_json(ctx, &report), is_error)
}

pub fn tool_workspace_info(ctx: &CliContext) -> (String, bool) {
    match Workspace::detect(Path::new("")) {
        Ok(ws) => match ws.info() {
            Ok(info) => (build_success_json(ctx, &info), false),
            Err(e) => (
                build_error_json(ctx, "WORKSPACE_NOT_FOUND", e.to_string(), None),
                true,
            ),
        },
        Err(e) => (
            build_error_json(ctx, "WORKSPACE_NOT_FOUND", e.to_string(), None),
            true,
        ),
    }
}

pub fn tool_graph(ctx: &CliContext, assistant: Option<&str>) -> (String, bool) {
    match Workspace::detect(Path::new("")) {
        Ok(ws) => match graph::build_graph_filtered(&ws, assistant) {
            Ok(g) => (build_success_json(ctx, &g), false),
            Err(GraphError::Io(e)) => (
                build_error_json(ctx, "GRAPH_BUILD_FAILED", e.to_string(), None),
                true,
            ),
        },
        Err(e) => (
            build_error_json(ctx, "WORKSPACE_NOT_FOUND", e.to_string(), None),
            true,
        ),
    }
}

pub fn tool_validate(ctx: &CliContext, strict: bool) -> (String, bool) {
    match Workspace::detect(Path::new("")) {
        Ok(ws) => match run_workspace_validation(
            &ws,
            ValidateOptions {
                strict,
                offline: true,
                remote: false,
            },
        ) {
            Ok(report) => {
                let is_error = report.summary.error_count > 0;
                (build_success_json(ctx, &report), is_error)
            }
            Err(ValidateError::Graph(e)) => (
                build_error_json(ctx, "GRAPH_BUILD_FAILED", e.to_string(), None),
                true,
            ),
        },
        Err(e) => (
            build_error_json(ctx, "WORKSPACE_NOT_FOUND", e.to_string(), None),
            true,
        ),
    }
}

pub fn tool_workflow_validate(ctx: &CliContext, name: &str) -> (String, bool) {
    match Workspace::detect(Path::new("")) {
        Ok(ws) => {
            let dir = ws.workflows_dir.join(name);
            match workflow_validate::validate_workflow_dir(&dir) {
                Ok(data) => (build_success_json(ctx, &data), false),
                Err(err) => (
                    build_error_json(ctx, err.code, err.message, err.details),
                    true,
                ),
            }
        }
        Err(e) => (
            build_error_json(ctx, "WORKSPACE_NOT_FOUND", e.to_string(), None),
            true,
        ),
    }
}

pub fn tool_push_all_dry_run(ctx: &CliContext, assistant: Option<&str>) -> (String, bool) {
    match build_plan(ctx, assistant) {
        Ok(plan) => (build_success_json(ctx, &plan), false),
        Err((code, msg, details)) => (build_error_json(ctx, code, msg, details), true),
    }
}

pub async fn tool_push_all_execute(
    ctx: &CliContext,
    force: bool,
    assistant: Option<&str>,
) -> (String, bool) {
    let plan = match build_plan(ctx, assistant) {
        Ok(p) => p,
        Err((code, msg, details)) => {
            return (build_error_json(ctx, code, msg, details), true);
        }
    };
    let blockers = preflight_blockers(&plan, force);
    if !blockers.is_empty() {
        return (
            build_error_json(
                ctx,
                "PUSH_BLOCKED",
                "Push blocked by validation blockers",
                Some(json!({ "blockers": blockers })),
            ),
            true,
        );
    }
    let ws = Workspace::detect(Path::new("")).expect("workspace checked in build_plan");
    match execute_push_plan(
        &ws,
        &plan,
        PushExecuteOptions {
            force,
            stop_on_error: true,
        },
    )
    .await
    {
        Ok(run_report) => {
            let is_error = run_report.failed_at.is_some();
            (build_success_json(ctx, &run_report), is_error)
        }
        Err(e) => (
            build_error_json(ctx, "PUSH_BLOCKED", e.to_string(), None),
            true,
        ),
    }
}

fn build_plan(
    ctx: &CliContext,
    assistant: Option<&str>,
) -> Result<crate::push_plan::PushPlan, (&'static str, String, Option<serde_json::Value>)> {
    let ws = Workspace::detect(Path::new(""))
        .map_err(|e| ("WORKSPACE_NOT_FOUND", e.to_string(), None))?;
    let report = run_workspace_validation(
        &ws,
        ValidateOptions {
            strict: false,
            offline: true,
            remote: false,
        },
    )
    .map_err(|e| ("GRAPH_BUILD_FAILED", e.to_string(), None))?;
    build_push_plan(&ws, &report, assistant)
        .map_err(|e| ("GRAPH_BUILD_FAILED", e.to_string(), None))
}

pub fn tool_explain(ctx: &CliContext, code: &str) -> (String, bool) {
    match lookup(code) {
        Some(doc) => (build_success_json(ctx, &doc), false),
        None => (
            build_error_json(
                ctx,
                "UNKNOWN_ERROR_CODE",
                format!("Unknown error code: {}", code),
                Some(json!({ "requested": code })),
            ),
            true,
        ),
    }
}

pub fn tool_explain_list(ctx: &CliContext, domain: Option<&str>) -> (String, bool) {
    let codes = match domain {
        Some(d) => list_by_domain(d),
        None => list_all(),
    };
    (build_success_json(ctx, &ExplainListData { codes }), false)
}

pub fn tool_init(
    ctx: &CliContext,
    name: &str,
    description: Option<&str>,
    force: bool,
) -> (String, bool) {
    let root = match std::env::current_dir() {
        Ok(p) => p,
        Err(e) => {
            return (
                build_error_json(ctx, "INIT_FAILED", e.to_string(), None),
                true,
            )
        }
    };
    let offenders = init_unsafe_entries(&root);
    if !offenders.is_empty() && !force {
        return (
            build_error_json(
                ctx,
                "INIT_REFUSED",
                init_refused_message(&offenders),
                Some(json!({
                    "root": root.display().to_string(),
                    "offending_paths": offenders,
                    "allowlist": INIT_SAFE_TOP_LEVEL,
                })),
            ),
            true,
        );
    }
    let description = description
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_DESCRIPTION);
    let options = InitOptions {
        name: name.to_string(),
        force,
        no_examples: false,
        dry_run: false,
    };
    let mut files_written = match copy_workspace_kit(&root, &options) {
        Ok(w) => w,
        Err(e) => {
            return (
                build_error_json(ctx, "INIT_FAILED", e.to_string(), None),
                true,
            )
        }
    };
    let created_dirs = match ensure_live_dirs(&root) {
        Ok(d) => d,
        Err(e) => {
            return (
                build_error_json(ctx, "INIT_FAILED", e.to_string(), None),
                true,
            )
        }
    };
    let seed_files = match render_seed_files(name, description) {
        Ok(f) => f,
        Err(e) => {
            return (
                build_error_json(ctx, "INIT_FAILED", e.to_string(), None),
                true,
            )
        }
    };
    for (rel, content) in seed_files {
        let path = root.join(&rel);
        if path.exists() && !force {
            continue;
        }
        if let Some(parent) = path.parent() {
            if std::fs::create_dir_all(parent).is_err() {
                return (
                    build_error_json(ctx, "INIT_FAILED", "Failed to write seed file", None),
                    true,
                );
            }
        }
        if std::fs::write(&path, content).is_ok() {
            files_written.push(rel);
        }
    }
    let data = InitData {
        root: root.display().to_string(),
        created_dirs,
        manifest_path: root.join(MANIFEST_FILE).display().to_string(),
        kit_version: crate::kit::kit_version().to_string(),
        files_written,
    };
    (build_success_json(ctx, &data), false)
}

pub fn tool_scaffold(ctx: &CliContext, recipe: &str, name: &str, force: bool) -> (String, bool) {
    let root = match std::env::current_dir() {
        Ok(p) => p,
        Err(e) => {
            return (
                build_error_json(ctx, "SCAFFOLD_FAILED", e.to_string(), None),
                true,
            )
        }
    };
    if !known_recipes().contains(&recipe) {
        return (
            build_error_json(
                ctx,
                "SCAFFOLD_FAILED",
                format!("Unknown recipe '{recipe}'"),
                None,
            ),
            true,
        );
    }
    if !kit_is_present(&root) {
        return (
            build_error_json(
                ctx,
                "KIT_NOT_PRESENT",
                "Authoring kit not found. Run init_workspace first",
                Some(json!({ "root": root.display().to_string() })),
            ),
            true,
        );
    }
    let files_written = match copy_recipe(&root, recipe, name, force) {
        Ok(w) => w,
        Err(e) => {
            let code = if e.to_string().contains("Refusing to overwrite") {
                "SCAFFOLD_REFUSED"
            } else {
                "SCAFFOLD_FAILED"
            };
            return (build_error_json(ctx, code, e.to_string(), None), true);
        }
    };
    let data = ScaffoldData {
        root: root.display().to_string(),
        recipe: recipe.to_string(),
        files_written,
    };
    (build_success_json(ctx, &data), false)
}

pub fn tool_kit_update(ctx: &CliContext, examples: bool, dry_run: bool) -> (String, bool) {
    let root = match std::env::current_dir() {
        Ok(p) => p,
        Err(e) => {
            return (
                build_error_json(ctx, "KIT_UPDATE_FAILED", e.to_string(), None),
                true,
            )
        }
    };

    if !root.join(MANIFEST_FILE).is_file() {
        return (
            build_error_json(
                ctx,
                "NOT_A_WORKSPACE",
                format!(
                    "{} not found in {}. Run init_workspace first to bootstrap a workspace.",
                    MANIFEST_FILE,
                    root.display()
                ),
                Some(json!({ "root": root.display().to_string() })),
            ),
            true,
        );
    }

    let options = InitOptions {
        name: root
            .file_name()
            .and_then(|s| s.to_str().map(String::from))
            .unwrap_or_else(|| "my-use-case".to_string()),
        force: true,
        no_examples: !examples,
        dry_run,
    };

    let files_written = match copy_workspace_kit(&root, &options) {
        Ok(w) => w,
        Err(e) => {
            return (
                build_error_json(ctx, "KIT_UPDATE_FAILED", e.to_string(), None),
                true,
            )
        }
    };

    let manifest_updated = if dry_run {
        false
    } else {
        match update_kit_version_in_manifest(&root, kit_version()) {
            Ok(updated) => updated,
            Err(e) => return (build_error_json(ctx, "KIT_UPDATE_FAILED", e, None), true),
        }
    };

    let data = KitUpdateData {
        root: root.display().to_string(),
        files_written,
        kit_version: kit_version().to_string(),
        dry_run,
        manifest_updated,
    };
    (build_success_json(ctx, &data), false)
}
