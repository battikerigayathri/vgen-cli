use crate::kit::{copy_workspace_kit, kit_version, update_kit_version_in_manifest, InitOptions};
use crate::manifest::MANIFEST_FILE;
use crate::output::{emit_error, emit_success, CliContext, CliExitCode, OutputMode};
use serde::Serialize;
use std::env;
use std::process::ExitCode;

#[derive(Debug, Serialize)]
pub struct KitUpdateData {
    pub root: String,
    pub files_written: Vec<String>,
    pub kit_version: String,
    pub dry_run: bool,
    pub manifest_updated: bool,
}

/// Refresh the authoring kit (skills, rules, docs, AGENTS.md) in an already
/// scaffolded workspace, without touching live artifacts or requiring the
/// `init` emptiness gate.
pub fn run_kit_update(ctx: &CliContext, examples: bool, dry_run: bool) -> ExitCode {
    let root = match env::current_dir() {
        Ok(p) => p,
        Err(e) => return emit_error(ctx, "KIT_UPDATE_FAILED", e, None, CliExitCode::RuntimeError),
    };

    if !root.join(MANIFEST_FILE).is_file() {
        return emit_error(
            ctx,
            "NOT_A_WORKSPACE",
            format!(
                "{} not found in {}. Run `resmate init` first to bootstrap a workspace.",
                MANIFEST_FILE,
                root.display()
            ),
            Some(serde_json::json!({ "root": root.display().to_string() })),
            CliExitCode::UsageError,
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
        Err(e) => return emit_error(ctx, "KIT_UPDATE_FAILED", e, None, CliExitCode::RuntimeError),
    };

    let manifest_updated = if dry_run {
        false
    } else {
        match update_kit_version_in_manifest(&root, kit_version()) {
            Ok(updated) => updated,
            Err(e) => {
                return emit_error(ctx, "KIT_UPDATE_FAILED", e, None, CliExitCode::RuntimeError)
            }
        }
    };

    let data = KitUpdateData {
        root: root.display().to_string(),
        files_written,
        kit_version: kit_version().to_string(),
        dry_run,
        manifest_updated,
    };

    match ctx.mode {
        OutputMode::Human => {
            if dry_run {
                println!(
                    "Dry run: would refresh {} kit file(s) at {}",
                    data.files_written.len(),
                    data.root
                );
            } else {
                println!("Refreshed kit at {}", data.root);
            }
            println!("Kit version: {}", data.kit_version);
            println!("Files: {}", data.files_written.len());
            if !data.files_written.is_empty() {
                for f in &data.files_written {
                    println!("  {}", f);
                }
            }
            if manifest_updated {
                println!("Updated kit_version in {}", MANIFEST_FILE);
            }
        }
        OutputMode::Json => emit_success(ctx, data),
    }
    CliExitCode::Success.into()
}
