use crate::graph::Graph;
use crate::specs::{
    list_assistant_files, list_workflow_dirs, load_workflow_from_dir,
    parse_assistant_workflow_slug, ResourceIndex,
};
use crate::workflow_validate::{validate_workflow_dir, WorkflowValidateData};
use crate::workspace::Workspace;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::{
    push_finding, read_yaml, relative_path, string_field, Finding, ResourceKind, ResourceRef,
    Severity,
};

pub fn check_workflow_rules(ws: &Workspace, _graph: &Graph, index: &ResourceIndex) -> Vec<Finding> {
    let mut findings = Vec::new();
    check_assistant_workflow_binding(ws, index, &mut findings);
    check_workflow_folder_orphans(ws, index, &mut findings);
    check_workflow_dirs(ws, &mut findings);
    findings
}

fn check_assistant_workflow_binding(
    ws: &Workspace,
    index: &ResourceIndex,
    findings: &mut Vec<Finding>,
) {
    for (stem, path) in list_assistant_files(&ws.assistants_dir) {
        let Ok(value) = read_yaml(&path) else {
            continue;
        };
        let resource = ResourceRef {
            kind: ResourceKind::Assistant,
            slug: string_field(&value, "slug").or(Some(stem)),
            id: string_field(&value, "id"),
        };
        let rel_path = relative_path(&path, &ws.root);

        // 1. Check if the explicit workflows array is present
        if let Some(workflows_arr) = value.get("workflows").and_then(|v| v.as_array()) {
            for (i, wf_val) in workflows_arr.iter().enumerate() {
                let Some(slug) = wf_val.get("slug").and_then(|v| v.as_str()) else {
                    push_finding(
                        findings,
                        Finding {
                            code: "ASSISTANT_WORKFLOW_MISSING_SLUG".to_string(),
                            severity: Severity::Error,
                            message: format!(
                                "Workflow reference at index {i} is missing its 'slug' property."
                            ),
                            path: Some(rel_path.clone()),
                            resource: Some(resource.clone()),
                        },
                    );
                    continue;
                };

                // Validate that the workflow exists in compile set
                if !index.workflows_by_slug.contains_key(slug) {
                    push_finding(
                        findings,
                        Finding {
                            code: "WORKFLOW_SLUG_MISMATCH".to_string(),
                            severity: Severity::Error,
                            message: format!(
                                "workflows[{i}].slug `{slug}` does not match any workflow definition meta slug"
                            ),
                            path: Some(rel_path.clone()),
                            resource: Some(resource.clone()),
                        },
                    );
                }
            }
        } else {
            // 2. Fallback to legacy validation via systemContext
            let Some(system_context) = value.get("systemContext").and_then(|v| v.as_str()) else {
                continue;
            };

            let has_workflow_type = system_context.lines().any(|line| {
                let line = line.trim();
                line.starts_with("workflow_type:")
                    && !line["workflow_type:".len()..].trim().is_empty()
            });
            let workflow_slug = parse_assistant_workflow_slug(system_context);

            if has_workflow_type && workflow_slug.is_none() {
                push_finding(
                    findings,
                    Finding {
                        code: "WORKFLOW_SLUG_MISSING".to_string(),
                        severity: Severity::Error,
                        message: "systemContext has workflow_type but no workflow_definition_slug"
                            .to_string(),
                        path: Some(rel_path.clone()),
                        resource: Some(resource.clone()),
                    },
                );
                continue;
            }

            if let Some(slug) = workflow_slug {
                if !index.workflows_by_slug.contains_key(&slug) {
                    push_finding(
                        findings,
                        Finding {
                            code: "WORKFLOW_SLUG_MISMATCH".to_string(),
                            severity: Severity::Error,
                            message: format!(
                                "workflow_definition_slug `{slug}` does not match any workflow meta slug"
                            ),
                            path: Some(rel_path),
                            resource: Some(resource),
                        },
                    );
                }
            }
        }
    }
}

fn check_workflow_folder_orphans(
    ws: &Workspace,
    index: &ResourceIndex,
    findings: &mut Vec<Finding>,
) {
    let mut referenced = std::collections::HashSet::new();
    for (_stem, path) in list_assistant_files(&ws.assistants_dir) {
        if let Ok(value) = read_yaml(&path) {
            // Decoupled workflows array
            if let Some(workflows_arr) = value.get("workflows").and_then(|v| v.as_array()) {
                for wf_val in workflows_arr {
                    if let Some(slug) = wf_val.get("slug").and_then(|v| v.as_str()) {
                        referenced.insert(slug.to_string());
                    }
                }
            }
            // Legacy systemContext
            if let Some(system_context) = value.get("systemContext").and_then(|v| v.as_str()) {
                if let Some(slug) = parse_assistant_workflow_slug(system_context) {
                    referenced.insert(slug);
                }
            }
        }
    }

    for workflow_dir in list_workflow_dirs(&ws.workflows_dir) {
        let Ok((bundle, _, _)) = load_workflow_from_dir(&workflow_dir) else {
            continue;
        };
        let slug = bundle
            .get("meta")
            .and_then(|m| string_field(m, "slug"))
            .or_else(|| string_field(&bundle, "slug"));
        let Some(slug) = slug else {
            continue;
        };
        if referenced.contains(&slug) {
            continue;
        }
        let rel = relative_path(&workflow_dir, &ws.root);
        push_finding(
            findings,
            Finding {
                code: "WORKFLOW_FOLDER_ORPHAN".to_string(),
                severity: Severity::Warning,
                message: format!("workflow slug `{slug}` is not referenced by any assistant"),
                path: Some(rel),
                resource: Some(ResourceRef {
                    kind: ResourceKind::Workflow,
                    slug: Some(slug.clone()),
                    id: index
                        .workflows_by_slug
                        .get(&slug)
                        .and_then(|_| bundle.get("meta").and_then(|m| string_field(m, "id"))),
                }),
            },
        );
    }
}

fn check_workflow_dirs(ws: &Workspace, findings: &mut Vec<Finding>) {
    for workflow_dir in list_workflow_dirs(&ws.workflows_dir) {
        let rel = relative_path(&workflow_dir, &ws.root);
        match validate_workflow_dir(&workflow_dir) {
            Ok(data) => {
                check_linear_only_workflow_hint(&workflow_dir, &rel, &data, findings);
                check_required_on_skippable_stage_hint(&workflow_dir, &rel, &data, findings);
                check_back_edge_without_reset_hint(&workflow_dir, &rel, &data, findings);
                check_duplicate_inline_condition_hint(&workflow_dir, &rel, &data, findings);
            }
            Err(err) => {
                push_finding(
                    findings,
                    Finding {
                        code: err.code.to_string(),
                        severity: Severity::Error,
                        message: err.message,
                        path: Some(rel),
                        resource: Some(ResourceRef {
                            kind: ResourceKind::Workflow,
                            slug: workflow_dir
                                .file_name()
                                .and_then(|s| s.to_str())
                                .map(str::to_string),
                            id: None,
                        }),
                    },
                );
            }
        }
    }
}

fn check_linear_only_workflow_hint(
    workflow_dir: &Path,
    rel: &str,
    data: &WorkflowValidateData,
    findings: &mut Vec<Finding>,
) {
    let Ok((bundle, _, _)) = load_workflow_from_dir(workflow_dir) else {
        return;
    };
    let Some(stages) = bundle_flow_stages(&bundle) else {
        return;
    };
    if !is_linear_only_workflow(&stages) {
        return;
    }
    push_finding(
        findings,
        Finding {
            code: "WORKFLOW_LINEAR_ONLY".to_string(),
            severity: Severity::Info,
            message: format!(
                "workflow `{}` uses linear next/back_to only; for conditional branching see spec-workflow.md §2.4 (transitions + Condition)",
                data.slug
            ),
            path: Some(rel.to_string()),
            resource: Some(ResourceRef {
                kind: ResourceKind::Workflow,
                slug: Some(data.slug.clone()),
                id: bundle
                    .get("meta")
                    .and_then(|m| string_field(m, "id")),
            }),
        },
    );
}

fn bundle_flow_stages(bundle: &Value) -> Option<Vec<&Value>> {
    bundle
        .get("flow")
        .and_then(|flow| flow.get("stages"))
        .and_then(|stages| stages.as_array())
        .map(|stages| stages.iter().collect())
}

fn is_linear_only_workflow(stages: &[&Value]) -> bool {
    !stages.is_empty()
        && stages.iter().all(|stage| {
            stage
                .get("transitions")
                .and_then(|transitions| transitions.as_array())
                .is_none_or(|transitions| transitions.is_empty())
        })
}

/// Stages that may be skipped on some branch: transition targets plus default
/// `next` from any stage that declares conditional transitions.
fn branch_skippable_stage_ids(stages: &[&Value]) -> HashSet<String> {
    let mut skippable = HashSet::new();
    let mut branching_stage_ids = HashSet::new();

    for stage in stages {
        let Some(stage_id) = stage.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        if let Some(transitions) = stage.get("transitions").and_then(|v| v.as_array()) {
            if !transitions.is_empty() {
                branching_stage_ids.insert(stage_id.to_string());
                for rule in transitions {
                    if let Some(target) = rule.get("target").and_then(|v| v.as_str()) {
                        skippable.insert(target.to_string());
                    }
                }
            }
        }
    }

    for stage in stages {
        let Some(stage_id) = stage.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        if !branching_stage_ids.contains(stage_id) {
            continue;
        }
        if let Some(next) = stage.get("next").and_then(|v| v.as_str()) {
            skippable.insert(next.to_string());
        }
    }

    skippable
}

fn done_when_field_keys(stage: &Value) -> Vec<String> {
    let Some(done_when) = stage.get("doneWhen") else {
        return Vec::new();
    };
    match done_when {
        Value::String(key) if key != "terminal" && key != "validator_pass" => {
            vec![key.clone()]
        }
        Value::Array(items) => items
            .iter()
            .filter_map(|item| item.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

fn infer_done_when_stages(stages: &[&Value]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for stage in stages {
        let Some(stage_id) = stage.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        for key in done_when_field_keys(stage) {
            map.entry(key).or_insert_with(|| stage_id.to_string());
        }
    }
    map
}

fn field_is_required(field: &Value) -> bool {
    field.get("required").and_then(|v| v.as_bool()) == Some(true)
}

fn field_owning_stage(field: &Value, done_when_stages: &HashMap<String, String>) -> Option<String> {
    if let Some(stage) = field.get("requiredFromStage").and_then(|v| v.as_str()) {
        return Some(stage.to_string());
    }
    let key = field.get("key").and_then(|v| v.as_str())?;
    done_when_stages.get(key).cloned()
}

fn check_required_on_skippable_stage_hint(
    workflow_dir: &Path,
    rel: &str,
    data: &WorkflowValidateData,
    findings: &mut Vec<Finding>,
) {
    let Ok((bundle, _, _)) = load_workflow_from_dir(workflow_dir) else {
        return;
    };
    let Some(stages) = bundle_flow_stages(&bundle) else {
        return;
    };
    let skippable = branch_skippable_stage_ids(&stages);
    if skippable.is_empty() {
        return;
    }

    let done_when_stages = infer_done_when_stages(&stages);
    let Some(fields) = bundle
        .get("schema")
        .and_then(|schema| schema.get("fields"))
        .and_then(|fields| fields.as_array())
    else {
        return;
    };

    for field in fields {
        if !field_is_required(field) {
            continue;
        }
        let Some(field_key) = field.get("key").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(stage_id) = field_owning_stage(field, &done_when_stages) else {
            continue;
        };
        if !skippable.contains(&stage_id) {
            continue;
        }
        push_finding(
            findings,
            Finding {
                code: "WORKFLOW_REQUIRED_ON_SKIPPABLE_STAGE".to_string(),
                severity: Severity::Info,
                message: format!(
                    "field `{field_key}` uses required: true on branch-skippable stage `{stage_id}`; prefer requiredFromStage (see spec-workflow.md §2.4.6)"
                ),
                path: Some(rel.to_string()),
                resource: Some(ResourceRef {
                    kind: ResourceKind::Workflow,
                    slug: Some(data.slug.clone()),
                    id: bundle.get("meta").and_then(|m| string_field(m, "id")),
                }),
            },
        );
    }
}

fn stage_declaration_order(stages: &[&Value]) -> HashMap<String, usize> {
    stages
        .iter()
        .enumerate()
        .filter_map(|(idx, stage)| {
            stage
                .get("id")
                .and_then(|v| v.as_str())
                .map(|id| (id.to_string(), idx))
        })
        .collect()
}

fn transition_reset_is_empty(rule: &Value) -> bool {
    match rule.get("reset") {
        None => true,
        Some(Value::Array(items)) => items.is_empty(),
        Some(_) => true,
    }
}

fn is_back_edge_transition(from_idx: usize, target: &str, order: &HashMap<String, usize>) -> bool {
    order
        .get(target)
        .is_some_and(|&target_idx| target_idx < from_idx)
}

fn check_back_edge_without_reset_hint(
    workflow_dir: &Path,
    rel: &str,
    data: &WorkflowValidateData,
    findings: &mut Vec<Finding>,
) {
    let Ok((bundle, _, _)) = load_workflow_from_dir(workflow_dir) else {
        return;
    };
    let Some(stages) = bundle_flow_stages(&bundle) else {
        return;
    };
    let order = stage_declaration_order(&stages);

    for stage in &stages {
        let Some(from_id) = stage.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(from_idx) = order.get(from_id).copied() else {
            continue;
        };
        let Some(transitions) = stage.get("transitions").and_then(|v| v.as_array()) else {
            continue;
        };

        for (idx, rule) in transitions.iter().enumerate() {
            let Some(target) = rule.get("target").and_then(|v| v.as_str()) else {
                continue;
            };
            if !is_back_edge_transition(from_idx, target, &order) {
                continue;
            }
            if !transition_reset_is_empty(rule) {
                continue;
            }
            push_finding(
                findings,
                Finding {
                    code: "WORKFLOW_BACK_EDGE_WITHOUT_RESET".to_string(),
                    severity: Severity::Info,
                    message: format!(
                        "flow.stages[{from_id}].transitions[{idx}] targets earlier stage `{target}` without `reset`; stale `doneWhen` fields may cause an immediate bounce forward. Declare `reset` for gating fields (see spec-workflow.md §2.4.3 and §9 Reset Completeness)"
                    ),
                    path: Some(rel.to_string()),
                    resource: Some(ResourceRef {
                        kind: ResourceKind::Workflow,
                        slug: Some(data.slug.clone()),
                        id: bundle.get("meta").and_then(|m| string_field(m, "id")),
                    }),
                },
            );
        }
    }
}

fn workflow_has_named_gates(workflow_dir: &Path, bundle: &Value) -> bool {
    if workflow_dir.join("gates.yaml").is_file() {
        return true;
    }
    bundle
        .get("gates")
        .and_then(|gates| gates.as_array())
        .is_some_and(|gates| !gates.is_empty())
}

fn canonical_json_string(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let parts: Vec<String> = keys
                .iter()
                .map(|key| format!("{key}:{}", canonical_json_string(&map[*key])))
                .collect();
            format!("{{{}}}", parts.join(","))
        }
        Value::Array(items) => {
            let parts: Vec<String> = items.iter().map(canonical_json_string).collect();
            format!("[{}]", parts.join(","))
        }
        _ => value.to_string(),
    }
}

fn collect_transition_when_clauses(stages: &[&Value]) -> Vec<(String, Value)> {
    let mut clauses = Vec::new();
    for stage in stages {
        let Some(stage_id) = stage.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(transitions) = stage.get("transitions").and_then(|v| v.as_array()) else {
            continue;
        };
        for (idx, rule) in transitions.iter().enumerate() {
            let Some(when) = rule.get("when") else {
                continue;
            };
            clauses.push((
                format!("flow.stages[{stage_id}].transitions[{idx}].when"),
                when.clone(),
            ));
        }
    }
    clauses
}

fn duplicate_when_clause_groups(clauses: &[(String, Value)]) -> Vec<(String, Vec<String>)> {
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for (path, when) in clauses {
        groups
            .entry(canonical_json_string(when))
            .or_default()
            .push(path.clone());
    }
    groups
        .into_iter()
        .filter(|(_, paths)| paths.len() > 1)
        .map(|(canonical, mut paths)| {
            paths.sort();
            (canonical, paths)
        })
        .collect()
}

fn check_duplicate_inline_condition_hint(
    workflow_dir: &Path,
    rel: &str,
    data: &WorkflowValidateData,
    findings: &mut Vec<Finding>,
) {
    let Ok((bundle, _, _)) = load_workflow_from_dir(workflow_dir) else {
        return;
    };
    if workflow_has_named_gates(workflow_dir, &bundle) {
        return;
    }
    let Some(stages) = bundle_flow_stages(&bundle) else {
        return;
    };
    let clauses = collect_transition_when_clauses(&stages);
    for (_canonical, paths) in duplicate_when_clause_groups(&clauses) {
        push_finding(
            findings,
            Finding {
                code: "WORKFLOW_DUPLICATE_INLINE_CONDITION".to_string(),
                severity: Severity::Info,
                message: format!(
                    "identical `when` condition copy-pasted at {} — extract into gates.yaml and reference with {{ gate: <id> }} (see spec-workflow.md §2.4.5 and §9 Gate Reuse)",
                    paths.join(", ")
                ),
                path: Some(rel.to_string()),
                resource: Some(ResourceRef {
                    kind: ResourceKind::Workflow,
                    slug: Some(data.slug.clone()),
                    id: bundle.get("meta").and_then(|m| string_field(m, "id")),
                }),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_only_when_no_stage_has_transitions() {
        let s1 = serde_json::json!({ "id": "s1", "next": "s2" });
        let s2 = serde_json::json!({ "id": "s2", "kind": "terminal" });
        let stages = vec![&s1, &s2];
        assert!(is_linear_only_workflow(&stages));
    }

    #[test]
    fn not_linear_only_when_any_stage_has_transitions() {
        let s1 = serde_json::json!({
            "id": "s1",
            "transitions": [{ "target": "fast", "when": { "all": [] } }]
        });
        let s2 = serde_json::json!({ "id": "s2", "kind": "terminal" });
        let stages = vec![&s1, &s2];
        assert!(!is_linear_only_workflow(&stages));
    }

    #[test]
    fn empty_transitions_array_counts_as_linear_only() {
        let s1 = serde_json::json!({ "id": "s1", "transitions": [], "next": "done" });
        let stages = vec![&s1];
        assert!(is_linear_only_workflow(&stages));
    }

    #[test]
    fn branch_skippable_includes_transition_targets_and_branch_next() {
        let collect = serde_json::json!({
            "id": "collect_vendor",
            "transitions": [{ "target": "express_review", "when": { "all": [] } }],
            "next": "collect_line_items"
        });
        let express = serde_json::json!({ "id": "express_review" });
        let line_items = serde_json::json!({ "id": "collect_line_items" });
        let stages = vec![&collect, &express, &line_items];
        let skippable = branch_skippable_stage_ids(&stages);
        assert!(skippable.contains("express_review"));
        assert!(skippable.contains("collect_line_items"));
        assert!(!skippable.contains("collect_vendor"));
    }

    #[test]
    fn required_on_skippable_stage_emits_doctor_hint() {
        let collect = serde_json::json!({
            "id": "collect_vendor",
            "transitions": [{ "target": "express_review", "when": { "all": [] } }],
            "next": "collect_line_items"
        });
        let express = serde_json::json!({
            "id": "express_review",
            "doneWhen": ["expressReason"]
        });
        let stages = vec![&collect, &express];
        let skippable = branch_skippable_stage_ids(&stages);
        let done_when = infer_done_when_stages(&stages);
        let field = serde_json::json!({
            "key": "expressReason",
            "required": true
        });
        assert!(skippable.contains("express_review"));
        assert_eq!(
            field_owning_stage(&field, &done_when).as_deref(),
            Some("express_review")
        );
    }

    #[test]
    fn required_from_stage_only_field_on_skippable_stage_no_hint_trigger() {
        let collect = serde_json::json!({
            "id": "collect_vendor",
            "transitions": [{ "target": "express_review", "when": { "all": [] } }],
            "next": "collect_line_items"
        });
        let express = serde_json::json!({ "id": "express_review" });
        let stages = vec![&collect, &express];
        let done_when = infer_done_when_stages(&stages);
        let field = serde_json::json!({
            "key": "expressReason",
            "requiredFromStage": "express_review"
        });
        assert!(!field_is_required(&field));
        assert_eq!(
            field_owning_stage(&field, &done_when).as_deref(),
            Some("express_review")
        );
    }

    #[test]
    fn back_edge_without_reset_detected_by_declaration_order() {
        let collect = serde_json::json!({ "id": "collect_line_items" });
        let review = serde_json::json!({
            "id": "review_summary",
            "transitions": [{
                "target": "collect_line_items",
                "when": { "all": [] }
            }]
        });
        let stages = vec![&collect, &review];
        let order = stage_declaration_order(&stages);
        assert!(is_back_edge_transition(1, "collect_line_items", &order));
        assert!(!is_back_edge_transition(0, "review_summary", &order));
        assert!(transition_reset_is_empty(&stages[1]["transitions"][0]));
    }

    #[test]
    fn back_edge_with_reset_not_flagged_as_empty() {
        let rule = serde_json::json!({
            "target": "collect_line_items",
            "when": { "all": [] },
            "reset": ["reviewConfirmed", "lineItems"]
        });
        assert!(!transition_reset_is_empty(&rule));
    }

    #[test]
    fn back_edge_without_reset_emits_doctor_hint() {
        let collect = serde_json::json!({ "id": "collect_line_items", "doneWhen": ["lineItems"] });
        let review = serde_json::json!({
            "id": "review_summary",
            "doneWhen": ["reviewConfirmed"],
            "transitions": [{
                "target": "collect_line_items",
                "when": { "eq": { "field": "inputs.reviewConfirmed", "value": false } }
            }]
        });
        let stages: Vec<&Value> = vec![&collect, &review];
        let order = stage_declaration_order(&stages);
        let from_idx = order["review_summary"];
        let rule = &review["transitions"][0];
        assert!(is_back_edge_transition(
            from_idx,
            "collect_line_items",
            &order
        ));
        assert!(transition_reset_is_empty(rule));
    }

    #[test]
    fn duplicate_when_clause_groups_detects_copy_pasted_conditions() {
        let shared = serde_json::json!({
            "eq": { "field": "inputs.vendorId", "value": "V-PRE_APPROVED" }
        });
        let s1 = serde_json::json!({
            "id": "collect_vendor",
            "transitions": [{ "target": "express_review", "when": shared.clone() }]
        });
        let s2 = serde_json::json!({
            "id": "review_summary",
            "transitions": [{ "target": "submit", "when": shared }]
        });
        let stages = vec![&s1, &s2];
        let clauses = collect_transition_when_clauses(&stages);
        let groups = duplicate_when_clause_groups(&clauses);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].1.len(), 2);
        assert!(groups[0].1[0].contains("collect_vendor"));
        assert!(groups[0].1[1].contains("review_summary"));
    }

    #[test]
    fn duplicate_when_clause_groups_ignores_distinct_conditions() {
        let s1 = serde_json::json!({
            "id": "collect_vendor",
            "transitions": [{
                "target": "express_review",
                "when": { "eq": { "field": "inputs.vendorId", "value": "A" } }
            }]
        });
        let s2 = serde_json::json!({
            "id": "review_summary",
            "transitions": [{
                "target": "submit",
                "when": { "eq": { "field": "inputs.vendorId", "value": "B" } }
            }]
        });
        let stages = vec![&s1, &s2];
        let clauses = collect_transition_when_clauses(&stages);
        assert!(duplicate_when_clause_groups(&clauses).is_empty());
    }

    #[test]
    fn canonical_json_string_treats_object_key_order_equivalently() {
        let a = serde_json::json!({ "eq": { "field": "inputs.flag", "value": true } });
        let b = serde_json::json!({ "eq": { "value": true, "field": "inputs.flag" } });
        assert_eq!(canonical_json_string(&a), canonical_json_string(&b));
    }
}
