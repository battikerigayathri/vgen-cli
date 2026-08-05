use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowBlockStyle {
    /// Contract block: === ACTIVE WORKFLOW === … === END WORKFLOW ===
    Full,
    /// Agent handoff: 4–6 lines (stage, kind, missing, expected outcome, completeness)
    Slim,
}

/// Returns None when snapshot JSON is absent or fails minimal shape check — caller omits section.
pub fn format_workflow_block(snapshot: &Value, style: WorkflowBlockStyle) -> Option<String> {
    if !snapshot_has_minimal_shape(snapshot) {
        return None;
    }

    match style {
        WorkflowBlockStyle::Full => Some(format_full_block(snapshot)),
        WorkflowBlockStyle::Slim => format_slim_block(snapshot),
    }
}

/// Convenience: Full block only.
pub fn format_workflow_block_full(snapshot: &Value) -> Option<String> {
    format_workflow_block(snapshot, WorkflowBlockStyle::Full)
}

/// One-line summary for validator when full block skipped.
pub fn workflow_validator_summary(snapshot: &Value) -> Option<String> {
    if !snapshot_has_minimal_shape(snapshot) {
        return None;
    }
    let workflow_type = field_str(snapshot, "workflowType", "workflow_type")?;
    let stage = field_str(snapshot, "stage", "stage")?;
    let kind = stage_kind_display(snapshot);
    let completeness = completeness_display(snapshot);
    let missing = missing_required_list(snapshot);
    let missing_part = if missing.is_empty() {
        String::new()
    } else {
        format!(" missing=[{}]", missing.join(", "))
    };
    Some(format!(
        "Workflow: {workflow_type} stage={stage} [{kind}] completeness={completeness}{missing_part}"
    ))
}

fn snapshot_has_minimal_shape(snapshot: &Value) -> bool {
    if snapshot.is_null() {
        return false;
    }
    let has_id = field_str(snapshot, "workflowId", "workflow_id").is_some();
    let has_type = field_str(snapshot, "workflowType", "workflow_type").is_some();
    let has_stage = field_str(snapshot, "stage", "stage").is_some();
    (has_id || has_type) && has_stage
}

fn format_full_block(snapshot: &Value) -> String {
    let mut lines = vec!["=== ACTIVE WORKFLOW ===".to_string()];

    if let Some(id) = field_str(snapshot, "workflowId", "workflow_id") {
        lines.push(format!("workflowId: {id}"));
    }
    if let Some(wt) = field_str(snapshot, "workflowType", "workflow_type") {
        lines.push(format!("workflowType: {wt}"));
    }
    if let Some(stage_line) = stage_composite_line(snapshot) {
        lines.push(format!("stage: {stage_line}"));
    }
    if let Some(status) = status_display(snapshot) {
        lines.push(format!("status: {status}"));
    }
    lines.push(format!("completeness: {}", completeness_display(snapshot)));

    let empty_object = Value::Object(Default::default());
    let inputs = snapshot
        .get("inputsSummary")
        .or_else(|| snapshot.get("inputs_summary"))
        .unwrap_or(&empty_object);
    let artifacts = snapshot
        .get("artifactsSummary")
        .or_else(|| snapshot.get("artifacts_summary"))
        .unwrap_or(&empty_object);
    lines.push(format!("inputs: {}", sorted_json_string(inputs)));
    lines.push(format!("artifacts: {}", sorted_json_string(artifacts)));

    let missing = missing_required_list(snapshot);
    if !missing.is_empty() {
        lines.push(format!(
            "missingRequired: {}",
            serde_json::to_string(&missing).unwrap_or_else(|_| "[]".into())
        ));
    }

    if let Some(slug) = field_str(snapshot, "nextHitlSlug", "next_hitl_slug") {
        lines.push(format!("nextHitlSlug: {slug}"));
    }
    if let Some(outcome) = field_str(snapshot, "expectedOutcome", "expected_outcome") {
        lines.push(format!("expectedOutcome: {outcome}"));
    }

    let validation_errors = snapshot
        .get("validationErrors")
        .or_else(|| snapshot.get("validation_errors"))
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    if validation_errors > 0 {
        lines.push(format!(
            "validationErrors: {validation_errors} field error(s)"
        ));
    }

    lines.push("=== END WORKFLOW ===".to_string());

    let mut block = lines.join("\n");
    apply_block_char_cap(&mut block, inputs, artifacts);
    block
}

fn format_slim_block(snapshot: &Value) -> Option<String> {
    let workflow_type = field_str(snapshot, "workflowType", "workflow_type")?;
    let stage_line = stage_composite_line(snapshot)?;
    let mut lines = vec![format!("Business workflow: {workflow_type} @ {stage_line}")];
    lines.push(format!("Completeness: {}", completeness_display(snapshot)));

    let missing = missing_required_list(snapshot);
    if !missing.is_empty() {
        lines.push(format!("Missing required: {}", missing.join(", ")));
    }

    if let Some(outcome) = field_str(snapshot, "expectedOutcome", "expected_outcome") {
        lines.push(format!("Stage expected outcome: {outcome}"));
    }

    Some(lines.join("\n"))
}

fn field_str(value: &Value, camel: &str, snake: &str) -> Option<String> {
    value
        .get(camel)
        .or_else(|| value.get(snake))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn stage_composite_line(snapshot: &Value) -> Option<String> {
    let stage = field_str(snapshot, "stage", "stage")?;
    let label = field_str(snapshot, "stageLabel", "stage_label").unwrap_or_default();
    let kind = stage_kind_display(snapshot);
    if label.is_empty() {
        Some(format!("{stage} [{kind}]"))
    } else {
        Some(format!("{stage} ({label}) [{kind}]"))
    }
}

fn stage_kind_display(snapshot: &Value) -> String {
    snapshot
        .get("stageKind")
        .or_else(|| snapshot.get("stage_kind"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn status_display(snapshot: &Value) -> Option<String> {
    snapshot
        .get("status")
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

fn completeness_display(snapshot: &Value) -> String {
    snapshot
        .get("completenessPct")
        .or_else(|| snapshot.get("completeness_pct"))
        .and_then(|v| v.as_u64())
        .map(|n| format!("{n}%"))
        .unwrap_or_else(|| "0%".to_string())
}

fn missing_required_list(snapshot: &Value) -> Vec<String> {
    snapshot
        .get("missingRequired")
        .or_else(|| snapshot.get("missing_required"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn sorted_json_string(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut sorted = serde_json::Map::new();
            let mut keys: Vec<_> = map.keys().cloned().collect();
            keys.sort();
            for k in keys {
                if let Some(v) = map.get(&k) {
                    sorted.insert(k, v.clone());
                }
            }
            serde_json::to_string(&Value::Object(sorted)).unwrap_or_else(|_| "{}".into())
        }
        other => serde_json::to_string(other).unwrap_or_else(|_| "{}".into()),
    }
}

fn workflow_prompt_block_max_chars() -> usize {
    std::env::var("WORKFLOW_PROMPT_BLOCK_MAX_CHARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8192)
}

fn apply_block_char_cap(block: &mut String, inputs: &Value, artifacts: &Value) {
    let cap = workflow_prompt_block_max_chars();
    if block.len() <= cap {
        return;
    }

    let truncated_inputs = truncate_json_value(inputs);
    let truncated_artifacts = truncate_json_value(artifacts);
    let inputs_str = sorted_json_string(&truncated_inputs);
    let artifacts_str = sorted_json_string(&truncated_artifacts);

    if let Some(idx) = block.find("inputs: ")
        && let Some(end) = block[idx..].find('\n')
    {
        let replace_end = idx + end;
        block.replace_range(idx..replace_end, &format!("inputs: {inputs_str}"));
    }
    if block.len() <= cap {
        return;
    }
    if let Some(idx) = block.find("artifacts: ")
        && let Some(end) = block[idx..].find('\n')
    {
        let replace_end = idx + end;
        block.replace_range(idx..replace_end, &format!("artifacts: {artifacts_str}"));
    }
    if block.len() > cap {
        block.truncate(cap);
        block.push_str("\n… [truncated]");
    }
}

fn truncate_json_value(value: &Value) -> Value {
    let s = sorted_json_string(value);
    const MAX: usize = 256;
    if s.len() <= MAX {
        return value.clone();
    }
    json!(format!("{}… [truncated]", &s[..MAX]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Value {
        serde_json::from_str(include_str!(
            "../tests/fixtures/workflow_snapshot_collect.json"
        ))
        .unwrap()
    }

    #[test]
    fn format_workflow_block_full_golden() {
        let block = format_workflow_block_full(&fixture()).expect("block");
        assert!(block.starts_with("=== ACTIVE WORKFLOW ==="));
        assert!(block.ends_with("=== END WORKFLOW ==="));
        assert!(block.contains("workflowId: wf-8f3a2b1c"));
        assert!(block.contains("workflowType: purchase_requisition"));
        assert!(block.contains("stage: collect_line_items (Collect line items) [collect]"));
        assert!(block.contains("status: in_progress"));
        assert!(block.contains("completeness: 40%"));
        assert!(block.contains(r#""lineItems":"[2 items]""#));
        assert!(block.contains(r#""vendorId":"V-123""#));
        assert!(block.contains("missingRequired: [\"lineItems\",\"costCenter\"]"));
        assert!(block.contains("nextHitlSlug: purchase-req-lines-form"));
        assert!(block.contains("expectedOutcome: User entered all required line items"));
        // lineItems before vendorId (alphabetical keys)
        assert!(block.find("\"lineItems\"").unwrap() < block.find("\"vendorId\"").unwrap());
    }

    #[test]
    fn format_workflow_block_slim_golden() {
        let block =
            format_workflow_block(&fixture(), WorkflowBlockStyle::Slim).expect("slim block");
        assert!(block.starts_with(
            "Business workflow: purchase_requisition @ collect_line_items (Collect line items) [collect]"
        ));
        assert!(block.contains("Completeness: 40%"));
        assert!(block.contains("Missing required: lineItems, costCenter"));
        assert!(block.contains("Stage expected outcome: User entered all required line items"));
        assert!(!block.contains("=== ACTIVE WORKFLOW ==="));
        assert!(!block.contains("inputs:"));
    }

    #[test]
    fn format_workflow_block_none_on_null_or_incomplete() {
        assert!(format_workflow_block_full(&Value::Null).is_none());
        assert!(format_workflow_block_full(&json!({})).is_none());
        assert!(format_workflow_block_full(&json!({"workflowType": "x"})).is_none());
    }

    #[test]
    fn format_workflow_block_omits_empty_missing_required() {
        let mut snap = fixture();
        snap.as_object_mut()
            .unwrap()
            .insert("missingRequired".into(), json!([]));
        let block = format_workflow_block_full(&snap).expect("block");
        assert!(!block.contains("missingRequired:"));
    }

    #[test]
    fn format_workflow_block_slim_omits_empty_missing() {
        let mut snap = fixture();
        snap.as_object_mut()
            .unwrap()
            .insert("missingRequired".into(), json!([]));
        let block = format_workflow_block(&snap, WorkflowBlockStyle::Slim).expect("slim");
        assert!(!block.contains("Missing required:"));
    }

    #[test]
    fn workflow_validator_summary_one_line() {
        let summary = workflow_validator_summary(&fixture()).expect("summary");
        assert!(summary.contains("purchase_requisition"));
        assert!(summary.contains("collect_line_items"));
        assert!(summary.contains("completeness=40%"));
        assert!(summary.contains("lineItems"));
    }
}
