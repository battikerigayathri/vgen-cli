use chrono::Utc;
use serde_json::json;

use crate::workflow::completeness::completeness_check;
use crate::workflow::instance::{WorkflowAuditEntry, WorkflowInstance, WorkflowStatus};
use crate::workflow::merge::{field_is_present, value_at_key};
use crate::workflow::types::{
    StageDoneWhen, WorkflowDefinition, WorkflowStageDef, WorkflowStageKind,
};

/// Collect required field keys missing for `stage_id`.
pub fn compute_missing_required(
    definition: &WorkflowDefinition,
    instance: &WorkflowInstance,
    stage_id: &str,
) -> Vec<String> {
    let mut missing = Vec::new();
    for field in &definition.fields {
        let required = field.required
            || field
                .required_from_stage
                .as_deref()
                .is_some_and(|s| s == stage_id);
        if !required {
            continue;
        }
        let bag = match field.bag {
            crate::workflow::types::FieldBag::Inputs => &instance.inputs,
            crate::workflow::types::FieldBag::Artifacts => &instance.artifacts,
        };
        let present = value_at_key(bag, &field.key).is_some_and(field_is_present);
        if !present {
            missing.push(field.key.clone());
        }
    }
    missing.sort();
    missing.dedup();
    missing
}

/// Evaluate whether the current stage's `done_when` condition holds.
///
/// `ValidatorPass` stages require an explicit platform signal via
/// [`evaluate_done_when_with_validator`] — patch paths pass `validator_passed: false`.
pub fn evaluate_done_when(instance: &WorkflowInstance, stage_def: &WorkflowStageDef) -> bool {
    evaluate_done_when_with_validator(instance, stage_def, false)
}

/// Evaluate `done_when` with optional platform validator-pass signal (Phase 9.6).
pub fn evaluate_done_when_with_validator(
    instance: &WorkflowInstance,
    stage_def: &WorkflowStageDef,
    validator_passed: bool,
) -> bool {
    match &stage_def.done_when {
        StageDoneWhen::RequiredInputs(keys) => keys
            .iter()
            .all(|key| value_at_key(&instance.inputs, key).is_some_and(field_is_present)),
        StageDoneWhen::Terminal => stage_def.is_terminal,
        StageDoneWhen::ValidatorPass => validator_passed,
    }
}

/// Platform-owned auto-advance when `done_when` is satisfied (at most one hop per call).
pub fn maybe_auto_advance(
    instance: &mut WorkflowInstance,
    definition: &WorkflowDefinition,
    source: &str,
) -> Option<WorkflowAuditEntry> {
    maybe_auto_advance_with_validator(instance, definition, source, false)
}

/// Auto-advance with optional validator-pass signal for `done_when: validator_pass` stages.
pub fn maybe_auto_advance_with_validator(
    instance: &mut WorkflowInstance,
    definition: &WorkflowDefinition,
    source: &str,
    validator_passed: bool,
) -> Option<WorkflowAuditEntry> {
    let stage_def = definition.stage_by_id(&instance.stage)?;
    if !evaluate_done_when_with_validator(instance, stage_def, validator_passed) {
        return None;
    }
    let next_id = stage_def.next.as_ref()?;
    let next_def = definition.stage_by_id(next_id)?;

    if next_def.is_terminal || stage_def.kind == WorkflowStageKind::Review {
        let check = completeness_check(
            definition,
            instance,
            Some(&instance.stage),
            &instance.validation_errors,
        );
        instance.missing_required = check.missing_required.clone();
        if !check.complete {
            return None;
        }
    }

    let from = instance.stage.clone();
    instance.stage = next_id.clone();
    if next_def.is_terminal {
        instance.status = WorkflowStatus::Submitted;
    } else {
        instance.status = WorkflowStatus::InProgress;
    }
    Some(WorkflowAuditEntry {
        timestamp: Utc::now(),
        source: source.to_string(),
        action: "stage_advance".into(),
        details: Some(json!({ "from": from, "to": next_id })),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::instance::WorkflowInstance;
    use crate::workflow::loader::WorkflowDefinitionLoader;
    use crate::workflow::types::{StageDoneWhen, WorkflowStageDef, WorkflowStageKind};
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
    fn required_inputs_done_when_satisfied() {
        let def = purchase_definition();
        let stage = def.stage_by_id("collect_vendor").unwrap().clone();
        let mut instance = base_instance(&def);
        assert!(!evaluate_done_when(&instance, &stage));

        instance.inputs = json!({ "vendorId": "V-1" });
        assert!(evaluate_done_when(&instance, &stage));
    }

    #[test]
    fn auto_advance_moves_to_next_collect_stage() {
        let def = purchase_definition();
        let mut instance = base_instance(&def);
        instance.inputs = json!({ "vendorId": "V-1" });

        let audit = maybe_auto_advance(&mut instance, &def, "test").expect("should advance");
        assert_eq!(audit.action, "stage_advance");
        assert_eq!(instance.stage, "collect_line_items");
        assert_eq!(instance.status, WorkflowStatus::InProgress);
    }

    #[test]
    fn auto_advance_moves_to_review_after_line_items() {
        let def = purchase_definition();
        let mut instance = base_instance(&def);
        instance.stage = "collect_line_items".into();
        instance.inputs = json!({ "vendorId": "V-1", "lineItems": [{ "sku": "A" }] });

        let _ = maybe_auto_advance(&mut instance, &def, "test").expect("advance to review");
        assert_eq!(instance.stage, "review_summary");
        assert_eq!(instance.status, WorkflowStatus::InProgress);
    }

    #[test]
    fn auto_advance_terminal_sets_submitted_from_review() {
        let def = purchase_definition();
        let mut instance = base_instance(&def);
        instance.stage = "review_summary".into();
        instance.inputs = json!({
            "vendorId": "V-1",
            "lineItems": [{ "sku": "A" }],
            "reviewConfirmed": true
        });

        let _ = maybe_auto_advance(&mut instance, &def, "test").expect("advance to submit");
        assert_eq!(instance.stage, "submit");
        assert_eq!(instance.status, WorkflowStatus::Submitted);
    }

    #[test]
    fn unsatisfied_done_when_no_op() {
        let def = purchase_definition();
        let mut instance = base_instance(&def);
        assert!(maybe_auto_advance(&mut instance, &def, "test").is_none());
        assert_eq!(instance.stage, "collect_vendor");
    }

    #[test]
    fn validator_pass_false_without_platform_signal() {
        let stage = WorkflowStageDef {
            id: "agent".into(),
            label: "Agent".into(),
            kind: WorkflowStageKind::AgentTask,
            hitl_slug: None,
            agent_slug: Some("agent".into()),
            done_when: StageDoneWhen::ValidatorPass,
            expected_outcome: None,
            next: Some("done".into()),
            back_to: None,
            is_terminal: false,
        };
        let def = purchase_definition();
        let instance = base_instance(&def);
        assert!(!evaluate_done_when(&instance, &stage));
        assert!(!evaluate_done_when_with_validator(&instance, &stage, false));
    }

    #[test]
    fn validator_pass_true_with_platform_signal() {
        let stage = WorkflowStageDef {
            id: "agent".into(),
            label: "Agent".into(),
            kind: WorkflowStageKind::AgentTask,
            hitl_slug: None,
            agent_slug: Some("agent".into()),
            done_when: StageDoneWhen::ValidatorPass,
            expected_outcome: None,
            next: Some("done".into()),
            back_to: None,
            is_terminal: false,
        };
        let def = purchase_definition();
        let instance = base_instance(&def);
        assert!(evaluate_done_when_with_validator(&instance, &stage, true));
    }

    #[test]
    fn terminal_advance_blocked_when_incomplete() {
        let def = purchase_definition();
        let mut instance = base_instance(&def);
        instance.stage = "review_summary".into();
        instance.inputs = json!({
            "lineItems": [{ "sku": "A" }],
            "reviewConfirmed": true
        });

        assert!(maybe_auto_advance(&mut instance, &def, "test").is_none());
        assert_eq!(instance.stage, "review_summary");
        assert!(instance.missing_required.contains(&"vendorId".to_string()));
    }

    fn github_pr_definition() -> WorkflowDefinition {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/github-pr-create-bundle");
        WorkflowDefinitionLoader::load_from_dir(&dir).unwrap()
    }

    #[test]
    fn validator_pass_advance_moves_to_submitted_when_complete() {
        let def = github_pr_definition();
        let mut instance = WorkflowInstance {
            mongo_id: None,
            workflow_id: "wf-pr".into(),
            definition_slug: def.slug.clone(),
            definition_version: def.version,
            workflow_type: def.workflow_type.clone(),
            user_id: "user-1".into(),
            session_id: None,
            assistant_id: None,
            stage: "generate_and_open_pr".into(),
            status: WorkflowStatus::InProgress,
            inputs: json!({
                "repoName": "org/repo",
                "featureDescription": "Add feature"
            }),
            artifacts: json!({ "prUrl": "https://github.com/org/repo/pull/42" }),
            validation_errors: vec![],
            missing_required: vec![],
            version: 2,
            external_refs: json!({}),
            audit: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let audit = maybe_auto_advance_with_validator(&mut instance, &def, "platform", true)
            .expect("advance");
        assert_eq!(audit.action, "stage_advance");
        assert_eq!(instance.stage, "submitted");
        assert_eq!(instance.status, WorkflowStatus::Submitted);
    }

    #[test]
    fn patch_path_does_not_advance_validator_pass_stage() {
        let def = github_pr_definition();
        let mut instance = WorkflowInstance {
            mongo_id: None,
            workflow_id: "wf-pr".into(),
            definition_slug: def.slug.clone(),
            definition_version: def.version,
            workflow_type: def.workflow_type.clone(),
            user_id: "user-1".into(),
            session_id: None,
            assistant_id: None,
            stage: "generate_and_open_pr".into(),
            status: WorkflowStatus::InProgress,
            inputs: json!({
                "repoName": "org/repo",
                "featureDescription": "Add feature"
            }),
            artifacts: json!({ "prUrl": "https://github.com/org/repo/pull/42" }),
            validation_errors: vec![],
            missing_required: vec![],
            version: 2,
            external_refs: json!({}),
            audit: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(maybe_auto_advance(&mut instance, &def, "patch").is_none());
        assert_eq!(instance.stage, "generate_and_open_pr");
    }

    #[test]
    fn compute_missing_required_for_collect_vendor() {
        let def = purchase_definition();
        let instance = base_instance(&def);
        let missing = compute_missing_required(&def, &instance, "collect_vendor");
        assert!(missing.contains(&"vendorId".to_string()));
    }
}
