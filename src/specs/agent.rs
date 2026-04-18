use indexmap::IndexMap;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Canonical key order for agent YAML: id first, then name, slug, description, etc.
const AGENT_YAML_KEY_ORDER: &[&str] = &[
    "id",
    "name",
    "slug",
    "description",
    "systemInstructions",
    "skills",
    "is_public",
    "roles",
    "admins",
    "version",
];

/// Resolve agents directory: env vgen_AGENTS_DIR or default "agents" under cwd.
pub fn default_agents_dir() -> PathBuf {
    std::env::var("vgen_AGENTS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("agents"))
}

/// Load agent spec from a YAML file. Resolve path as agents_dir/name.yaml or agents_dir/name.yml.
/// Returns (body for create/update, path to the yaml file for id write-back).
pub fn load_agent(
    agents_dir: &Path,
    name: &str,
) -> Result<(Value, PathBuf), Box<dyn std::error::Error + Send + Sync>> {
    let yaml_path = resolve_yaml_path(agents_dir, name, "Agent")?;
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
        return Err(format!(
            "{} directory not found: {}",
            entity_label,
            dir.display()
        )
        .into());
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

fn reorder_agent_yaml_keys(mut value: Value) -> Result<IndexMap<String, Value>, Box<dyn std::error::Error + Send + Sync>> {
    let obj = value.as_object_mut().ok_or("YAML root is not an object")?;
    let mut ordered: IndexMap<String, Value> = IndexMap::new();
    for &key in AGENT_YAML_KEY_ORDER {
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

/// Write agent YAML from get-record response data. Normalizes all $oid (e.g. _id, skills, createdBy) to plain strings, reorders keys.
pub fn write_agent_yaml_from_record(
    yaml_path: &Path,
    data: &Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let value = super::normalize::normalize_mongo_oids(data.clone());
    let ordered = reorder_agent_yaml_keys(value)?;
    let out = serde_yaml::to_string(&ordered).map_err(|e| e.to_string())?;
    std::fs::write(yaml_path, out)
        .map_err(|e| format!("Failed to write {}: {}", yaml_path.display(), e))?;
    Ok(())
}

/// Update the agent's YAML file to set id. Writes keys in canonical order (id first).
pub fn write_agent_id_to_yaml(
    yaml_path: &Path,
    id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let content = std::fs::read_to_string(yaml_path)
        .map_err(|e| format!("Failed to read {}: {}", yaml_path.display(), e))?;
    let mut value: Value = serde_yaml::from_str(&content).map_err(|e| format!("Invalid YAML: {}", e))?;

    if let Some(obj) = value.as_object_mut() {
        obj.insert("id".to_string(), Value::String(id.to_string()));
    } else {
        return Err("YAML root is not an object".into());
    }

    let ordered = reorder_agent_yaml_keys(value)?;
    let out = serde_yaml::to_string(&ordered).map_err(|e| e.to_string())?;
    std::fs::write(yaml_path, out)
        .map_err(|e| format!("Failed to write {}: {}", yaml_path.display(), e))?;
    Ok(())
}
