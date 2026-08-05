use serde_json::Value;

use crate::error::{Result, SmritiError};
use crate::workflow::instance::FieldError;
use crate::workflow::types::{FieldBag, WorkflowDefinition, WorkflowFieldType};

const MAX_INLINE_ARTIFACT_BYTES: usize = 512;

/// Deep-merge top-level keys in `base` with `patch` (objects recurse one level deep).
pub fn deep_merge_object(base: &Value, patch: &Value) -> Value {
    match (base, patch) {
        (_, Value::Null) => base.clone(),
        (Value::Object(base_map), Value::Object(patch_map)) => {
            let mut merged = base_map.clone();
            for (key, patch_val) in patch_map {
                if let Some(base_val) = merged.get(key) {
                    merged.insert(key.clone(), deep_merge_object(base_val, patch_val));
                } else {
                    merged.insert(key.clone(), patch_val.clone());
                }
            }
            Value::Object(merged)
        }
        (_, patch_val) => patch_val.clone(),
    }
}

/// Validate patch keys against definition fields and return merged bags + errors.
pub fn validate_and_merge_patch(
    definition: &WorkflowDefinition,
    existing_inputs: &Value,
    existing_artifacts: &Value,
    existing_external_refs: &Value,
    patch_inputs: Option<&Value>,
    patch_artifacts: Option<&Value>,
    patch_external_refs: Option<&Value>,
) -> Result<(Value, Value, Value, Vec<FieldError>)> {
    let mut errors = Vec::new();

    let merged_inputs = if let Some(p) = patch_inputs {
        validate_bag_keys(definition, FieldBag::Inputs, p, &mut errors)?;
        deep_merge_object(existing_inputs, p)
    } else {
        existing_inputs.clone()
    };

    let merged_artifacts = if let Some(p) = patch_artifacts {
        validate_bag_keys(definition, FieldBag::Artifacts, p, &mut errors)?;
        validate_artifact_refs(p, &mut errors);
        deep_merge_object(existing_artifacts, p)
    } else {
        existing_artifacts.clone()
    };

    let merged_external_refs = if let Some(p) = patch_external_refs {
        deep_merge_object(existing_external_refs, p)
    } else {
        existing_external_refs.clone()
    };

    Ok((
        merged_inputs,
        merged_artifacts,
        merged_external_refs,
        errors,
    ))
}

fn validate_bag_keys(
    definition: &WorkflowDefinition,
    bag: FieldBag,
    patch: &Value,
    errors: &mut Vec<FieldError>,
) -> Result<()> {
    let Some(obj) = patch.as_object() else {
        return Err(SmritiError::InvalidResponse(format!(
            "patch bag must be an object, got {patch}"
        )));
    };

    for key in obj.keys() {
        let Some(field) = definition.fields.iter().find(|f| f.key == *key) else {
            errors.push(FieldError {
                field: key.clone(),
                message: "unknown field key for workflow definition".into(),
            });
            continue;
        };
        if field.bag != bag {
            errors.push(FieldError {
                field: key.clone(),
                message: format!("field belongs to {:?} bag, not patch target", field.bag),
            });
            continue;
        }
        if let Some(val) = obj.get(key)
            && let Err(msg) = validate_field_type(field.field_type, val)
        {
            errors.push(FieldError {
                field: key.clone(),
                message: msg,
            });
        }
    }
    Ok(())
}

fn validate_field_type(
    expected: WorkflowFieldType,
    value: &Value,
) -> std::result::Result<(), String> {
    match expected {
        WorkflowFieldType::String => {
            if value.is_string() {
                Ok(())
            } else {
                Err("expected string".into())
            }
        }
        WorkflowFieldType::Number => {
            if value.is_number() {
                Ok(())
            } else {
                Err("expected number".into())
            }
        }
        WorkflowFieldType::Boolean => {
            if value.is_boolean() {
                Ok(())
            } else {
                Err("expected boolean".into())
            }
        }
        WorkflowFieldType::Array => {
            if value.is_array() {
                Ok(())
            } else {
                Err("expected array".into())
            }
        }
        WorkflowFieldType::Object => {
            if value.is_object() {
                Ok(())
            } else {
                Err("expected object".into())
            }
        }
    }
}

fn validate_artifact_refs(patch: &Value, errors: &mut Vec<FieldError>) {
    let Some(obj) = patch.as_object() else {
        return;
    };
    for (key, val) in obj {
        let serialized = serde_json::to_string(val).unwrap_or_default();
        if serialized.len() > MAX_INLINE_ARTIFACT_BYTES {
            errors.push(FieldError {
                field: key.clone(),
                message: format!(
                    "artifact inline value exceeds {MAX_INLINE_ARTIFACT_BYTES} bytes; use object-store ref"
                ),
            });
        }
    }
}

pub fn field_is_present(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(m) => !m.is_empty(),
        _ => true,
    }
}

pub fn value_at_key<'a>(obj: &'a Value, key: &str) -> Option<&'a Value> {
    obj.as_object()?.get(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::types::{WorkflowFieldDef, WorkflowFieldType};
    use serde_json::json;

    fn sample_definition() -> WorkflowDefinition {
        WorkflowDefinition {
            id: None,
            slug: "test".into(),
            name: "Test".into(),
            workflow_type: "test".into(),
            version: 1,
            description: None,
            fields: vec![
                WorkflowFieldDef {
                    key: "vendorId".into(),
                    label: "Vendor".into(),
                    field_type: WorkflowFieldType::String,
                    bag: FieldBag::Inputs,
                    required: true,
                    required_from_stage: None,
                    validation: None,
                    hitl_field_id: None,
                    phi: false,
                    redact_in_prompt: false,
                },
                WorkflowFieldDef {
                    key: "prUrl".into(),
                    label: "PR".into(),
                    field_type: WorkflowFieldType::Object,
                    bag: FieldBag::Artifacts,
                    required: false,
                    required_from_stage: None,
                    validation: None,
                    hitl_field_id: None,
                    phi: false,
                    redact_in_prompt: false,
                },
            ],
            stages: vec![],
            gates: vec![],
            initial_stage: "start".into(),
            source_format: crate::workflow::types::SourceFormat::SplitYaml,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn deep_merge_overwrites_scalar_and_nests_objects() {
        let base = json!({ "a": 1, "nested": { "x": 1, "y": 2 } });
        let patch = json!({ "a": 2, "nested": { "y": 3, "z": 4 } });
        let merged = deep_merge_object(&base, &patch);
        assert_eq!(merged["a"], 2);
        assert_eq!(merged["nested"]["x"], 1);
        assert_eq!(merged["nested"]["y"], 3);
        assert_eq!(merged["nested"]["z"], 4);
    }

    #[test]
    fn validate_patch_rejects_unknown_field() {
        let def = sample_definition();
        let (_, _, _, errors) = validate_and_merge_patch(
            &def,
            &json!({}),
            &json!({}),
            &json!({}),
            Some(&json!({ "unknown": "x" })),
            None,
            None,
        )
        .unwrap();
        assert!(errors.iter().any(|e| e.field == "unknown"));
    }

    #[test]
    fn validate_patch_accepts_artifact_ref_shape() {
        let def = sample_definition();
        let artifact = json!({ "ref": "obj://pr-1", "kind": "url", "label": "PR" });
        let (_, merged_artifacts, _, errors) = validate_and_merge_patch(
            &def,
            &json!({}),
            &json!({}),
            &json!({}),
            None,
            Some(&json!({ "prUrl": artifact })),
            None,
        )
        .unwrap();
        assert!(errors.is_empty());
        assert_eq!(merged_artifacts["prUrl"]["ref"], "obj://pr-1");
    }

    #[test]
    fn field_is_present_handles_empty_values() {
        assert!(!field_is_present(&json!(null)));
        assert!(!field_is_present(&json!("")));
        assert!(!field_is_present(&json!([])));
        assert!(field_is_present(&json!("V-1")));
    }
}
