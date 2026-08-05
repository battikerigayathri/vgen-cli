use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::error::{Result, SmritiError};
use crate::workflow::types::{AuthorBundle, SourceFormat, WorkflowDefinition};
use crate::workflow::validate::{validate_json_schema, validate_semantics};

/// Detected author layout for a workflow directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorFormat {
    SplitYaml,
    BundleYaml,
    BundleJson,
}

impl AuthorFormat {
    pub fn source_format(self) -> SourceFormat {
        match self {
            Self::SplitYaml => SourceFormat::SplitYaml,
            Self::BundleYaml => SourceFormat::BundleYaml,
            Self::BundleJson => SourceFormat::BundleJson,
        }
    }
}

pub struct WorkflowDefinitionLoader;

impl WorkflowDefinitionLoader {
    /// Detect YAML-based layout (split or bundle), excluding JSON bundle.
    pub fn detect_yaml_format(dir: &Path) -> Result<AuthorFormat> {
        if dir.join("workflow.yaml").is_file() {
            return Ok(AuthorFormat::BundleYaml);
        }
        if dir.join("meta.yaml").is_file() && dir.join("flow.yaml").is_file() {
            return Ok(AuthorFormat::SplitYaml);
        }
        Err(SmritiError::InvalidAuthorLayout {
            path: dir.display().to_string(),
            reason: "expected workflow.yaml or meta.yaml+flow.yaml for --format yaml".into(),
        })
    }

    /// Detect format from directory layout per authoring-formats.md rules.
    pub fn detect_format(dir: &Path) -> Result<AuthorFormat> {
        if dir.join("workflow.json").is_file() {
            return Ok(AuthorFormat::BundleJson);
        }
        if dir.join("workflow.yaml").is_file() {
            return Ok(AuthorFormat::BundleYaml);
        }
        if dir.join("meta.yaml").is_file() && dir.join("flow.yaml").is_file() {
            return Ok(AuthorFormat::SplitYaml);
        }

        Err(SmritiError::InvalidAuthorLayout {
            path: dir.display().to_string(),
            reason: "expected workflow.json, workflow.yaml, or meta.yaml+flow.yaml".into(),
        })
    }

    /// Load from workspace folder `workflows/<name>/`.
    pub fn load_from_dir(dir: &Path) -> Result<WorkflowDefinition> {
        Self::load(dir, None)
    }

    /// Parse explicit path (file or dir) with optional format override.
    pub fn load(path: &Path, format_override: Option<AuthorFormat>) -> Result<WorkflowDefinition> {
        if path.is_file() {
            return Self::load_file(path, format_override);
        }
        if !path.is_dir() {
            return Err(SmritiError::InvalidAuthorLayout {
                path: path.display().to_string(),
                reason: "path is not a file or directory".into(),
            });
        }

        let format = match format_override {
            Some(f) => f,
            None => Self::detect_format(path)?,
        };
        let bundle_value = match format {
            AuthorFormat::SplitYaml => Self::load_split_yaml(path)?,
            AuthorFormat::BundleYaml => Self::read_yaml_file(&path.join("workflow.yaml"))?,
            AuthorFormat::BundleJson => Self::read_json_file(&path.join("workflow.json"))?,
        };

        Self::parse_bundle_value(bundle_value, format.source_format())
    }

    fn load_file(path: &Path, format_override: Option<AuthorFormat>) -> Result<WorkflowDefinition> {
        let format = format_override.or_else(|| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .and_then(|ext| match ext {
                    "json" => Some(AuthorFormat::BundleJson),
                    "yaml" | "yml" => Some(AuthorFormat::BundleYaml),
                    _ => None,
                })
        });

        let format = format.ok_or_else(|| SmritiError::InvalidAuthorLayout {
            path: path.display().to_string(),
            reason: "cannot infer format from file extension; use --format".into(),
        })?;

        let bundle_value = match format {
            AuthorFormat::BundleJson => Self::read_json_file(path)?,
            AuthorFormat::BundleYaml => Self::read_yaml_file(path)?,
            AuthorFormat::SplitYaml => {
                return Err(SmritiError::InvalidAuthorLayout {
                    path: path.display().to_string(),
                    reason: "split YAML requires a directory with meta.yaml and flow.yaml".into(),
                });
            }
        };

        Self::parse_bundle_value(bundle_value, format.source_format())
    }

    fn parse_bundle_value(
        bundle_value: Value,
        source_format: SourceFormat,
    ) -> Result<WorkflowDefinition> {
        validate_json_schema(&bundle_value)?;
        let bundle: AuthorBundle = serde_json::from_value(bundle_value).map_err(|e| {
            SmritiError::InvalidResponse(format!("failed to deserialize author bundle: {e}"))
        })?;
        validate_semantics(&bundle)?;
        Ok(bundle.into_definition(source_format))
    }

    fn load_split_yaml(dir: &Path) -> Result<Value> {
        let schema_path = dir.join("schema.yaml");
        if !schema_path.is_file() {
            return Err(SmritiError::InvalidAuthorLayout {
                path: dir.display().to_string(),
                reason: "split layout requires schema.yaml".into(),
            });
        }

        let meta = Self::read_yaml_file(&dir.join("meta.yaml"))?;
        let schema = Self::read_yaml_file(&schema_path)?;
        let flow = Self::read_yaml_file(&dir.join("flow.yaml"))?;
        let gates_path = dir.join("gates.yaml");
        let gates = if gates_path.is_file() {
            Self::read_yaml_file(&gates_path)?
        } else {
            Value::Array(Vec::new())
        };

        Ok(serde_json::json!({
            "meta": meta,
            "schema": schema,
            "flow": flow,
            "gates": gates,
        }))
    }

    fn read_yaml_file(path: &Path) -> Result<Value> {
        let content =
            fs::read_to_string(path).map_err(|e| SmritiError::io(path.display().to_string(), e))?;
        serde_yaml::from_str(&content).map_err(|e| SmritiError::YamlParse(e.to_string()))
    }

    fn read_json_file(path: &Path) -> Result<Value> {
        let content =
            fs::read_to_string(path).map_err(|e| SmritiError::io(path.display().to_string(), e))?;
        serde_json::from_str(&content).map_err(|e| SmritiError::JsonParse(e.to_string()))
    }
}

/// Resolve `--path` against optional workflows root directory.
///
/// Resolution order for relative paths: absolute → existing cwd-relative → `WORKFLOWS_DIR`/name.
pub fn resolve_workflow_path(workflows_dir: &Path, path: &str) -> PathBuf {
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        return candidate.to_path_buf();
    }
    if candidate.exists() {
        return candidate.to_path_buf();
    }
    workflows_dir.join(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    #[test]
    fn detects_split_yaml_layout() {
        let dir = fixtures_dir().join("purchase-requisition-split");
        assert_eq!(
            WorkflowDefinitionLoader::detect_format(&dir).unwrap(),
            AuthorFormat::SplitYaml
        );
    }

    #[test]
    fn detects_bundle_json_layout() {
        let dir = fixtures_dir().join("github-pr-create-bundle");
        assert_eq!(
            WorkflowDefinitionLoader::detect_format(&dir).unwrap(),
            AuthorFormat::BundleJson
        );
    }

    #[test]
    fn loads_split_yaml_fixture() {
        let dir = fixtures_dir().join("purchase-requisition-split");
        let def = WorkflowDefinitionLoader::load_from_dir(&dir).unwrap();
        assert_eq!(def.slug, "purchase-requisition-v1");
        assert_eq!(def.workflow_type, "purchase_requisition");
        assert_eq!(def.source_format, SourceFormat::SplitYaml);
        assert_eq!(def.initial_stage, "collect_vendor");
        assert!(!def.stages.is_empty());
    }

    #[test]
    fn loads_bundle_json_fixture() {
        let dir = fixtures_dir().join("github-pr-create-bundle");
        let def = WorkflowDefinitionLoader::load_from_dir(&dir).unwrap();
        assert_eq!(def.slug, "github-pr-create-v1");
        assert_eq!(def.workflow_type, "github_pr_create");
        assert_eq!(def.source_format, SourceFormat::BundleJson);
        assert_eq!(def.initial_stage, "collect_intent");
    }

    #[test]
    fn rejects_invalid_layout() {
        let dir = std::env::temp_dir().join(format!(
            "wf-invalid-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let err = WorkflowDefinitionLoader::detect_format(&dir).unwrap_err();
        assert!(matches!(err, SmritiError::InvalidAuthorLayout { .. }));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn split_missing_schema_is_error() {
        let dir = std::env::temp_dir().join(format!(
            "wf-no-schema-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("meta.yaml"),
            "slug: x\nname: X\nworkflowType: x\nversion: 1\n",
        )
        .unwrap();
        fs::write(dir.join("flow.yaml"), "initialStage: a\nstages: []\n").unwrap();
        let err = WorkflowDefinitionLoader::load_from_dir(&dir).unwrap_err();
        assert!(matches!(err, SmritiError::InvalidAuthorLayout { .. }));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn resolve_workflow_path_relative_and_absolute() {
        let root = Path::new("/workflows");
        assert_eq!(
            resolve_workflow_path(root, "purchase-requisition"),
            PathBuf::from("/workflows/purchase-requisition")
        );
        assert_eq!(
            resolve_workflow_path(root, "/abs/path"),
            PathBuf::from("/abs/path")
        );
    }
}
