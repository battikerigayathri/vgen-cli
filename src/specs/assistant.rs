use indexmap::IndexMap;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Canonical key order for assistant YAML: id first, then name, slug, description, etc.
const ASSISTANT_YAML_KEY_ORDER: &[&str] = &[
    "id",
    "name",
    "slug",
    "description",
    "status",
    "visibility",
    "agents",
    "workflows",
    "systemContext",
    "guardrailsContext",
    "guardrailsViolationFallback",
    "owner",
    "managedBy",
    "outcome_profile",
    "deliverable_fields",
    "tool_feeds",
];

/// Parse `workflow_definition_slug:` from assistant `systemContext` text.
pub fn parse_assistant_workflow_slug(system_context: &str) -> Option<String> {
    for line in system_context.lines() {
        let line = line.trim();
        if line.starts_with("workflow_definition_slug:") {
            let value = line["workflow_definition_slug:".len()..].trim();
            let value = value.trim_matches(|c| c == '\'' || c == '"');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// List assistant YAML files as `(file_stem, path)`.
pub fn list_assistant_files(base: &Path) -> Vec<(String, PathBuf)> {
    super::list_yaml_files(base)
}

/// Resolve assistants directory: env RESMATE_ASSISTANTS_DIR or default "assistants" under cwd.
pub fn default_assistants_dir() -> PathBuf {
    std::env::var("RESMATE_ASSISTANTS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("assistants"))
}

/// Load assistant spec from a YAML file. Resolve path as assistants_dir/name.yaml or assistants_dir/name.yml.
/// Returns (payload for create/update, path to the yaml file for id write-back).
pub fn load_assistant(
    assistants_dir: &Path,
    name: &str,
) -> Result<(Value, PathBuf), Box<dyn std::error::Error + Send + Sync>> {
    let yaml_path = resolve_yaml_path(assistants_dir, name, "Assistant")?;
    let content = std::fs::read_to_string(&yaml_path)
        .map_err(|e| format!("Failed to read {}: {}", yaml_path.display(), e))?;
    let value: Value = serde_yaml::from_str(&content)
        .map_err(|e| format!("Invalid YAML in {}: {}", yaml_path.display(), e))?;
    let value = super::normalize::normalize_mongo_oids(value);
    Ok((value, yaml_path))
}

fn resolve_yaml_path(
    dir: &Path,
    name: &str,
    entity_label: &str,
) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    if !dir.exists() || !dir.is_dir() {
        return Err(format!("{} directory not found: {}", entity_label, dir.display()).into());
    }
    let yaml = dir.join(format!("{}.yaml", name));
    let yml = dir.join(format!("{}.yml", name));
    if yaml.exists() {
        return Ok(yaml);
    }
    if yml.exists() {
        return Ok(yml);
    }
    Err(format!(
        "{} file not found: {}/{}.yaml or {}/{}.yml",
        entity_label,
        dir.display(),
        name,
        dir.display(),
        name
    )
    .into())
}

fn reorder_assistant_yaml_keys(
    mut value: Value,
) -> Result<IndexMap<String, Value>, Box<dyn std::error::Error + Send + Sync>> {
    let obj = value.as_object_mut().ok_or("YAML root is not an object")?;
    let mut ordered: IndexMap<String, Value> = IndexMap::new();
    for &key in ASSISTANT_YAML_KEY_ORDER {
        if let Some(v) = obj.remove(key) {
            ordered.insert(key.to_string(), v);
        }
    }
    let rest: Vec<String> = obj.keys().cloned().collect();
    for k in rest {
        if let Some(v) = obj.remove(&k) {
            ordered.insert(k, v);
        }
    }
    Ok(ordered)
}

/// Write assistant YAML from get-record response data. Normalizes all $oid (e.g. _id, agents) to plain strings, reorders keys.
pub fn write_assistant_yaml_from_record(
    yaml_path: &Path,
    data: &Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let value = super::normalize::normalize_mongo_oids(data.clone());
    let ordered = reorder_assistant_yaml_keys(value)?;
    let out = serde_yaml::to_string(&ordered).map_err(|e| e.to_string())?;
    std::fs::write(yaml_path, out)
        .map_err(|e| format!("Failed to write {}: {}", yaml_path.display(), e))?;
    Ok(())
}

/// Update the assistant's YAML file to set id. Writes keys in canonical order (id first).
pub fn write_assistant_id_to_yaml(
    yaml_path: &Path,
    id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let content = std::fs::read_to_string(yaml_path)
        .map_err(|e| format!("Failed to read {}: {}", yaml_path.display(), e))?;
    let mut value: Value =
        serde_yaml::from_str(&content).map_err(|e| format!("Invalid YAML: {}", e))?;

    if let Some(obj) = value.as_object_mut() {
        obj.insert("id".to_string(), Value::String(id.to_string()));
    } else {
        return Err("YAML root is not an object".into());
    }

    let ordered = reorder_assistant_yaml_keys(value)?;
    let out = serde_yaml::to_string(&ordered).map_err(|e| e.to_string())?;
    std::fs::write(yaml_path, out)
        .map_err(|e| format!("Failed to write {}: {}", yaml_path.display(), e))?;
    Ok(())
}
