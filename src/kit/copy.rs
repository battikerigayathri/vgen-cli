use super::load::{
    kit_version, recipes_dir, seed_dir, templates_root, workspace_kit_dir, KitError,
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Default `description` written to `vgen.yaml` when none is provided.
pub const DEFAULT_DESCRIPTION: &str = "ResMate use case workspace";

/// Top-level entries that do not block `vgen init` (bootstrap is init-safe).
///
/// Case-sensitive exact names. Anything else (including empty live dirs like
/// `tools/`) makes the workspace not init-safe unless `--force` is passed.
pub const INIT_SAFE_TOP_LEVEL: &[&str] = &[".git", ".gitignore", "README.md", ".DS_Store"];

#[derive(Debug, Clone)]
pub struct InitOptions {
    pub name: String,
    pub force: bool,
    pub no_examples: bool,
    pub dry_run: bool,
}

/// Returns the top-level entries that make `root` not init-safe.
///
/// An empty vec means init may proceed without `--force`. A truly empty
/// directory is init-safe. Any top-level name outside [`INIT_SAFE_TOP_LEVEL`]
/// is collected as an offender.
pub fn init_unsafe_entries(root: &Path) -> Vec<String> {
    let mut offenders = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return offenders;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy().to_string();
        if !INIT_SAFE_TOP_LEVEL.contains(&name.as_str()) {
            offenders.push(name);
        }
    }
    offenders.sort();
    offenders
}

/// Convenience predicate: true when `root` is empty or only allowlisted entries.
pub fn workspace_is_init_safe(root: &Path) -> bool {
    init_unsafe_entries(root).is_empty()
}

/// Returns true when the workspace already has live (pushable) artifacts.
pub fn workspace_has_live_artifacts(root: &Path) -> bool {
    for dir in ["tools", "agents", "assistants", "hitl", "workflows"] {
        let path = root.join(dir);
        if !path.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if ft.is_dir() {
                return true;
            }
            if ft.is_file() && (name.ends_with(".yaml") || name.ends_with(".yml")) {
                return true;
            }
        }
    }
    false
}

pub fn kit_is_present(root: &Path) -> bool {
    root.join("AGENTS.md").is_file()
}

fn is_live_artifact_path(rel: &str) -> bool {
    if let Some(rest) = rel.strip_prefix("tools/") {
        return rest.contains('/');
    }
    if let Some(rest) = rel.strip_prefix("agents/") {
        return rest.ends_with(".yaml") || rest.ends_with(".yml");
    }
    if let Some(rest) = rel.strip_prefix("assistants/") {
        return rest.ends_with(".yaml") || rest.ends_with(".yml");
    }
    if rel.starts_with("hitl/") && rel != "hitl" {
        let rest = &rel["hitl/".len()..];
        return rest.contains('/') || rest.ends_with(".json");
    }
    if rel.starts_with("workflows/") && rel != "workflows" {
        let rest = &rel["workflows/".len()..];
        return rest.contains('/');
    }
    false
}

fn should_skip_kit_path(rel: &str, no_examples: bool) -> bool {
    if no_examples && (rel == "examples" || rel.starts_with("examples/")) {
        return true;
    }
    false
}

fn copy_tree(
    src: &Path,
    dst_root: &Path,
    rel_prefix: &str,
    force: bool,
    no_examples: bool,
    dry_run: bool,
    written: &mut Vec<String>,
) -> Result<(), KitError> {
    if !src.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(src).map_err(|e| KitError::Io(e.to_string()))? {
        let entry = entry.map_err(|e| KitError::Io(e.to_string()))?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let rel = if rel_prefix.is_empty() {
            file_name.to_string()
        } else {
            format!("{rel_prefix}/{file_name}")
        };

        if should_skip_kit_path(&rel, no_examples) {
            continue;
        }

        let ft = entry.file_type().map_err(|e| KitError::Io(e.to_string()))?;
        if ft.is_dir() {
            copy_tree(
                &entry.path(),
                dst_root,
                &rel,
                force,
                no_examples,
                dry_run,
                written,
            )?;
        } else if ft.is_file() {
            if is_live_artifact_path(&rel) {
                continue;
            }
            let target = dst_root.join(&rel);
            if target.exists() && !force {
                continue;
            }
            if !dry_run {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(|e| KitError::Io(e.to_string()))?;
                }
                fs::copy(entry.path(), &target).map_err(|e| KitError::Io(e.to_string()))?;
            }
            written.push(rel);
        }
    }
    Ok(())
}

pub fn copy_workspace_kit(
    workspace_root: &Path,
    options: &InitOptions,
) -> Result<Vec<String>, KitError> {
    let templates = templates_root()?;
    let kit_src = workspace_kit_dir(&templates);
    if !kit_src.is_dir() {
        return Err(KitError::TemplatesNotFound(format!(
            "workspace kit missing at {}",
            kit_src.display()
        )));
    }

    let mut written = Vec::new();
    copy_tree(
        &kit_src,
        workspace_root,
        "",
        options.force,
        options.no_examples,
        options.dry_run,
        &mut written,
    )?;
    Ok(written)
}

/// Update `kit_version` in `vgen.yaml` in place, preserving comments and formatting.
///
/// Line-based rewrite (no YAML parse/re-serialize) so unrelated formatting,
/// comments, and key order in the manifest are untouched. Returns `Ok(false)`
/// when there is no manifest or no `kit_version` key to update.
pub fn update_kit_version_in_manifest(root: &Path, new_version: &str) -> Result<bool, String> {
    let path = root.join("vgen.yaml");
    if !path.is_file() {
        return Ok(false);
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut updated = false;
    let mut new_lines = Vec::new();
    for line in content.lines() {
        if line.trim_start().starts_with("kit_version:") {
            let indent = line.len() - line.trim_start().len();
            let indent_str = &line[..indent];
            new_lines.push(format!("{}kit_version: \"{}\"", indent_str, new_version));
            updated = true;
        } else {
            new_lines.push(line.to_string());
        }
    }
    if updated {
        let new_content = new_lines.join("\n") + "\n";
        fs::write(&path, new_content).map_err(|e| e.to_string())?;
    }
    Ok(updated)
}

fn iso8601_now() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn render_template(content: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = content.to_string();
    for (key, value) in vars {
        out = out.replace(&format!("{{{{{key}}}}}"), value);
    }
    out
}

pub fn render_seed_files(name: &str, description: &str) -> Result<Vec<(String, String)>, KitError> {
    let templates = templates_root()?;
    let seed = seed_dir(&templates);
    let mut vars = HashMap::new();
    vars.insert("name", name.to_string());
    vars.insert("description", description.to_string());
    vars.insert("kit_version", kit_version().to_string());
    vars.insert("initialized_at", iso8601_now());

    let manifest_tmpl = fs::read_to_string(seed.join("vgen.yaml.tmpl"))
        .map_err(|e| KitError::Io(e.to_string()))?;
    let gitignore =
        fs::read_to_string(seed.join("gitignore")).map_err(|e| KitError::Io(e.to_string()))?;

    Ok(vec![
        (
            "vgen.yaml".to_string(),
            render_template(&manifest_tmpl, &vars),
        ),
        (".gitignore".to_string(), gitignore),
    ])
}

fn recipe_template_vars(name: &str) -> HashMap<&'static str, String> {
    let mut vars = HashMap::new();
    vars.insert("name", name.to_string());
    vars.insert("kit_version", kit_version().to_string());
    vars.insert("initialized_at", iso8601_now());
    vars
}

fn copy_recipe_tree(
    src: &Path,
    dst_root: &Path,
    rel_prefix: &str,
    name: &str,
    force: bool,
    written: &mut Vec<String>,
) -> Result<(), KitError> {
    if !src.is_dir() {
        return Ok(());
    }
    let vars = recipe_template_vars(name);
    for entry in fs::read_dir(src).map_err(|e| KitError::Io(e.to_string()))? {
        let entry = entry.map_err(|e| KitError::Io(e.to_string()))?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let rel = if rel_prefix.is_empty() {
            file_name.to_string()
        } else {
            format!("{rel_prefix}/{file_name}")
        };

        let ft = entry.file_type().map_err(|e| KitError::Io(e.to_string()))?;
        if ft.is_dir() {
            copy_recipe_tree(&entry.path(), dst_root, &rel, name, force, written)?;
        } else if ft.is_file() {
            let entry_path = entry.path();
            let path_str = entry_path.to_string_lossy();
            if path_str.ends_with(".tmpl") {
                let raw =
                    fs::read_to_string(&entry_path).map_err(|e| KitError::Io(e.to_string()))?;
                let rendered = render_template(&raw, &vars);
                let out_rel = rel.strip_suffix(".tmpl").unwrap_or(&rel).to_string();
                let out_target = dst_root.join(&out_rel);
                let always_overwrite = out_rel == "vgen.yaml";
                if out_target.exists() && !force && !always_overwrite {
                    return Err(KitError::Io(format!(
                        "Refusing to overwrite existing file: {} (use --force)",
                        out_target.display()
                    )));
                }
                if let Some(parent) = out_target.parent() {
                    fs::create_dir_all(parent).map_err(|e| KitError::Io(e.to_string()))?;
                }
                fs::write(&out_target, rendered).map_err(|e| KitError::Io(e.to_string()))?;
                written.push(out_rel);
                continue;
            }

            let target = dst_root.join(&rel);
            if target.exists() && !force {
                return Err(KitError::Io(format!(
                    "Refusing to overwrite existing file: {} (use --force)",
                    target.display()
                )));
            }
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| KitError::Io(e.to_string()))?;
            }
            let content = fs::read(&entry_path).map_err(|e| KitError::Io(e.to_string()))?;
            fs::write(&target, content).map_err(|e| KitError::Io(e.to_string()))?;
            written.push(rel);
        }
    }
    Ok(())
}

pub fn copy_recipe(
    workspace_root: &Path,
    recipe: &str,
    name: &str,
    force: bool,
) -> Result<Vec<String>, KitError> {
    let templates = templates_root()?;
    let recipe_src = recipes_dir(&templates).join(recipe);
    if !recipe_src.is_dir() {
        return Err(KitError::Io(format!(
            "Unknown recipe '{}'. Available: minimal, form-wizard, oracle-pr",
            recipe
        )));
    }
    let mut written = Vec::new();
    copy_recipe_tree(&recipe_src, workspace_root, "", name, force, &mut written)?;
    Ok(written)
}

pub fn ensure_live_dirs(workspace_root: &Path) -> Result<Vec<String>, KitError> {
    let dirs = ["tools", "agents", "assistants", "hitl", "workflows"];
    let mut created = Vec::new();
    for dir in dirs {
        let path = workspace_root.join(dir);
        if !path.exists() {
            fs::create_dir_all(&path).map_err(|e| KitError::Io(e.to_string()))?;
            created.push(dir.to_string());
        }
    }
    Ok(created)
}
