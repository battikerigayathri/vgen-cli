use serde::Deserialize;
use serde_json::Value;

use crate::error::{Result, SmritiError};
use crate::workflow::types::{
    AuthorBundle, AuthorFlow, AuthorMeta, AuthorSchemaSection, AuthorStage, SourceFormat,
    WorkflowDefinition, WorkflowStageDef,
};
use crate::workflow::validate::{validate_json_schema, validate_semantics};

/// Tantra / Prajna create body — `authorBundle` or flattened `definition`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkflowDefinitionRequest {
    pub author_bundle: Option<Value>,
    pub definition: Option<WorkflowDefinition>,
    pub source_format: Option<SourceFormat>,
}

/// Tantra / Prajna get body.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetWorkflowDefinitionRequest {
    pub slug: String,
    pub version: Option<u32>,
}

/// Parse and validate a create request into a push-ready [`WorkflowDefinition`].
pub fn parse_create_definition_request(
    req: CreateWorkflowDefinitionRequest,
) -> Result<WorkflowDefinition> {
    match (req.author_bundle, req.definition) {
        (Some(bundle_value), None) => parse_author_bundle(bundle_value, req.source_format),
        (None, Some(def)) => parse_flattened_definition(def),
        (Some(_), Some(_)) => Err(SmritiError::InvalidRequest(
            "provide either authorBundle or definition, not both".into(),
        )),
        (None, None) => Err(SmritiError::InvalidRequest(
            "authorBundle or definition is required".into(),
        )),
    }
}

fn parse_author_bundle(
    bundle_value: Value,
    source_format: Option<SourceFormat>,
) -> Result<WorkflowDefinition> {
    validate_json_schema(&bundle_value)?;
    let bundle: AuthorBundle = serde_json::from_value(bundle_value).map_err(|e| {
        SmritiError::InvalidResponse(format!("failed to deserialize author bundle: {e}"))
    })?;
    validate_semantics(&bundle)?;
    let source_format = source_format.ok_or_else(|| {
        SmritiError::InvalidRequest("sourceFormat is required when using authorBundle".into())
    })?;
    Ok(bundle.into_definition(source_format))
}

fn parse_flattened_definition(def: WorkflowDefinition) -> Result<WorkflowDefinition> {
    if def.id.is_some() {
        return Err(SmritiError::InvalidRequest(
            "_id must not be set on create".into(),
        ));
    }
    if def.created_at.is_some() || def.updated_at.is_some() {
        return Err(SmritiError::InvalidRequest(
            "createdAt and updatedAt must not be set on create".into(),
        ));
    }

    let bundle = definition_to_author_bundle(&def);
    // Structural shape is enforced by WorkflowDefinition deserialization; semantic lint only.
    validate_semantics(&bundle)?;
    Ok(def)
}

fn definition_to_author_bundle(def: &WorkflowDefinition) -> AuthorBundle {
    AuthorBundle {
        meta: AuthorMeta {
            id: def.id.clone(),
            slug: def.slug.clone(),
            name: def.name.clone(),
            workflow_type: def.workflow_type.clone(),
            version: def.version,
            description: def.description.clone(),
        },
        schema: AuthorSchemaSection {
            fields: def.fields.clone(),
        },
        flow: AuthorFlow {
            initial_stage: def.initial_stage.clone(),
            stages: def.stages.iter().map(stage_def_to_author).collect(),
        },
        gates: def.gates.clone(),
    }
}

fn stage_def_to_author(stage: &WorkflowStageDef) -> AuthorStage {
    AuthorStage {
        id: stage.id.clone(),
        label: stage.label.clone(),
        kind: stage.kind,
        hitl_slug: stage.hitl_slug.clone(),
        agent_slug: stage.agent_slug.clone(),
        done_when: stage.done_when.clone(),
        expected_outcome: stage.expected_outcome.clone(),
        next: stage.next.clone(),
        back_to: stage.back_to.clone(),
        transitions: stage.transitions.clone(),
        playbook: stage.playbook.clone(),
        terminal: stage.is_terminal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::loader::WorkflowDefinitionLoader;
    use std::path::PathBuf;

    fn split_fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/purchase-requisition-split")
    }

    fn split_fixture() -> WorkflowDefinition {
        WorkflowDefinitionLoader::load_from_dir(&split_fixture_dir()).unwrap()
    }

    fn split_author_bundle_json() -> Value {
        let meta = std::fs::read_to_string(split_fixture_dir().join("meta.yaml")).unwrap();
        let schema = std::fs::read_to_string(split_fixture_dir().join("schema.yaml")).unwrap();
        let flow = std::fs::read_to_string(split_fixture_dir().join("flow.yaml")).unwrap();
        let meta: Value = serde_yaml::from_str(&meta).unwrap();
        let schema: Value = serde_yaml::from_str(&schema).unwrap();
        let flow: Value = serde_yaml::from_str(&flow).unwrap();
        serde_json::json!({
            "meta": meta,
            "schema": schema,
            "flow": flow,
            "gates": []
        })
    }

    #[test]
    fn parse_author_bundle_request() {
        let def = parse_create_definition_request(CreateWorkflowDefinitionRequest {
            author_bundle: Some(split_author_bundle_json()),
            definition: None,
            source_format: Some(SourceFormat::SplitYaml),
        })
        .unwrap();
        assert_eq!(def.slug, "purchase-requisition-v1");
        assert_eq!(def.source_format, SourceFormat::SplitYaml);
    }

    #[test]
    fn parse_flattened_definition_request() {
        let loaded = split_fixture();
        let def = parse_create_definition_request(CreateWorkflowDefinitionRequest {
            author_bundle: None,
            definition: Some(loaded),
            source_format: None,
        })
        .unwrap();
        assert_eq!(def.slug, "purchase-requisition-v1");
    }

    #[test]
    fn rejects_both_bundle_and_definition() {
        let err = parse_create_definition_request(CreateWorkflowDefinitionRequest {
            author_bundle: Some(serde_json::json!({})),
            definition: Some(split_fixture()),
            source_format: Some(SourceFormat::SplitYaml),
        })
        .unwrap_err();
        assert!(matches!(err, SmritiError::InvalidRequest(_)));
    }

    #[test]
    fn rejects_author_bundle_without_source_format() {
        let err = parse_create_definition_request(CreateWorkflowDefinitionRequest {
            author_bundle: Some(split_author_bundle_json()),
            definition: None,
            source_format: None,
        })
        .unwrap_err();
        assert!(matches!(err, SmritiError::InvalidRequest(_)));
    }

    #[test]
    fn rejects_definition_with_id() {
        let mut def = split_fixture();
        def.id = Some("507f1f77bcf86cd799439011".into());
        let err = parse_create_definition_request(CreateWorkflowDefinitionRequest {
            author_bundle: None,
            definition: Some(def),
            source_format: None,
        })
        .unwrap_err();
        assert!(matches!(err, SmritiError::InvalidRequest(_)));
    }
}
