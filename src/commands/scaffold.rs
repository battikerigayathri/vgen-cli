use crate::kit::{
    copy_recipe, copy_workspace_kit, ensure_live_dirs, kit_is_present, render_seed_files,
    InitOptions, DEFAULT_DESCRIPTION,
};
use crate::output::{emit_error, emit_success, CliContext, CliExitCode, OutputMode};
use crate::scaffold::known_recipes;
use serde::Serialize;
use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

#[derive(Debug, Serialize)]
pub struct ScaffoldData {
    pub root: String,
    pub recipe: String,
    pub files_written: Vec<String>,
}

pub fn run_scaffold(
    ctx: &CliContext,
    recipe: &str,
    name: Option<&str>,
    force: bool,
    with_kit: bool,
) -> ExitCode {
    let root = match env::current_dir() {
        Ok(p) => p,
        Err(e) => return emit_error(ctx, "SCAFFOLD_FAILED", e, None, CliExitCode::RuntimeError),
    };

    if !known_recipes().contains(&recipe) {
        return emit_error(
            ctx,
            "SCAFFOLD_FAILED",
            format!(
                "Unknown recipe '{}'. Available: {}",
                recipe,
                known_recipes().join(", ")
            ),
            None,
            CliExitCode::UsageError,
        );
    }

    let project_name = name
        .map(String::from)
        .or_else(|| root.file_name().and_then(|s| s.to_str().map(String::from)))
        .unwrap_or_else(|| "my-use-case".to_string());

    if !kit_is_present(&root) {
        if !with_kit {
            return emit_error(
                ctx,
                "KIT_NOT_PRESENT",
                "Authoring kit not found. Run `resmate init` first or pass --with-kit",
                Some(serde_json::json!({ "root": root.display().to_string() })),
                CliExitCode::UsageError,
            );
        }
        let options = InitOptions {
            name: project_name.clone(),
            force,
            no_examples: true,
            dry_run: false,
        };
        if let Err(e) = copy_workspace_kit(&root, &options) {
            return emit_error(ctx, "SCAFFOLD_FAILED", e, None, CliExitCode::RuntimeError);
        }
        let _ = ensure_live_dirs(&root);
        if let Ok(seed) = render_seed_files(&project_name, DEFAULT_DESCRIPTION) {
            for (rel, content) in seed {
                let _ = write_file(&root, &rel, &content, force);
            }
        }
    }

    let files_written = match copy_recipe(&root, recipe, &project_name, force) {
        Ok(w) => w,
        Err(e) => {
            let msg = e.to_string();
            let code = if msg.contains("Refusing to overwrite") {
                "SCAFFOLD_REFUSED"
            } else {
                "SCAFFOLD_FAILED"
            };
            return emit_error(
                ctx,
                code,
                msg,
                None,
                if code == "SCAFFOLD_REFUSED" {
                    CliExitCode::UsageError
                } else {
                    CliExitCode::RuntimeError
                },
            );
        }
    };

    let data = ScaffoldData {
        root: root.display().to_string(),
        recipe: recipe.to_string(),
        files_written,
    };

    match ctx.mode {
        OutputMode::Human => {
            println!(
                "Scaffolded recipe '{}' at {} ({} files)",
                data.recipe,
                data.root,
                data.files_written.len()
            );
        }
        OutputMode::Json => emit_success(ctx, data),
    }
    CliExitCode::Success.into()
}

fn write_file(root: &Path, rel: &str, content: &str, force: bool) -> Result<(), String> {
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
