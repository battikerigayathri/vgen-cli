use indexmap::IndexMap;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Canonical key order for meta.yaml: id first, then name, slug, preMessage, postMessage.
const HITL_META_KEY_ORDER: &[&str] = &["id", "name", "slug", "preMessage", "postMessage"];

/// True when a directory contains HITL `config.json` and `meta.yaml`.
pub fn is_hitl_dir(path: &Path) -> bool {
    path.join("config.json").is_file()
        && (path.join("meta.yaml").is_file() || path.join("meta.yml").is_file())
}

/// List HITL directories with `config.json` and `meta.yaml`.
pub fn list_hitl_dirs(base: &Path) -> Vec<PathBuf> {
    if !base.is_dir() {
        return Vec::new();
    }
    std::fs::read_dir(base)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && is_hitl_dir(path))
        .collect()
}

/// Resolve HITL directory: env RESMATE_HITL_DIR or default "hitl" under cwd.
pub fn default_hitl_dir() -> PathBuf {
    std::env::var("RESMATE_HITL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("hitl"))
}

/// Load HITL from a folder: config.json (adaptive card) and meta.yaml. Returns (body for create/update, path to meta.yaml).
/// Body has "config" (stringified JSON), "name", "slug", "preMessage", "postMessage"; "id" from meta if present.
pub fn load_hitl_from_dir(
    hitl_dir: &Path,
) -> Result<(Value, PathBuf), Box<dyn std::error::Error + Send + Sync>> {
    if !hitl_dir.exists() || !hitl_dir.is_dir() {
        return Err(format!("HITL folder not found: {}", hitl_dir.display()).into());
    }

    let config_path = hitl_dir.join("config.json");
    if !config_path.exists() {
        return Err(format!("Missing config.json in {}", hitl_dir.display()).into());
    }
    let config_str = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config.json: {}", e))?;
    let config_value: Value =
        serde_json::from_str(&config_str).map_err(|e| format!("Invalid config.json: {}", e))?;
    let config_string = serde_json::to_string(&config_value).map_err(|e| e.to_string())?;

    let meta_path = hitl_dir.join("meta.yaml");
    if !meta_path.exists() {
        return Err(format!("Missing meta.yaml in {}", hitl_dir.display()).into());
    }
    let meta_content = std::fs::read_to_string(&meta_path)
        .map_err(|e| format!("Failed to read meta.yaml: {}", e))?;
    let meta_value: Value =
        serde_yaml::from_str(&meta_content).map_err(|e| format!("Invalid meta.yaml: {}", e))?;

    let meta_obj = meta_value
        .as_object()
        .ok_or("meta.yaml must be an object")?;
    let name = meta_obj
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or("meta.yaml must contain name")?
        .to_string();
    let slug = meta_obj
        .get("slug")
        .and_then(|v| v.as_str())
        .ok_or("meta.yaml must contain slug")?
        .to_string();
    let pre_message = meta_obj
        .get("preMessage")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_default();
    let post_message = meta_obj
        .get("postMessage")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_default();

    let mut body = serde_json::json!({
        "config": config_string,
        "name": name,
        "slug": slug,
        "preMessage": pre_message,
        "postMessage": post_message,
    });

    if let Some(id) = meta_obj
        .get("id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        body["id"] = Value::String(id.to_string());
    }

    Ok((body, meta_path))
}

/// Write HITL from get-record response: config.json (parsed from config string) and meta.yaml (normalized).
pub fn write_hitl_from_record(
    hitl_dir: &Path,
    data: &Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let data_obj = data
        .as_object()
        .ok_or("get-record HITL data is not an object")?;

    let config_str = data_obj
        .get("config")
        .and_then(|v| v.as_str())
        .ok_or("HITL record has no config")?;
    let config_value: Value = serde_json::from_str(config_str)
        .map_err(|e| format!("HITL config is not valid JSON: {}", e))?;
    let config_out = serde_json::to_string_pretty(&config_value).map_err(|e| e.to_string())?;
    let config_path = hitl_dir.join("config.json");
    std::fs::write(&config_path, config_out)
        .map_err(|e| format!("Failed to write config.json: {}", e))?;

    let mut meta_value = data.clone();
    if let Some(obj) = meta_value.as_object_mut() {
        obj.remove("config");
    }
    let meta_value = super::normalize::normalize_mongo_oids(meta_value);
    let ordered = reorder_hitl_meta_keys(meta_value)?;
    let meta_out = serde_yaml::to_string(&ordered).map_err(|e| e.to_string())?;
    let meta_path = hitl_dir.join("meta.yaml");
    std::fs::write(&meta_path, meta_out)
        .map_err(|e| format!("Failed to write meta.yaml: {}", e))?;

    Ok(())
}

fn reorder_hitl_meta_keys(
    mut value: Value,
) -> Result<IndexMap<String, Value>, Box<dyn std::error::Error + Send + Sync>> {
    let obj = value
        .as_object_mut()
        .ok_or("HITL record is not an object")?;
    let mut ordered: IndexMap<String, Value> = IndexMap::new();
    for &key in HITL_META_KEY_ORDER {
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

/// Read meta.yaml and return (id, meta_path). Errors if id is missing or empty (required for pull).
pub fn get_hitl_id_and_meta_path(
    hitl_dir: &Path,
) -> Result<(String, PathBuf), Box<dyn std::error::Error + Send + Sync>> {
    if !hitl_dir.exists() || !hitl_dir.is_dir() {
        return Err(format!("HITL folder not found: {}", hitl_dir.display()).into());
    }
    let meta_path = hitl_dir.join("meta.yaml");
    if !meta_path.exists() {
        return Err(format!("Missing meta.yaml in {}", hitl_dir.display()).into());
    }
    let content = std::fs::read_to_string(&meta_path)
        .map_err(|e| format!("Failed to read meta.yaml: {}", e))?;
    let value: Value =
        serde_yaml::from_str(&content).map_err(|e| format!("Invalid meta.yaml: {}", e))?;
    let id = value
        .get("id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            format!(
                "No id in {}/meta.yaml; push first or add id.",
                hitl_dir.display()
            )
        })?
        .to_string();
    Ok((id, meta_path))
}

/// Write id into meta.yaml (e.g. after create). Preserves other keys and uses canonical order.
pub fn write_hitl_id_to_meta(
    meta_path: &Path,
    id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let content = std::fs::read_to_string(meta_path)
        .map_err(|e| format!("Failed to read {}: {}", meta_path.display(), e))?;
    let mut value: Value =
        serde_yaml::from_str(&content).map_err(|e| format!("Invalid YAML: {}", e))?;

    if let Some(obj) = value.as_object_mut() {
        obj.insert("id".to_string(), Value::String(id.to_string()));
    } else {
        return Err("meta.yaml root is not an object".into());
    }

    let ordered = reorder_hitl_meta_keys(value)?;
    let out = serde_yaml::to_string(&ordered).map_err(|e| e.to_string())?;
    std::fs::write(meta_path, out)
        .map_err(|e| format!("Failed to write {}: {}", meta_path.display(), e))?;
    Ok(())
}
