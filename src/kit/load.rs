use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum KitError {
    TemplatesNotFound(String),
    Io(String),
}

impl std::fmt::Display for KitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KitError::TemplatesNotFound(msg) => write!(f, "{msg}"),
            KitError::Io(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for KitError {}

/// Kit version recorded in `vgen.yaml` after init.
pub fn kit_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn is_valid_templates_root(path: &Path) -> bool {
    path.join("workspace").is_dir()
}

fn normalize_templates_root(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

/// Candidate install-time template roots (checked in order after `VGEN_TEMPLATES_DIR`).
fn installed_template_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join("../share/vgen/templates"));
        }
    }

    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".local/share/vgen/templates"));
    }

    #[cfg(unix)]
    {
        candidates.push(PathBuf::from("/usr/local/share/vgen/templates"));
        candidates.push(PathBuf::from("/opt/homebrew/share/vgen/templates"));
    }

    #[cfg(windows)]
    {
        if let Ok(local) = env::var("LOCALAPPDATA") {
            let local = PathBuf::from(local);
            candidates.push(local.join("ResMate/templates"));
            candidates.push(local.join("Programs/ResMate/share/templates"));
        }
        if let Ok(pf) = env::var("ProgramFiles") {
            candidates.push(PathBuf::from(pf).join("ResMate/share/templates"));
        }
    }

    candidates
}

/// Resolve the `templates/` directory for the authoring kit.
///
/// Resolution order:
/// 1. `VGEN_TEMPLATES_DIR` when set
/// 2. Install-relative and OS default paths (see `installed_template_candidates`)
/// 3. `CARGO_MANIFEST_DIR/templates` when running from a source checkout
pub fn templates_root() -> Result<PathBuf, KitError> {
    let mut attempted = Vec::new();

    if let Ok(dir) = env::var("VGEN_TEMPLATES_DIR") {
        let path = PathBuf::from(&dir);
        attempted.push(format!("VGEN_TEMPLATES_DIR={}", path.display()));
        if is_valid_templates_root(&path) {
            return Ok(normalize_templates_root(path));
        }
        return Err(KitError::TemplatesNotFound(format!(
            "VGEN_TEMPLATES_DIR={} does not contain workspace/",
            path.display()
        )));
    }

    for candidate in installed_template_candidates() {
        attempted.push(candidate.display().to_string());
        if is_valid_templates_root(&candidate) {
            return Ok(normalize_templates_root(candidate));
        }
    }

    let manifest_templates = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("templates");
    attempted.push(format!(
        "CARGO_MANIFEST_DIR/templates ({})",
        manifest_templates.display()
    ));
    if is_valid_templates_root(&manifest_templates) {
        return Ok(manifest_templates);
    }

    Err(KitError::TemplatesNotFound(format!(
        "Authoring kit templates not found. Attempted:\n  {}\n\
         Re-run install.sh / install.ps1 or set VGEN_TEMPLATES_DIR.",
        attempted.join("\n  ")
    )))
}

pub fn workspace_kit_dir(root: &Path) -> PathBuf {
    root.join("workspace")
}

pub fn recipes_dir(root: &Path) -> PathBuf {
    root.join("recipes")
}

pub fn seed_dir(root: &Path) -> PathBuf {
    root.join("seed")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_root_finds_manifest_dir_in_dev() {
        let root = templates_root().expect("dev checkout should include templates/");
        assert!(root.join("workspace").is_dir());
        assert!(root.join("recipes").is_dir());
        assert!(root.join("seed/vgen.yaml.tmpl").is_file());
    }

    #[test]
    fn templates_root_respects_env_override() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("templates");
        unsafe {
            env::set_var("VGEN_TEMPLATES_DIR", &manifest);
        }
        let root = templates_root().expect("VGEN_TEMPLATES_DIR override should work");
        assert_eq!(root, normalize_templates_root(manifest.clone()));
        unsafe {
            env::remove_var("VGEN_TEMPLATES_DIR");
        }
    }

    #[test]
    fn templates_root_env_must_contain_workspace() {
        let tmp = env::temp_dir().join(format!("vgen-no-workspace-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        unsafe {
            env::set_var("VGEN_TEMPLATES_DIR", &tmp);
        }
        let err = templates_root().unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("workspace/"), "{msg}");
        unsafe {
            env::remove_var("VGEN_TEMPLATES_DIR");
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
