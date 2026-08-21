use serde::Serialize;
use smriti_client::{SmritiError, SourceFormat, WorkflowDefinitionLoader};
use std::path::Path;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WorkflowValidateData {
    pub slug: String,
    pub version: u32,
    pub source_format: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hints: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowValidateError {
    pub code: &'static str,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl std::fmt::Display for WorkflowValidateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

pub fn validate_workflow_dir(dir: &Path) -> Result<WorkflowValidateData, WorkflowValidateError> {
    if !dir.is_dir() {
        return Err(WorkflowValidateError {
            code: "WORKFLOW_LAYOUT_INVALID",
            message: format!("workflow directory not found: {}", dir.display()),
            details: Some(serde_json::json!({ "path": dir.display().to_string() })),
        });
    }

    match WorkflowDefinitionLoader::load_from_dir(dir) {
        Ok(def) => Ok(WorkflowValidateData {
            slug: def.slug,
            version: def.version,
            source_format: source_format_str(def.source_format).to_string(),
            path: dir.display().to_string(),
            hints: collect_required_from_stage_conflict_hints(&def.fields),
        }),
        Err(err) => Err(map_smriti_error(err)),
    }
}

fn source_format_str(fmt: SourceFormat) -> &'static str {
    match fmt {
        SourceFormat::SplitYaml => "split_yaml",
        SourceFormat::BundleYaml => "bundle_yaml",
        SourceFormat::BundleJson => "bundle_json",
    }
}

fn map_smriti_error(err: SmritiError) -> WorkflowValidateError {
    match err {
        SmritiError::SchemaValidation(msg) => WorkflowValidateError {
            code: "WORKFLOW_SCHEMA_INVALID",
            message: format!("JSON Schema validation failed: {msg}"),
            details: None,
        },
        SmritiError::SemanticValidation { path, message } => WorkflowValidateError {
            code: "WORKFLOW_SEMANTIC_INVALID",
            message: semantic_validation_message(&path, &message),
            details: Some(serde_json::json!({ "path": path, "message": message })),
        },
        SmritiError::InvalidAuthorLayout { path, reason } => WorkflowValidateError {
            code: "WORKFLOW_LAYOUT_INVALID",
            message: format!("invalid author layout at {path}: {reason}"),
            details: Some(serde_json::json!({ "path": path, "reason": reason })),
        },
        SmritiError::YamlParse(msg) => WorkflowValidateError {
            code: "WORKFLOW_LAYOUT_INVALID",
            message: format!("YAML parse error: {msg}"),
            details: None,
        },
        SmritiError::JsonParse(msg) => WorkflowValidateError {
            code: "WORKFLOW_LAYOUT_INVALID",
            message: format!("JSON parse error: {msg}"),
            details: None,
        },
        SmritiError::Io { path, source } => WorkflowValidateError {
            code: "WORKFLOW_LAYOUT_INVALID",
            message: format!("IO error at {path}: {source}"),
            details: Some(serde_json::json!({ "path": path })),
        },
        SmritiError::InvalidResponse(msg) => WorkflowValidateError {
            code: "WORKFLOW_VALIDATION",
            message: msg,
            details: None,
        },
        other => WorkflowValidateError {
            code: "WORKFLOW_VALIDATION",
            message: other.to_string(),
            details: None,
        },
    }
}

#[cfg(test)]
mod phase1_transition_tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/workflows")
    }

    fn write_minimal_fixture(dir: &Path, flow_yaml: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("meta.yaml"),
            "name: Phase 1 Fixture\nslug: phase1-fixture-v1\nworkflowType: phase1_fixture\nversion: 1\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("schema.yaml"),
            "fields:\n  - key: flag\n    label: Flag\n    type: boolean\n    bag: inputs\n",
        )
        .unwrap();
        std::fs::write(dir.join("flow.yaml"), flow_yaml).unwrap();
    }

    #[test]
    fn accepts_conditional_branch_v1_fixture() {
        let dir = fixtures_dir().join("conditional-branch-v1");
        let data = validate_workflow_dir(&dir).expect("conditional-branch-v1 should validate");
        assert_eq!(data.slug, "conditional-branch-v1");
        assert_eq!(data.source_format, "split_yaml");
    }

    #[test]
    fn rejects_dead_end_invalid_fixture() {
        let dir = fixtures_dir().join("dead-end-invalid");
        let err = validate_workflow_dir(&dir).expect_err("dead-end-invalid should fail");
        assert_eq!(err.code, "WORKFLOW_SEMANTIC_INVALID");
        assert!(
            err.message.contains("flow.stages[collect].transitions"),
            "expected transitions path in message, got: {}",
            err.message
        );
        assert!(
            err.message.contains("dead end"),
            "expected dead end mention, got: {}",
            err.message
        );
        assert!(
            err.message.contains("Hint:"),
            "expected remediation hint, got: {}",
            err.message
        );
    }

    #[test]
    fn rejects_dangling_transition_target() {
        let dir =
            std::env::temp_dir().join(format!("vgen-phase1-dangling-{}", uuid::Uuid::new_v4()));
        write_minimal_fixture(
            &dir,
            r#"initialStage: collect
stages:
  - id: collect
    label: Collect
    kind: collect
    hitlSlug: phase1-form
    doneWhen: [flag]
    transitions:
      - target: nonexistent_stage
        when:
          present: inputs.flag
    next: done
  - id: done
    label: Done
    kind: terminal
    terminal: true
    doneWhen: terminal
"#,
        );

        let err = validate_workflow_dir(&dir).expect_err("dangling target must fail");
        assert_eq!(err.code, "WORKFLOW_SEMANTIC_INVALID");
        assert!(
            err.message
                .contains("flow.stages[collect].transitions[0].target"),
            "expected target path in message, got: {}",
            err.message
        );
        assert!(
            err.message.contains("nonexistent_stage"),
            "expected missing stage id in message, got: {}",
            err.message
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_invalid_when_tree() {
        let dir = std::env::temp_dir().join(format!(
            "vgen-phase1-invalid-when-{}",
            uuid::Uuid::new_v4()
        ));
        write_minimal_fixture(
            &dir,
            r#"initialStage: collect
stages:
  - id: collect
    label: Collect
    kind: collect
    hitlSlug: phase1-form
    doneWhen: [flag]
    transitions:
      - target: done
        when:
          gate: does_not_exist
    next: done
  - id: done
    label: Done
    kind: terminal
    terminal: true
    doneWhen: terminal
"#,
        );

        let err = validate_workflow_dir(&dir).expect_err("invalid when tree must fail");
        assert_eq!(err.code, "WORKFLOW_SEMANTIC_INVALID");
        assert!(
            err.message
                .contains("flow.stages[collect].transitions[0].when"),
            "expected when path in message, got: {}",
            err.message
        );
        assert!(
            err.message.contains("does_not_exist"),
            "expected gate id in message, got: {}",
            err.message
        );
        assert!(
            err.message.contains("Hint:"),
            "expected unknown gate remediation hint, got: {}",
            err.message
        );
        assert!(
            err.message.contains("§2.4.5"),
            "expected spec §2.4.5 reference in hint, got: {}",
            err.message
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod phase2_path_completeness_tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/workflows")
    }

    fn write_minimal_fixture(dir: &Path, schema_yaml: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("meta.yaml"),
            "name: Phase 2 Fixture\nslug: phase2-fixture-v1\nworkflowType: phase2_fixture\nversion: 1\n",
        )
        .unwrap();
        std::fs::write(dir.join("schema.yaml"), schema_yaml).unwrap();
        std::fs::write(
            dir.join("flow.yaml"),
            r#"initialStage: collect
stages:
  - id: collect
    label: Collect
    kind: collect
    hitlSlug: phase2-form
    doneWhen: [flag]
    next: done
  - id: done
    label: Done
    kind: terminal
    terminal: true
    doneWhen: terminal
"#,
        )
        .unwrap();
    }

    #[test]
    fn accepts_conditional_branch_v1_with_required_from_stage_fields() {
        let dir = fixtures_dir().join("conditional-branch-v1");
        let data = validate_workflow_dir(&dir)
            .expect("conditional-branch-v1 with path-aware schema should validate");
        assert_eq!(data.slug, "conditional-branch-v1");
        assert_eq!(data.source_format, "split_yaml");
    }

    #[test]
    fn rejects_required_from_stage_invalid_fixture() {
        let dir = fixtures_dir().join("required-from-stage-invalid");
        let err = validate_workflow_dir(&dir).expect_err("required-from-stage-invalid should fail");
        assert_eq!(err.code, "WORKFLOW_SEMANTIC_INVALID");
        assert!(
            err.message
                .contains("schema.fields[orphanField].requiredFromStage"),
            "expected requiredFromStage path in message, got: {}",
            err.message
        );
        assert!(
            err.message.contains("ghost_stage"),
            "expected missing stage id in message, got: {}",
            err.message
        );
        assert!(
            err.message.contains("Hint:"),
            "expected requiredFromStage remediation hint, got: {}",
            err.message
        );
        assert!(
            err.message.contains("flow.yaml"),
            "expected flow.yaml mention in hint, got: {}",
            err.message
        );
    }

    #[test]
    fn rejects_dangling_required_from_stage_ref() {
        let dir = std::env::temp_dir().join(format!(
            "vgen-phase2-dangling-required-from-stage-{}",
            uuid::Uuid::new_v4()
        ));
        write_minimal_fixture(
            &dir,
            "fields:\n  - key: flag\n    label: Flag\n    type: boolean\n    bag: inputs\n  - key: expressReason\n    label: Express reason\n    type: string\n    bag: inputs\n    requiredFromStage: nonexistent_stage\n",
        );

        let err = validate_workflow_dir(&dir).expect_err("dangling requiredFromStage must fail");
        assert_eq!(err.code, "WORKFLOW_SEMANTIC_INVALID");
        assert!(
            err.message
                .contains("schema.fields[expressReason].requiredFromStage"),
            "expected requiredFromStage path in message, got: {}",
            err.message
        );
        assert!(
            err.message.contains("nonexistent_stage"),
            "expected missing stage id in message, got: {}",
            err.message
        );
        assert!(
            err.message.contains("Hint:"),
            "expected requiredFromStage remediation hint, got: {}",
            err.message
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn emits_conflict_hint_when_required_and_required_from_stage_both_set() {
        let dir = std::env::temp_dir().join(format!(
            "vgen-phase2-required-conflict-{}",
            uuid::Uuid::new_v4()
        ));
        write_minimal_fixture(
            &dir,
            "fields:\n  - key: flag\n    label: Flag\n    type: boolean\n    bag: inputs\n  - key: vendorId\n    label: Vendor ID\n    type: string\n    bag: inputs\n    required: true\n    requiredFromStage: collect\n",
        );

        let data = validate_workflow_dir(&dir)
            .expect("workflow with required+requiredFromStage overlap should still validate");
        assert!(
            data.hints.iter().any(|hint| {
                hint.contains("schema.fields[vendorId]")
                    && hint.contains("required wins")
                    && hint.contains("requiredFromStage is a no-op")
            }),
            "expected required+requiredFromStage conflict hint, got: {:?}",
            data.hints
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod phase3_reset_tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/workflows")
    }

    fn write_minimal_fixture(dir: &Path, flow_yaml: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("meta.yaml"),
            "name: Phase 3 Fixture\nslug: phase3-fixture-v1\nworkflowType: phase3_fixture\nversion: 1\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("schema.yaml"),
            "fields:\n  - key: reviewConfirmed\n    label: Review confirmed\n    type: boolean\n    bag: inputs\n  - key: lineItems\n    label: Line items\n    type: array\n    bag: inputs\n",
        )
        .unwrap();
        std::fs::write(dir.join("flow.yaml"), flow_yaml).unwrap();
    }

    #[test]
    fn accepts_rework_loop_v1_fixture() {
        let dir = fixtures_dir().join("rework-loop-v1");
        let data = validate_workflow_dir(&dir).expect("rework-loop-v1 should validate");
        assert_eq!(data.slug, "rework-loop-v1");
        assert_eq!(data.source_format, "split_yaml");
    }

    #[test]
    fn rejects_reset_field_invalid_fixture() {
        let dir = fixtures_dir().join("reset-field-invalid");
        let err = validate_workflow_dir(&dir).expect_err("reset-field-invalid should fail");
        assert_eq!(err.code, "WORKFLOW_SEMANTIC_INVALID");
        assert!(
            err.message
                .contains("flow.stages[review_summary].transitions[1].reset[0]"),
            "expected reset path in message, got: {}",
            err.message
        );
        assert!(
            err.message.contains("notARealField"),
            "expected unknown field name in message, got: {}",
            err.message
        );
    }

    #[test]
    fn rejects_unknown_reset_key_in_temp_dir() {
        let dir = std::env::temp_dir().join(format!(
            "vgen-phase3-unknown-reset-{}",
            uuid::Uuid::new_v4()
        ));
        write_minimal_fixture(
            &dir,
            r#"initialStage: collect_line_items
stages:
  - id: collect_line_items
    label: Collect line items
    kind: collect
    hitlSlug: phase3-form
    doneWhen: [lineItems]
    next: review_summary
  - id: review_summary
    label: Review summary
    kind: review
    hitlSlug: phase3-review-form
    doneWhen: [reviewConfirmed]
    transitions:
      - target: submit
        when:
          eq:
            field: inputs.reviewConfirmed
            value: true
      - target: collect_line_items
        when:
          eq:
            field: inputs.reviewConfirmed
            value: false
        reset:
          - ghostFieldKey
    next: submit
  - id: submit
    label: Submitted
    kind: terminal
    terminal: true
    doneWhen: terminal
"#,
        );

        let err = validate_workflow_dir(&dir).expect_err("unknown reset key must fail");
        assert_eq!(err.code, "WORKFLOW_SEMANTIC_INVALID");
        assert!(
            err.message
                .contains("flow.stages[review_summary].transitions[1].reset[0]"),
            "expected reset path in message, got: {}",
            err.message
        );
        assert!(
            err.message.contains("ghostFieldKey"),
            "expected unknown field name in message, got: {}",
            err.message
        );
        assert!(
            err.message.contains("Hint:"),
            "expected reset remediation hint, got: {}",
            err.message
        );
        assert!(
            err.message.contains("§2.4.3"),
            "expected spec §2.4.3 reference in hint, got: {}",
            err.message
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod phase4_named_gates_tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/workflows")
    }

    fn write_minimal_fixture(dir: &Path, flow_yaml: &str, gates_yaml: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("meta.yaml"),
            "name: Phase 4 Fixture\nslug: phase4-fixture-v1\nworkflowType: phase4_fixture\nversion: 1\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("schema.yaml"),
            "fields:\n  - key: totalAmount\n    label: Total amount\n    type: number\n    bag: inputs\n",
        )
        .unwrap();
        std::fs::write(dir.join("flow.yaml"), flow_yaml).unwrap();
        std::fs::write(dir.join("gates.yaml"), gates_yaml).unwrap();
    }

    #[test]
    fn accepts_named_gates_v1_fixture() {
        let dir = fixtures_dir().join("named-gates-v1");
        let data = validate_workflow_dir(&dir).expect("named-gates-v1 should validate");
        assert_eq!(data.slug, "named-gates-v1");
        assert_eq!(data.source_format, "split_yaml");
    }

    #[test]
    fn rejects_gate_cycle_invalid_fixture() {
        let dir = fixtures_dir().join("gate-cycle-invalid");
        let err = validate_workflow_dir(&dir).expect_err("gate-cycle-invalid should fail");
        assert_eq!(err.code, "WORKFLOW_SEMANTIC_INVALID");
        assert!(
            err.message.contains("gates"),
            "expected gates path in message, got: {}",
            err.message
        );
        assert!(
            err.message.contains("cycle"),
            "expected cycle mention in message, got: {}",
            err.message
        );
        assert!(
            err.message.contains("gate_a") && err.message.contains("gate_b"),
            "expected human-readable cycle path in message, got: {}",
            err.message
        );
        assert!(
            err.message.contains("Hint:"),
            "expected gate cycle remediation hint, got: {}",
            err.message
        );
        assert!(
            err.message.contains("§2.4.5"),
            "expected spec §2.4.5 reference in hint, got: {}",
            err.message
        );
    }

    #[test]
    fn rejects_unknown_gate_ref_invalid_fixture() {
        let dir = fixtures_dir().join("unknown-gate-ref-invalid");
        let err = validate_workflow_dir(&dir).expect_err("unknown-gate-ref-invalid should fail");
        assert_eq!(err.code, "WORKFLOW_SEMANTIC_INVALID");
        assert!(
            err.message.contains("gates[0].when"),
            "expected gates[0].when path in message, got: {}",
            err.message
        );
        assert!(
            err.message.contains("does_not_exist"),
            "expected undefined gate id in message, got: {}",
            err.message
        );
        assert!(
            err.message.contains("Hint:"),
            "expected unknown gate remediation hint, got: {}",
            err.message
        );
        assert!(
            err.message.contains("gates.yaml"),
            "expected gates.yaml mention in hint, got: {}",
            err.message
        );
    }

    #[test]
    fn rejects_duplicate_gate_ids_in_temp_dir() {
        let dir = std::env::temp_dir().join(format!(
            "vgen-phase4-duplicate-gate-{}",
            uuid::Uuid::new_v4()
        ));
        write_minimal_fixture(
            &dir,
            r#"initialStage: collect
stages:
  - id: collect
    label: Collect
    kind: collect
    hitlSlug: phase4-form
    doneWhen: [totalAmount]
    transitions:
      - target: done
        when:
          gate: is_high_value
    next: done
  - id: done
    label: Done
    kind: terminal
    terminal: true
    doneWhen: terminal
"#,
            r#"- id: is_high_value
  name: High value
  when:
    present: inputs.totalAmount

- id: is_high_value
  name: High value duplicate
  when:
    present: inputs.totalAmount
"#,
        );

        let err = validate_workflow_dir(&dir).expect_err("duplicate gate ids must fail");
        assert_eq!(err.code, "WORKFLOW_SEMANTIC_INVALID");
        assert!(
            err.message.contains("gates"),
            "expected gates path in message, got: {}",
            err.message
        );
        assert!(
            err.message.contains("duplicate"),
            "expected duplicate mention in message, got: {}",
            err.message
        );
        assert!(
            err.message.contains("Hint:"),
            "expected duplicate gate id remediation hint, got: {}",
            err.message
        );
        assert!(
            err.message.contains("Gate Reuse"),
            "expected Gate Reuse mention in hint, got: {}",
            err.message
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}

fn semantic_validation_message(path: &str, message: &str) -> String {
    let base = format!("semantic validation failed at {path}: {message}");
    if is_dead_end_semantic_error(message) {
        format!(
            "{base}. Hint: add a legacy `next` fallback on the stage, or an unconditional catch-all transition (`when: {{ all: [] }}`). See spec-workflow.md §2.4.4."
        )
    } else if is_gate_cycle_error(path, message) {
        format!(
            "{base}. Hint: break the gate reference chain in gates.yaml so no gate references itself transitively (e.g. gate_a → gate_b → gate_a). Static validation catches cycles at author time; at runtime an uncaught cycle evaluates to false (ADR 016). See spec-workflow.md §2.4.5."
        )
    } else if is_unknown_gate_reference(message) && is_gate_reference_path(path) {
        format!(
            "{base}. Hint: verify the gate id exists in gates.yaml (split layout) or the top-level gates: array (bundle YAML); check spelling and that gates.yaml is present when using named gates. See spec-workflow.md §2.4.5."
        )
    } else if is_duplicate_gate_id_error(path, message) {
        format!(
            "{base}. Hint: each gate entry in gates.yaml must have a unique id. Extract shared predicates once and reference them with {{ gate: <id> }} from transitions and other gates. See spec-workflow.md §2.4.5 and §9 Gate Reuse."
        )
    } else if is_reset_field_path(path) {
        format!(
            "{base}. Hint: check that each key in `reset` matches a field `key` in schema.yaml. On rework loops (transitions targeting an earlier stage), declare `reset` for every gating field the target stage's `doneWhen` depends on. See spec-workflow.md §2.4.3 and §9 Reset Completeness."
        )
    } else if is_required_from_stage_path(path) {
        format!(
            "{base}. Hint: check that the stage id in `requiredFromStage` matches a stage `id` in flow.yaml. For path-scoped fields on skippable branches, prefer `requiredFromStage` over blanket `required: true`. See spec-workflow.md §2.4.6."
        )
    } else {
        base
    }
}

fn is_dead_end_semantic_error(message: &str) -> bool {
    message.to_ascii_lowercase().contains("dead end")
}

fn is_reset_field_path(path: &str) -> bool {
    path.contains(".transitions[") && path.contains(".reset[")
}

fn is_required_from_stage_path(path: &str) -> bool {
    path.contains(".requiredFromStage")
}

fn is_gate_cycle_error(path: &str, message: &str) -> bool {
    message.to_ascii_lowercase().contains("cycle")
        && (path == "gates" || path.starts_with("gates["))
}

fn is_unknown_gate_reference(message: &str) -> bool {
    message.contains("gate reference") && message.contains("does not exist")
}

fn is_gate_reference_path(path: &str) -> bool {
    path.starts_with("gates[") || path.contains(".when")
}

fn is_duplicate_gate_id_error(path: &str, message: &str) -> bool {
    path == "gates" && message.to_ascii_lowercase().contains("duplicate")
}

fn collect_required_from_stage_conflict_hints(
    fields: &[smriti_client::WorkflowFieldDef],
) -> Vec<String> {
    fields
        .iter()
        .filter(|field| field.required && field.required_from_stage.is_some())
        .map(|field| {
            format!(
                "schema.fields[{}] has both required: true and requiredFromStage — required wins; requiredFromStage is a no-op. Prefer requiredFromStage alone for path-scoped fields on skippable stages. See spec-workflow.md §2.4.6.",
                field.key
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use smriti_client::SmritiError;

    #[test]
    fn dead_end_semantic_error_includes_remediation_hint() {
        let err = map_smriti_error(SmritiError::SemanticValidation {
            path: "flow.stages[collect_vendor].transitions".to_string(),
            message: "stage 'collect_vendor' has transitions but no exhaustive fallback and no next — risk of dead end".to_string(),
        });
        assert_eq!(err.code, "WORKFLOW_SEMANTIC_INVALID");
        assert!(
            err.message.contains("Hint:"),
            "expected remediation hint, got: {}",
            err.message
        );
        assert!(err.message.contains("`next` fallback"));
        assert!(err.message.contains("when: { all: [] }"));
    }

    #[test]
    fn non_dead_end_semantic_error_omits_remediation_hint() {
        let err = map_smriti_error(SmritiError::SemanticValidation {
            path: "flow.stages[s1].next".to_string(),
            message: "next target 'missing_stage' does not exist".to_string(),
        });
        assert!(
            !err.message.contains("Hint:"),
            "unexpected hint in message: {}",
            err.message
        );
    }

    #[test]
    fn reset_field_semantic_error_includes_remediation_hint() {
        let err = map_smriti_error(SmritiError::SemanticValidation {
            path: "flow.stages[review_summary].transitions[1].reset[0]".to_string(),
            message: "reset references unknown field key 'notAField'".to_string(),
        });
        assert_eq!(err.code, "WORKFLOW_SEMANTIC_INVALID");
        assert!(
            err.message.contains("Hint:"),
            "expected remediation hint, got: {}",
            err.message
        );
        assert!(
            err.message.contains("schema.yaml"),
            "expected schema.yaml mention in hint, got: {}",
            err.message
        );
        assert!(
            err.message.contains("§2.4.3"),
            "expected spec §2.4.3 reference in hint, got: {}",
            err.message
        );
    }

    #[test]
    fn required_from_stage_semantic_error_includes_remediation_hint() {
        let err = map_smriti_error(SmritiError::SemanticValidation {
            path: "schema.fields[expressReason].requiredFromStage".to_string(),
            message: "requiredFromStage 'ghost_stage' does not exist".to_string(),
        });
        assert_eq!(err.code, "WORKFLOW_SEMANTIC_INVALID");
        assert!(
            err.message.contains("Hint:"),
            "expected remediation hint, got: {}",
            err.message
        );
        assert!(
            err.message.contains("flow.yaml"),
            "expected flow.yaml mention in hint, got: {}",
            err.message
        );
        assert!(
            err.message.contains("§2.4.6"),
            "expected spec §2.4.6 reference in hint, got: {}",
            err.message
        );
    }

    #[test]
    fn collect_required_from_stage_conflict_hints_detects_overlap() {
        let hints = collect_required_from_stage_conflict_hints(&[
            smriti_client::WorkflowFieldDef {
                key: "vendorId".to_string(),
                label: "Vendor".to_string(),
                field_type: smriti_client::WorkflowFieldType::String,
                bag: smriti_client::FieldBag::Inputs,
                required: true,
                required_from_stage: Some("collect_vendor".to_string()),
                validation: None,
                hitl_field_id: None,
                phi: false,
                redact_in_prompt: false,
            },
            smriti_client::WorkflowFieldDef {
                key: "expressReason".to_string(),
                label: "Express reason".to_string(),
                field_type: smriti_client::WorkflowFieldType::String,
                bag: smriti_client::FieldBag::Inputs,
                required: false,
                required_from_stage: Some("express_review".to_string()),
                validation: None,
                hitl_field_id: None,
                phi: false,
                redact_in_prompt: false,
            },
        ]);
        assert_eq!(hints.len(), 1);
        assert!(hints[0].contains("schema.fields[vendorId]"));
        assert!(hints[0].contains("required wins"));
    }

    #[test]
    fn gate_cycle_semantic_error_includes_remediation_hint() {
        let err = map_smriti_error(SmritiError::SemanticValidation {
            path: "gates".to_string(),
            message: "gate cycle detected: gate_a -> gate_b -> gate_a".to_string(),
        });
        assert_eq!(err.code, "WORKFLOW_SEMANTIC_INVALID");
        assert!(
            err.message.contains("Hint:"),
            "expected remediation hint, got: {}",
            err.message
        );
        assert!(
            err.message.contains("gates.yaml"),
            "expected gates.yaml mention in hint, got: {}",
            err.message
        );
        assert!(
            err.message.contains("§2.4.5"),
            "expected spec §2.4.5 reference in hint, got: {}",
            err.message
        );
        assert!(
            err.message.contains("ADR 016"),
            "expected ADR 016 mention in hint, got: {}",
            err.message
        );
    }

    #[test]
    fn unknown_gate_from_stage_transition_includes_remediation_hint() {
        let err = map_smriti_error(SmritiError::SemanticValidation {
            path: "flow.stages[collect].transitions[0].when".to_string(),
            message: "gate reference 'does_not_exist' does not exist".to_string(),
        });
        assert_eq!(err.code, "WORKFLOW_SEMANTIC_INVALID");
        assert!(
            err.message.contains("Hint:"),
            "expected remediation hint, got: {}",
            err.message
        );
        assert!(
            err.message.contains("gates.yaml"),
            "expected gates.yaml mention in hint, got: {}",
            err.message
        );
        assert!(
            err.message.contains("§2.4.5"),
            "expected spec §2.4.5 reference in hint, got: {}",
            err.message
        );
    }

    #[test]
    fn unknown_gate_from_nested_gate_when_includes_remediation_hint() {
        let err = map_smriti_error(SmritiError::SemanticValidation {
            path: "gates[0].when".to_string(),
            message: "gate reference 'missing_gate' does not exist".to_string(),
        });
        assert_eq!(err.code, "WORKFLOW_SEMANTIC_INVALID");
        assert!(
            err.message.contains("Hint:"),
            "expected remediation hint, got: {}",
            err.message
        );
        assert!(
            err.message.contains("gates.yaml"),
            "expected gates.yaml mention in hint, got: {}",
            err.message
        );
    }

    #[test]
    fn duplicate_gate_id_semantic_error_includes_remediation_hint() {
        let err = map_smriti_error(SmritiError::SemanticValidation {
            path: "gates".to_string(),
            message: "duplicate gate ids are not allowed".to_string(),
        });
        assert_eq!(err.code, "WORKFLOW_SEMANTIC_INVALID");
        assert!(
            err.message.contains("Hint:"),
            "expected remediation hint, got: {}",
            err.message
        );
        assert!(
            err.message.contains("§2.4.5"),
            "expected spec §2.4.5 reference in hint, got: {}",
            err.message
        );
        assert!(
            err.message.contains("Gate Reuse"),
            "expected Gate Reuse mention in hint, got: {}",
            err.message
        );
    }
}
