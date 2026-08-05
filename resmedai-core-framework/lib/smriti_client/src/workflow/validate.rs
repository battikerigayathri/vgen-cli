use std::collections::HashSet;
use std::sync::LazyLock;

use jsonschema::Validator;
use serde_json::Value;
use workflow_schema::WORKFLOW_DEFINITION_SCHEMA;

use crate::error::{Result, SmritiError};
use crate::workflow::types::{AuthorBundle, StageDoneWhen, WorkflowFieldDef, WorkflowStageKind};

static SCHEMA_VALIDATOR: LazyLock<Validator> = LazyLock::new(|| {
    let schema: Value = serde_json::from_str(WORKFLOW_DEFINITION_SCHEMA)
        .expect("workflow-definition.schema.json must parse");
    Validator::new(&schema).expect("workflow-definition.schema.json must compile")
});

pub fn validate_json_schema(value: &Value) -> Result<()> {
    let errors: Vec<String> = SCHEMA_VALIDATOR
        .iter_errors(value)
        .map(|err| err.to_string())
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(SmritiError::SchemaValidation(errors.join("; ")))
    }
}

pub fn validate_semantics(bundle: &AuthorBundle) -> Result<()> {
    let field_keys: HashSet<&str> = bundle
        .schema
        .fields
        .iter()
        .map(|f| f.key.as_str())
        .collect();

    let stage_ids: HashSet<&str> = bundle.flow.stages.iter().map(|s| s.id.as_str()).collect();

    if !stage_ids.contains(bundle.flow.initial_stage.as_str()) {
        return Err(SmritiError::SemanticValidation {
            path: "flow.initialStage".into(),
            message: format!(
                "initialStage '{}' does not match any stage id",
                bundle.flow.initial_stage
            ),
        });
    }

    if stage_ids.len() != bundle.flow.stages.len() {
        return Err(SmritiError::SemanticValidation {
            path: "flow.stages".into(),
            message: "duplicate stage ids are not allowed".into(),
        });
    }

    let mut terminal_count = 0usize;

    for stage in &bundle.flow.stages {
        let path_prefix = format!("flow.stages[{}]", stage.id);

        if let Some(next) = &stage.next
            && !stage_ids.contains(next.as_str())
        {
            return Err(SmritiError::SemanticValidation {
                path: format!("{path_prefix}.next"),
                message: format!("next stage '{next}' does not exist"),
            });
        }

        if let Some(back_to) = &stage.back_to
            && !stage_ids.contains(back_to.as_str())
        {
            return Err(SmritiError::SemanticValidation {
                path: format!("{path_prefix}.back_to"),
                message: format!("back_to stage '{back_to}' does not exist"),
            });
        }

        if let StageDoneWhen::RequiredInputs(keys) = &stage.done_when {
            for key in keys {
                if !field_keys.contains(key.as_str()) {
                    return Err(SmritiError::SemanticValidation {
                        path: format!("{path_prefix}.doneWhen"),
                        message: format!("doneWhen references unknown field key '{key}'"),
                    });
                }
            }
        }

        match stage.kind {
            WorkflowStageKind::AgentTask
                if stage.expected_outcome.as_deref().unwrap_or("").is_empty() =>
            {
                return Err(SmritiError::SemanticValidation {
                    path: format!("{path_prefix}.expectedOutcome"),
                    message: "agent_task stages require expectedOutcome".into(),
                });
            }
            WorkflowStageKind::Collect | WorkflowStageKind::Review
                if stage.hitl_slug.as_deref().unwrap_or("").is_empty() =>
            {
                return Err(SmritiError::SemanticValidation {
                    path: format!("{path_prefix}.hitlSlug"),
                    message: format!("{} stages require hitlSlug", stage_kind_name(stage.kind)),
                });
            }
            _ => {}
        }

        if stage.terminal || matches!(stage.kind, WorkflowStageKind::Terminal) {
            terminal_count += 1;
        }
    }

    if terminal_count == 0 {
        return Err(SmritiError::SemanticValidation {
            path: "flow.stages".into(),
            message: "at least one terminal stage is required".into(),
        });
    }

    validate_required_from_stage_refs(&bundle.schema.fields, &stage_ids)?;

    Ok(())
}

fn validate_required_from_stage_refs(
    fields: &[WorkflowFieldDef],
    stage_ids: &HashSet<&str>,
) -> Result<()> {
    for field in fields {
        if let Some(stage) = &field.required_from_stage
            && !stage_ids.contains(stage.as_str())
        {
            return Err(SmritiError::SemanticValidation {
                path: format!("schema.fields[{}].requiredFromStage", field.key),
                message: format!("requiredFromStage '{stage}' does not exist"),
            });
        }
    }
    Ok(())
}

fn stage_kind_name(kind: WorkflowStageKind) -> &'static str {
    match kind {
        WorkflowStageKind::Collect => "collect",
        WorkflowStageKind::Review => "review",
        WorkflowStageKind::AgentTask => "agent_task",
        WorkflowStageKind::Terminal => "terminal",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::loader::WorkflowDefinitionLoader;
    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    #[test]
    fn schema_rejects_missing_meta() {
        let err = validate_json_schema(&serde_json::json!({
            "schema": { "fields": [] },
            "flow": { "initialStage": "a", "stages": [] }
        }))
        .unwrap_err();
        assert!(matches!(err, SmritiError::SchemaValidation(_)));
    }

    #[test]
    fn semantic_rejects_bad_next_reference() {
        let mut bundle: AuthorBundle = serde_json::from_value(serde_json::json!({
            "meta": {
                "slug": "x-v1",
                "name": "X",
                "workflowType": "x",
                "version": 1
            },
            "schema": {
                "fields": [{
                    "key": "a",
                    "label": "A",
                    "type": "string",
                    "bag": "inputs"
                }]
            },
            "flow": {
                "initialStage": "s1",
                "stages": [{
                    "id": "s1",
                    "label": "S1",
                    "kind": "collect",
                    "hitlSlug": "form",
                    "doneWhen": ["a"],
                    "next": "missing"
                }, {
                    "id": "done",
                    "label": "Done",
                    "kind": "terminal",
                    "terminal": true,
                    "doneWhen": "terminal"
                }]
            },
            "gates": []
        }))
        .unwrap();

        // Fix terminal for valid schema path — still bad next on s1
        let err = validate_semantics(&bundle).unwrap_err();
        assert!(matches!(err, SmritiError::SemanticValidation { .. }));

        bundle.flow.stages[0].next = None;
        validate_semantics(&bundle).expect("valid after fix");
    }

    #[test]
    fn semantic_rejects_agent_task_without_expected_outcome() {
        let bundle: AuthorBundle = serde_json::from_value(serde_json::json!({
            "meta": {
                "slug": "x-v1",
                "name": "X",
                "workflowType": "x",
                "version": 1
            },
            "schema": { "fields": [] },
            "flow": {
                "initialStage": "task",
                "stages": [{
                    "id": "task",
                    "label": "Task",
                    "kind": "agent_task",
                    "doneWhen": "validator_pass",
                    "next": "done"
                }, {
                    "id": "done",
                    "label": "Done",
                    "kind": "terminal",
                    "terminal": true,
                    "doneWhen": "terminal"
                }]
            },
            "gates": []
        }))
        .unwrap();

        let err = validate_semantics(&bundle).unwrap_err();
        assert!(matches!(err, SmritiError::SemanticValidation { .. }));
    }

    #[test]
    fn loaded_fixtures_pass_semantic_validation() {
        for name in ["purchase-requisition-split", "github-pr-create-bundle"] {
            let dir = fixtures_dir().join(name);
            WorkflowDefinitionLoader::load_from_dir(&dir)
                .unwrap_or_else(|e| panic!("fixture {name} should load: {e}"));
        }
    }
}
