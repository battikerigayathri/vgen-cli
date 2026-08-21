use crate::kit::{
    copy_workspace_kit, ensure_live_dirs, init_unsafe_entries, render_seed_files, InitOptions,
    KitError, DEFAULT_DESCRIPTION, INIT_SAFE_TOP_LEVEL,
};
use crate::manifest::MANIFEST_FILE;
use crate::output::{emit_error, emit_success, CliContext, CliExitCode, OutputMode};
use serde::Serialize;
use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

#[derive(Debug, Serialize)]
pub struct InitData {
    pub root: String,
    pub created_dirs: Vec<String>,
    pub manifest_path: String,
    pub kit_version: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_written: Vec<String>,
}

pub fn run_init(
    ctx: &CliContext,
    name: Option<&str>,
    description: Option<&str>,
    force: bool,
    no_examples: bool,
) -> ExitCode {
    let root = match env::current_dir() {
        Ok(p) => p,
        Err(e) => return emit_error(ctx, "INIT_FAILED", e, None, CliExitCode::RuntimeError),
    };

    let offenders = init_unsafe_entries(&root);
    if !offenders.is_empty() && !force {
        return emit_error(
            ctx,
            "INIT_REFUSED",
            init_refused_message(&offenders),
            Some(serde_json::json!({
                "root": root.display().to_string(),
                "offending_paths": offenders,
                "allowlist": INIT_SAFE_TOP_LEVEL,
            })),
            CliExitCode::UsageError,
        );
    }

    let project_name = name
        .map(String::from)
        .or_else(|| root.file_name().and_then(|s| s.to_str().map(String::from)))
        .unwrap_or_else(|| "my-use-case".to_string());

    let description = description
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_DESCRIPTION);

    let options = InitOptions {
        name: project_name.clone(),
        force,
        no_examples,
        dry_run: false,
    };

    let mut files_written = match copy_workspace_kit(&root, &options) {
        Ok(w) => w,
        Err(e) => return emit_error(ctx, "INIT_FAILED", e, None, CliExitCode::RuntimeError),
    };

    let created_dirs = match ensure_live_dirs(&root) {
        Ok(d) => d,
        Err(e) => return emit_error(ctx, "INIT_FAILED", e, None, CliExitCode::RuntimeError),
    };

    let seed_files = match render_seed_files(&project_name, description) {
        Ok(f) => f,
        Err(e) => return emit_error(ctx, "INIT_FAILED", e, None, CliExitCode::RuntimeError),
    };

    for (rel, content) in seed_files {
        if let Err(e) = write_seed_file(&root, &rel, &content, force) {
            return emit_error(ctx, "INIT_FAILED", e, None, CliExitCode::RuntimeError);
        }
        files_written.push(rel);
    }

    let data = InitData {
        root: root.display().to_string(),
        created_dirs,
        manifest_path: root.join(MANIFEST_FILE).display().to_string(),
        kit_version: crate::kit::kit_version().to_string(),
        files_written,
    };

    match ctx.mode {
        OutputMode::Human => {
            println!("Initialized workspace at {}", data.root);
            println!("Kit version: {}", data.kit_version);
            println!("Created directories: {}", data.created_dirs.join(", "));
            println!("Manifest: {}", data.manifest_path);
            println!("Files written: {}", data.files_written.len());
        }
        OutputMode::Json => emit_success(ctx, data),
    }
    CliExitCode::Success.into()
}

fn write_seed_file(root: &Path, rel: &str, content: &str, force: bool) -> Result<(), String> {
    let path = root.join(rel);
    if path.exists() && !force {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create {}: {}", parent.display(), e))?;
    }
    fs::write(&path, content).map_err(|e| format!("Failed to write {}: {}", path.display(), e))
}

pub fn kit_error_message(err: KitError) -> String {
    err.to_string()
}

/// Shared human-readable refuse message for the init emptiness gate.
///
/// Used by both the CLI and the MCP `init_workspace` tool so wording stays
/// identical. Lists offenders and explains the `--force` escape hatch plus the
/// `vgen kit update` path for refreshing existing use-case repos.
pub fn init_refused_message(offenders: &[String]) -> String {
    format!(
        "Directory is not init-safe: it must be empty or contain only allowlisted entries ({}). \
         Offending top-level entries: {}. \
         Pass --force to bootstrap-overwrite kit/seed files (never deletes live artifacts). \
         To refresh an existing use-case repo, prefer `vgen kit update`.",
        INIT_SAFE_TOP_LEVEL.join(", "),
        offenders.join(", ")
    )
}
