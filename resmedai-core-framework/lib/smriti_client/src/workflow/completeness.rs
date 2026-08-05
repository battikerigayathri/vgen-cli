use crate::workflow::advance::compute_missing_required;
use crate::workflow::instance::{CompletenessResult, FieldError, WorkflowInstance};
use crate::workflow::merge::validate_and_merge_patch;
use crate::workflow::types::WorkflowDefinition;

/// Read-only completeness evaluation (optionally for a different stage).
pub fn completeness_check(
    definition: &WorkflowDefinition,
    instance: &WorkflowInstance,
    for_stage: Option<&str>,
    validation_errors: &[FieldError],
) -> CompletenessResult {
    let stage_id = for_stage.unwrap_or(&instance.stage);
    let missing = compute_missing_required(definition, instance, stage_id);
    let (pct, complete) = completeness_pct(definition, instance, stage_id, &missing);

    CompletenessResult {
        complete: complete && validation_errors.is_empty(),
        completeness_pct: pct,
        missing_required: missing,
        validation_errors: validation_errors.to_vec(),
    }
}

/// Evaluate completeness after applying a hypothetical patch (no persist).
#[allow(dead_code)]
pub fn completeness_check_with_patch(
    definition: &WorkflowDefinition,
    instance: &WorkflowInstance,
    for_stage: Option<&str>,
    patch_inputs: Option<&serde_json::Value>,
    patch_artifacts: Option<&serde_json::Value>,
    patch_external_refs: Option<&serde_json::Value>,
) -> crate::error::Result<CompletenessResult> {
    let (inputs, artifacts, external_refs, validation_errors) = validate_and_merge_patch(
        definition,
        &instance.inputs,
        &instance.artifacts,
        &instance.external_refs,
        patch_inputs,
        patch_artifacts,
        patch_external_refs,
    )?;

    let mut preview = instance.clone();
    preview.inputs = inputs;
    preview.artifacts = artifacts;
    preview.external_refs = external_refs;

    Ok(completeness_check(
        definition,
        &preview,
        for_stage,
        &validation_errors,
    ))
}

fn completeness_pct(
    definition: &WorkflowDefinition,
    _instance: &WorkflowInstance,
    stage_id: &str,
    missing: &[String],
) -> (u8, bool) {
    let required_keys: Vec<&str> = definition
        .fields
        .iter()
        .filter(|f| {
            f.required
                || f.required_from_stage
                    .as_deref()
                    .is_some_and(|s| s == stage_id)
        })
        .map(|f| f.key.as_str())
        .collect();

    if required_keys.is_empty() {
        return (100, true);
    }

    let present = required_keys.len().saturating_sub(missing.len());
    let pct = ((present as f64 / required_keys.len() as f64) * 100.0).round() as u8;
    (pct, missing.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::instance::{WorkflowInstance, WorkflowStatus};
    use crate::workflow::loader::WorkflowDefinitionLoader;
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
            inputs: json!({}),
            artifacts: json!({}),
            validation_errors: vec![],
            missing_required: vec!["vendorId".into()],
            version: 1,
            external_refs: json!({}),
            audit: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn completeness_pct_zero_when_all_missing() {
        let def = purchase_definition();
        let instance = base_instance(&def);
        let result = completeness_check(&def, &instance, None, &[]);
        assert!(!result.complete);
        assert!(result.completeness_pct < 100);
        assert!(result.missing_required.contains(&"vendorId".to_string()));
    }

    #[test]
    fn for_stage_override_uses_different_requirements() {
        let def = purchase_definition();
        let mut instance = base_instance(&def);
        instance.inputs = json!({ "vendorId": "V-1" });

        let vendor_stage = completeness_check(&def, &instance, Some("collect_vendor"), &[]);
        assert!(
            !vendor_stage
                .missing_required
                .contains(&"vendorId".to_string())
        );
        assert!(
            vendor_stage
                .missing_required
                .contains(&"lineItems".to_string())
        );

        let lines_stage = completeness_check(&def, &instance, Some("collect_line_items"), &[]);
        assert!(
            lines_stage
                .missing_required
                .contains(&"lineItems".to_string())
        );
    }
}
