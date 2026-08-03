//! Workflow author layout loading — delegates to `smriti_client::WorkflowDefinitionLoader`
//! so validate and push paths share the same layout rules (including `schema.yaml` in split layout).

use serde_json::Value;
use smriti_client::{SmritiError, SourceFormat, WorkflowDefinition, WorkflowDefinitionLoader};
use std::path::{Path, PathBuf};

use crate::specs::normalize;
use crate::specs::AuthorFormat;
use smriti_client::AuthorFormat as SmritiAuthorFormat;

/// Load and validate a workflow directory, returning the author bundle for push.
pub fn load_workflow_from_dir(
    dir: &Path,
) -> Result<(Value, PathBuf, AuthorFormat), Box<dyn std::error::Error + Send + Sync>> {
    let def = WorkflowDefinitionLoader::load_from_dir(dir).map_err(map_smriti_err)?;
    let format = source_format_to_author_format(def.source_format);
    let version_path = version_file_path(dir, format)?;
    let bundle = definition_to_bundle(&def);
    let normalized = normalize::normalize_mongo_oids(bundle);
    Ok((normalized, version_path, format))
}

/// Detect author layout; aligned with smriti loader rules.
pub fn detect_format(dir: &Path) -> Result<AuthorFormat, Box<dyn std::error::Error + Send + Sync>> {
    WorkflowDefinitionLoader::detect_format(dir)
        .map(to_author_format)
        .map_err(map_smriti_err)
}

/// True when a directory contains a recognised workflow author layout.
pub fn is_workflow_dir(path: &Path) -> bool {
    detect_format(path).is_ok()
}

fn version_file_path(
    dir: &Path,
    format: AuthorFormat,
) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    let path = match format {
        AuthorFormat::BundleJson => dir.join("workflow.json"),
        AuthorFormat::BundleYaml => {
            let yaml = dir.join("workflow.yaml");
            if yaml.is_file() {
                yaml
            } else {
                dir.join("workflow.yml")
            }
        }
        AuthorFormat::SplitYaml => {
            let meta = dir.join("meta.yaml");
            if meta.is_file() {
                meta
            } else {
                dir.join("meta.yml")
            }
        }
    };
    if !path.is_file() {
        return Err(format!("version file not found: {}", path.display()).into());
    }
    Ok(path)
}

fn definition_to_bundle(def: &WorkflowDefinition) -> Value {
    let mut meta = serde_json::Map::new();
    if let Some(id) = &def.id {
        meta.insert("id".to_string(), Value::String(id.clone()));
    }
    meta.insert("slug".to_string(), Value::String(def.slug.clone()));
    meta.insert("name".to_string(), Value::String(def.name.clone()));
    meta.insert(
        "workflowType".to_string(),
        Value::String(def.workflow_type.clone()),
    );
    meta.insert("version".to_string(), Value::Number(def.version.into()));
    if let Some(desc) = &def.description {
        meta.insert("description".to_string(), Value::String(desc.clone()));
    }

    let mut playbooks = serde_json::Map::new();
    for stage in &def.stages {
        if let Some(playbook) = &stage.playbook {
            if let Ok(playbook_val) = serde_json::to_value(playbook) {
                playbooks.insert(stage.id.clone(), playbook_val);
            }
        }
    }

    serde_json::json!({
        "meta": Value::Object(meta),
        "schema": {
            "fields": def.fields,
        },
        "flow": {
            "initialStage": def.initial_stage,
            "stages": def.stages,
        },
        "gates": def.gates,
        "playbooks": Value::Object(playbooks),
    })
}

fn source_format_to_author_format(fmt: SourceFormat) -> AuthorFormat {
    match fmt {
        SourceFormat::SplitYaml => AuthorFormat::SplitYaml,
        SourceFormat::BundleYaml => AuthorFormat::BundleYaml,
        SourceFormat::BundleJson => AuthorFormat::BundleJson,
    }
}

fn to_author_format(fmt: SmritiAuthorFormat) -> AuthorFormat {
    match fmt {
        SmritiAuthorFormat::SplitYaml => AuthorFormat::SplitYaml,
        SmritiAuthorFormat::BundleYaml => AuthorFormat::BundleYaml,
        SmritiAuthorFormat::BundleJson => AuthorFormat::BundleJson,
    }
}

fn map_smriti_err(err: SmritiError) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow_validate::validate_workflow_dir;

    fn pr_agent_v2_root() -> PathBuf {
        PathBuf::from("/Users/roshankgujarathi/Workspace/ResMed/pr-agent-v2")
    }

    #[test]
    fn validate_and_load_parity_oracle_workflow() {
        let root = pr_agent_v2_root();
        if !root.is_dir() {
            eprintln!("Skipping: pr-agent-v2 not found at {}", root.display());
            return;
        }

        let dir = root.join("workflows/oracle-purchase-requisition-v1");
        let validate_data = validate_workflow_dir(&dir).expect("validate should pass");
        let (bundle, _, format) = load_workflow_from_dir(&dir).expect("load should pass");

        assert_eq!(format, AuthorFormat::SplitYaml);
        assert_eq!(
            bundle
                .get("meta")
                .and_then(|m| m.get("slug"))
                .and_then(|v| v.as_str()),
            Some(validate_data.slug.as_str())
        );
        assert_eq!(
            bundle
                .get("meta")
                .and_then(|m| m.get("version"))
                .and_then(|v| v.as_u64()),
            Some(validate_data.version as u64)
        );
    }
}
