use serde_json::{Value, json};

use crate::workflow::advance::compute_missing_required;
use crate::workflow::instance::{WorkflowInstance, WorkflowSnapshot};
use crate::workflow::merge::{field_is_present, value_at_key};
use crate::workflow::types::{FieldBag, WorkflowDefinition, WorkflowStageKind};

pub fn snapshot_max_field_chars() -> usize {
    std::env::var("WORKFLOW_SNAPSHOT_MAX_FIELD_CHARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2048)
}

pub fn project_workflow_snapshot(
    definition: &WorkflowDefinition,
    instance: &WorkflowInstance,
) -> Option<WorkflowSnapshot> {
    let stage_def = definition.stage_by_id(&instance.stage)?;
    let max_chars = snapshot_max_field_chars();
    let missing = compute_missing_required(definition, instance, &instance.stage);

    let required_total = definition
        .fields
        .iter()
        .filter(|f| {
            f.required
                || f.required_from_stage
                    .as_deref()
                    .is_some_and(|s| s == instance.stage)
        })
        .count();
    let completeness_pct = if required_total == 0 {
        100
    } else {
        let present = required_total.saturating_sub(missing.len());
        ((present as f64 / required_total as f64) * 100.0).round() as u8
    };

    let inputs_summary = project_inputs_summary(definition, &instance.inputs, max_chars);
    let artifacts_summary = project_artifacts_summary(definition, &instance.artifacts, max_chars);

    let next_hitl_slug = match stage_def.kind {
        WorkflowStageKind::Collect | WorkflowStageKind::Review => stage_def.hitl_slug.clone(),
        _ => None,
    };

    let allowed_actions = allowed_actions_for_stage(stage_def.kind, stage_def.is_terminal);

    Some(WorkflowSnapshot {
        workflow_id: instance.workflow_id.clone(),
        workflow_type: instance.workflow_type.clone(),
        stage: instance.stage.clone(),
        stage_label: stage_def.label.clone(),
        stage_kind: stage_def.kind,
        status: instance.status,
        inputs_summary,
        artifacts_summary,
        missing_required: missing,
        validation_errors: instance.validation_errors.clone(),
        completeness_pct,
        next_hitl_slug,
        expected_outcome: stage_def.expected_outcome.clone(),
        allowed_actions,
    })
}

fn project_inputs_summary(
    definition: &WorkflowDefinition,
    inputs: &Value,
    max_chars: usize,
) -> Value {
    let mut summary = serde_json::Map::new();
    for field in definition
        .fields
        .iter()
        .filter(|f| f.bag == FieldBag::Inputs)
    {
        let val = value_at_key(inputs, &field.key);
        summary.insert(
            field.key.clone(),
            project_field_value(val, field.phi || field.redact_in_prompt, max_chars),
        );
    }
    Value::Object(summary)
}

fn project_artifacts_summary(
    definition: &WorkflowDefinition,
    artifacts: &Value,
    max_chars: usize,
) -> Value {
    let mut summary = serde_json::Map::new();
    for field in definition
        .fields
        .iter()
        .filter(|f| f.bag == FieldBag::Artifacts)
    {
        let val = value_at_key(artifacts, &field.key);
        summary.insert(field.key.clone(), project_artifact_value(val, max_chars));
    }
    Value::Object(summary)
}

fn project_field_value(value: Option<&Value>, redact: bool, max_chars: usize) -> Value {
    match value {
        None => Value::Null,
        Some(_v) if redact => json!("[redacted]"),
        Some(Value::String(s)) if s.len() > max_chars => {
            json!(format!("{}…", &s[..max_chars]))
        }
        Some(Value::Array(arr)) => json!(format!("[{} items]", arr.len())),
        Some(v) if !field_is_present(v) => Value::Null,
        Some(v) => v.clone(),
    }
}

fn project_artifact_value(value: Option<&Value>, max_chars: usize) -> Value {
    match value {
        None => Value::Null,
        Some(v) if !field_is_present(v) => Value::Null,
        Some(Value::Object(obj)) if obj.contains_key("ref") => {
            let mut out = serde_json::Map::new();
            if let Some(r) = obj.get("ref").and_then(|v| v.as_str()) {
                let truncated = if r.len() > max_chars {
                    format!("{}…", &r[..max_chars])
                } else {
                    r.to_string()
                };
                out.insert("ref".into(), json!(truncated));
            }
            if let Some(label) = obj.get("label") {
                out.insert("label".into(), label.clone());
            }
            Value::Object(out)
        }
        Some(_) => json!("present"),
    }
}

fn allowed_actions_for_stage(kind: WorkflowStageKind, is_terminal: bool) -> Vec<String> {
    if is_terminal {
        return vec![];
    }
    match kind {
        WorkflowStageKind::Collect => vec!["patch_inputs".into()],
        // Submit is listed for review stages but blocked until completeness_check passes (9.6).
        WorkflowStageKind::Review => vec!["patch_inputs".into(), "submit".into()],
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::instance::{WorkflowInstance, WorkflowStatus};
    use crate::workflow::loader::WorkflowDefinitionLoader;
    use crate::workflow::types::{WorkflowFieldDef, WorkflowFieldType};
    use chrono::Utc;
    use serde_json::json;
    use std::path::PathBuf;

    fn purchase_definition() -> WorkflowDefinition {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/purchase-requisition-split");
        WorkflowDefinitionLoader::load_from_dir(&dir).unwrap()
    }

    fn base_instance(def: &WorkflowDefinition) -> WorkflowInstance {
        WorkflowInstance {
            mongo_id: None,
            workflow_id: "wf-test".into(),
            definition_slug: def.slug.clone(),
            definition_version: def.version,
            workflow_type: def.workflow_type.clone(),
            user_id: "user-1".into(),
            session_id: None,
            assistant_id: None,
            stage: "collect_vendor".into(),
            status: WorkflowStatus::InProgress,
            inputs: json!({ "vendorId": "SECRET-123" }),
            artifacts: json!({}),
            validation_errors: vec![],
            missing_required: vec![],
            version: 1,
            external_refs: json!({}),
            audit: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn phi_fields_redacted_in_snapshot() {
        let mut def = purchase_definition();
        def.fields.push(WorkflowFieldDef {
            key: "ssn".into(),
            label: "SSN".into(),
            field_type: WorkflowFieldType::String,
            bag: FieldBag::Inputs,
            required: false,
            required_from_stage: None,
            validation: None,
            hitl_field_id: None,
            phi: true,
            redact_in_prompt: false,
        });

        let mut instance = base_instance(&def);
        instance.inputs = json!({ "vendorId": "V-1", "ssn": "123-45-6789" });

        let snap = project_workflow_snapshot(&def, &instance).unwrap();
        assert_eq!(snap.inputs_summary["ssn"], "[redacted]");
        assert_eq!(snap.inputs_summary["vendorId"], "V-1");
    }

    #[test]
    fn artifact_projection_presence_only() {
        let def = purchase_definition();
        let mut instance = base_instance(&def);
        instance.artifacts = json!({
            "prUrl": { "ref": "obj://pr-8f3a", "kind": "url", "label": "PR #42" }
        });

        let snap = project_workflow_snapshot(&def, &instance).unwrap();
        assert_eq!(snap.artifacts_summary["prUrl"]["ref"], "obj://pr-8f3a");
    }

    #[test]
    fn string_truncation_respects_max_chars() {
        let long = "x".repeat(3000);
        let projected = project_field_value(Some(&json!(long)), false, 100);
        let s = projected.as_str().unwrap();
        assert!(s.len() < 3000);
        assert!(s.ends_with('…'));
    }

    #[test]
    fn collect_stage_allowed_actions() {
        let def = purchase_definition();
        let instance = base_instance(&def);
        let snap = project_workflow_snapshot(&def, &instance).unwrap();
        assert_eq!(snap.allowed_actions, vec!["patch_inputs"]);
    }
}
