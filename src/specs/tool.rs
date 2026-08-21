use indexmap::IndexMap;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Canonical key order for tool.yaml: id first, then name, description, and the rest.
const TOOL_YAML_KEY_ORDER: &[&str] = &[
    "id",
    "name",
    "description",
    "type",
    "input_modes",
    "output_modes",
    "systemInstructions",
    "arguments",
    "examples",
    "tags",
    "createdBy",
    "managedBy",
    "version",
    "feeds",
    "output",
];

const YAML_NAMES: &[&str] = &["tool.yaml", "tool.yml", "config.yaml", "config.yml"];
const HANDLER_NAMES: &[&str] = &["handler.js", "index.js", "handler.ts", "index.ts"];

/// True if tool YAML has type "JS" (case-insensitive). JS tools have no package.json.
fn tool_type_is_js(yaml: &Value) -> bool {
    yaml.get("type")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().eq_ignore_ascii_case("js"))
        .unwrap_or(false)
}

/// True when a directory contains a recognised tool YAML file.
pub fn is_tool_dir(path: &Path) -> bool {
    YAML_NAMES.iter().any(|name| path.join(name).is_file())
}

/// List tool directories that contain a tool YAML file.
pub fn list_tool_dirs(base: &Path) -> Vec<PathBuf> {
    if !base.is_dir() {
        return Vec::new();
    }
    std::fs::read_dir(base)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && is_tool_dir(path))
        .collect()
}

/// Resolve tools directory: env VGEN_TOOLS_DIR or default "tools" under cwd.
pub fn default_tools_dir() -> PathBuf {
    std::env::var("VGEN_TOOLS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("tools"))
}

/// Load only the tool YAML (without handler/package.json) and return (Value, yaml_path).
/// Used for the test command where only YAML metadata is needed.
pub fn load_tool_yaml(
    tool_dir: &Path,
) -> Result<(Value, PathBuf), Box<dyn std::error::Error + Send + Sync>> {
    if !tool_dir.exists() || !tool_dir.is_dir() {
        return Err(format!("Tool folder not found: {}", tool_dir.display()).into());
    }
    let yaml_path = find_yaml(tool_dir)?;
    let content = std::fs::read_to_string(&yaml_path)
        .map_err(|e| format!("Failed to read {}: {}", yaml_path.display(), e))?;
    let value: Value = serde_yaml::from_str(&content)
        .map_err(|e| format!("Invalid YAML in {}: {}", yaml_path.display(), e))?;
    Ok((value, yaml_path))
}

/// Load only the tool YAML and return (id, yaml_path). Used for pull when handler/package may be missing.
pub fn get_tool_id_and_yaml_path(
    tool_dir: &Path,
) -> Result<(String, PathBuf), Box<dyn std::error::Error + Send + Sync>> {
    if !tool_dir.exists() || !tool_dir.is_dir() {
        return Err(format!("Tool folder not found: {}", tool_dir.display()).into());
    }
    let yaml_path = find_yaml(tool_dir)?;
    let content = std::fs::read_to_string(&yaml_path)
        .map_err(|e| format!("Failed to read {}: {}", yaml_path.display(), e))?;
    let value: Value = serde_yaml::from_str(&content)
        .map_err(|e| format!("Invalid YAML in {}: {}", yaml_path.display(), e))?;
    let id = value
        .get("id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            format!(
                "No id in tool YAML ({}); push first or add id.",
                yaml_path.display()
            )
        })?
        .to_string();
    Ok((id, yaml_path))
}

/// Load tool spec from a folder: tool.yaml (or single yaml), handler file, package.json.
/// Returns (request body for create/update, path to the yaml file for id write-back).
pub fn load_tool_from_dir(
    tool_dir: &Path,
) -> Result<(Value, PathBuf), Box<dyn std::error::Error + Send + Sync>> {
    if !tool_dir.exists() || !tool_dir.is_dir() {
        return Err(format!("Tool folder not found: {}", tool_dir.display()).into());
    }

    let yaml_path = find_yaml(tool_dir)?;
    let yaml_content = std::fs::read_to_string(&yaml_path)
        .map_err(|e| format!("Failed to read {}: {}", yaml_path.display(), e))?;
    let mut yaml_value: Value = serde_yaml::from_str(&yaml_content)
        .map_err(|e| format!("Invalid YAML in {}: {}", yaml_path.display(), e))?;
    yaml_value = crate::specs::normalize::normalize_mongo_oids(yaml_value);

    let handler_path = find_handler(tool_dir)?;
    let code = std::fs::read_to_string(&handler_path)
        .map_err(|e| format!("Failed to read handler {}: {}", handler_path.display(), e))?;

    let is_js = tool_type_is_js(&yaml_value);
    if !is_js {
        let package_path = tool_dir.join("package.json");
        if !package_path.exists() {
            return Err(format!("Missing package.json in {}", tool_dir.display()).into());
        }
        let package_str = std::fs::read_to_string(&package_path)
            .map_err(|e| format!("Failed to read package.json: {}", e))?;
        let package_json: Value = serde_json::from_str(&package_str)
            .map_err(|e| format!("Invalid package.json: {}", e))?;
        if let Some(obj) = yaml_value.as_object_mut() {
            obj.insert("code".to_string(), Value::String(code));
            obj.insert("packageJson".to_string(), package_json);
        } else {
            return Err("Tool YAML must be an object".into());
        }
    } else {
        if let Some(obj) = yaml_value.as_object_mut() {
            obj.insert("code".to_string(), Value::String(code));
        } else {
            return Err("Tool YAML must be an object".into());
        }
    }

    Ok((yaml_value, yaml_path))
}

fn find_yaml(dir: &Path) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    for name in YAML_NAMES {
        let p = dir.join(name);
        if p.exists() {
            return Ok(p);
        }
    }
    let mut yamls: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .map(|e| e == "yaml" || e == "yml")
                .unwrap_or(false)
        })
        .collect();
    if yamls.len() == 1 {
        return Ok(yamls.remove(0));
    }
    if yamls.is_empty() {
        return Err(format!(
            "No YAML file found in {}. Add tool.yaml or config.yaml",
            dir.display()
        )
        .into());
    }
    Err(format!(
        "Multiple YAML files in {}; use tool.yaml to specify",
        dir.display()
    )
    .into())
}

fn find_handler(dir: &Path) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    for name in HANDLER_NAMES {
        let p = dir.join(name);
        if p.exists() {
            return Ok(p);
        }
    }
    Err(format!(
        "No handler file found in {} (looking for handler.js, index.js, handler.ts, index.ts)",
        dir.display()
    )
    .into())
}

/// Resolve handler path for writing: use first existing handler, else handler.js.
fn handler_path_for_write(dir: &Path) -> PathBuf {
    for name in HANDLER_NAMES {
        let p = dir.join(name);
        if p.exists() {
            return p;
        }
    }
    dir.join("handler.js")
}

/// Build a map with keys in canonical order so tool.yaml serializes consistently.
fn reorder_tool_yaml_keys(
    mut value: Value,
) -> Result<IndexMap<String, Value>, Box<dyn std::error::Error + Send + Sync>> {
    let obj = value.as_object_mut().ok_or("YAML root is not an object")?;
    let mut ordered: IndexMap<String, Value> = IndexMap::new();
    for &key in TOOL_YAML_KEY_ORDER {
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

/// Write tool from get-record response data: tool.yaml (reordered), handler file, package.json.
pub fn write_tool_from_record(
    tool_dir: &Path,
    data: &Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let data_obj = data
        .as_object()
        .ok_or("get-record tool data is not an object")?;

    let code = data_obj
        .get("code")
        .and_then(|v| v.as_str())
        .ok_or("Tool record has no code")?
        .to_string();
    let package_json = data_obj.get("packageJson");

    let mut yaml_value = data.clone();
    let yaml_obj = yaml_value
        .as_object_mut()
        .ok_or("Tool record is not an object")?;
    yaml_obj.remove("code");
    yaml_obj.remove("packageJson");
    let yaml_value = crate::specs::normalize::normalize_mongo_oids(yaml_value);

    let yaml_path = find_yaml(tool_dir)?;
    let ordered = reorder_tool_yaml_keys(yaml_value)?;
    let yaml_out = serde_yaml::to_string(&ordered).map_err(|e| e.to_string())?;
    std::fs::write(&yaml_path, yaml_out)
        .map_err(|e| format!("Failed to write {}: {}", yaml_path.display(), e))?;

    let handler_path = handler_path_for_write(tool_dir);
    std::fs::write(&handler_path, code)
        .map_err(|e| format!("Failed to write handler {}: {}", handler_path.display(), e))?;

    if let Some(pkg) = package_json {
        let package_path = tool_dir.join("package.json");
        let package_out = serde_json::to_string_pretty(pkg).map_err(|e| e.to_string())?;
        std::fs::write(&package_path, package_out)
            .map_err(|e| format!("Failed to write package.json: {}", e))?;
    }

    Ok(())
}

/// Derive a filesystem-safe folder name from a tool's `name` field:
/// lowercase, spaces/slashes replaced with hyphens, non-alphanumeric (except `-` and `.`) stripped.
fn tool_name_to_folder(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c == ' ' || c == '/' { '-' } else { c })
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '.')
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// Upsert a tool from API response data into tools_dir/<folder>/. Creates the directory and a
/// stub tool.yaml if the folder doesn't already exist, then writes the full record (YAML + handler
/// + package.json). Used by `vgen sync`.
pub fn upsert_tool_from_api_data(
    tools_dir: &Path,
    data: &serde_json::Value,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let name = data
        .get("name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or("Tool record has no name field")?;

    let folder = tool_name_to_folder(name);
    let tool_dir = tools_dir.join(&folder);
    std::fs::create_dir_all(&tool_dir)
        .map_err(|e| format!("Failed to create tool dir {}: {}", tool_dir.display(), e))?;

    // Ensure a yaml stub exists so write_tool_from_record can find it via find_yaml.
    let yaml_path = tool_dir.join("tool.yaml");
    if !yaml_path.exists() {
        // Also check for the other recognised YAML names before creating one.
        let has_yaml = YAML_NAMES.iter().any(|n| tool_dir.join(n).exists());
        if !has_yaml {
            std::fs::write(&yaml_path, "")
                .map_err(|e| format!("Failed to create stub tool.yaml: {}", e))?;
        }
    }

    // Ensure handler stub exists so write_tool_from_record's find_yaml path is satisfied.
    let handler_path = tool_dir.join("handler.js");
    if !HANDLER_NAMES.iter().any(|n| tool_dir.join(n).exists()) {
        std::fs::write(&handler_path, "")
            .map_err(|e| format!("Failed to create stub handler.js: {}", e))?;
    }

    write_tool_from_record(&tool_dir, data)?;
    Ok(folder)
}

/// Update the tool's YAML file to set id. Parses YAML to Value, sets id, reorders keys, writes back.
pub fn write_tool_id_to_yaml(
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

    let ordered = reorder_tool_yaml_keys(value)?;
    let out = serde_yaml::to_string(&ordered).map_err(|e| e.to_string())?;
    std::fs::write(yaml_path, out)
        .map_err(|e| format!("Failed to write {}: {}", yaml_path.display(), e))?;
    Ok(())
}
