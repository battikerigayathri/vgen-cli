//! Typed re-exports of the DAG-era workflow schema, sourced from `smriti_client`.
//! Kept as a thin alias module — never redefine these types locally, or the CLI's
//! parsing will silently drift from the runtime engine's schema.

pub use smriti_client::{Condition, TransitionRule, WorkflowGateDef};

use serde_json::Value;
use std::collections::HashMap;

/// Extracts the `transitions` array of a single stage `Value` as typed `TransitionRule`s,
/// for CLI code (e.g. `vgen graph`, `vgen explain`) that wants structural access
/// rather than raw JSON traversal. Returns an empty vec for legacy `next`-only stages.
pub fn stage_transitions(stage: &Value) -> Vec<TransitionRule> {
    stage
        .get("transitions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| serde_json::from_value::<TransitionRule>(t.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Extracts a workflow bundle's top-level `gates` array as typed `WorkflowGateDef`s.
pub fn bundle_gates(bundle: &Value) -> Vec<WorkflowGateDef> {
    bundle
        .get("gates")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|g| serde_json::from_value::<WorkflowGateDef>(g.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Reset field keys declared on a transition rule, or an empty slice when absent.
pub fn transition_reset_fields(rule: &TransitionRule) -> &[String] {
    rule.reset.as_deref().unwrap_or(&[])
}

/// Maps each stage id to its zero-based index in `flow.yaml` declaration order.
pub fn stage_declaration_order(stages: &[Value]) -> HashMap<String, usize> {
    stages
        .iter()
        .filter_map(|stage| stage.get("id").and_then(|v| v.as_str()))
        .enumerate()
        .map(|(idx, id)| (id.to_string(), idx))
        .collect()
}

/// True when `target` is declared before `from_stage_id` in the workflow stage list.
pub fn transition_is_back_edge(
    from_stage_id: &str,
    target: &str,
    stage_order: &HashMap<String, usize>,
) -> bool {
    match (stage_order.get(from_stage_id), stage_order.get(target)) {
        (Some(from_idx), Some(target_idx)) => target_idx < from_idx,
        _ => false,
    }
}

/// Graph edge label: condition summary plus optional `reset:[field,...]` suffix.
pub fn transition_ref_value(rule: &TransitionRule) -> String {
    let summary = condition_summary(&rule.when);
    let reset = transition_reset_fields(rule);
    if reset.is_empty() {
        summary
    } else {
        format!("{summary} reset:[{}]", reset.join(","))
    }
}

/// Compact human-readable summary of a transition condition for graph edge labels.
pub fn condition_summary(condition: &Condition) -> String {
    match condition {
        Condition::Present(field) => format!("present({field})"),
        Condition::Absent(field) => format!("absent({field})"),
        Condition::Eq { field, value } => {
            let value_str = match value {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            format!("eq({field}={value_str})")
        }
        Condition::All(conds) if conds.is_empty() => "all([])".to_string(),
        Condition::All(conds) => {
            let parts: Vec<String> = conds.iter().map(condition_summary).collect();
            format!("all({})", parts.join(", "))
        }
        Condition::Any(conds) => {
            let parts: Vec<String> = conds.iter().map(condition_summary).collect();
            format!("any({})", parts.join(", "))
        }
        Condition::Not(inner) => format!("not({})", condition_summary(inner)),
        Condition::Gate(id) => format!("gate({id})"),
    }
}

/// Collects every `gate:` id referenced within a condition tree (walking `All`/`Any`/`Not`).
pub fn gate_refs_in_condition<'a>(condition: &'a Condition) -> Vec<&'a str> {
    let mut out = Vec::new();
    collect_gate_refs_in_condition(condition, &mut out);
    out
}

fn collect_gate_refs_in_condition<'a>(condition: &'a Condition, out: &mut Vec<&'a str>) {
    match condition {
        Condition::Gate(id) => out.push(id.as_str()),
        Condition::All(conds) | Condition::Any(conds) => {
            for child in conds {
                collect_gate_refs_in_condition(child, out);
            }
        }
        Condition::Not(inner) => collect_gate_refs_in_condition(inner, out),
        Condition::Present(_) | Condition::Absent(_) | Condition::Eq { .. } => {}
    }
}

/// Returns directed gate→referenced_gate edges from composed gate `when` trees.
pub fn gate_composition_edges(gates: &[WorkflowGateDef]) -> Vec<(String, String)> {
    let mut edges = Vec::new();
    for gate in gates {
        for ref_id in gate_refs_in_condition(&gate.when) {
            edges.push((gate.id.clone(), ref_id.to_string()));
        }
    }
    edges
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn stage_transitions_parses_typed_rules() {
        let stage = json!({
            "id": "classify",
            "transitions": [
                {
                    "target": "fast_track",
                    "when": { "eq": { "field": "priority", "value": "high" } }
                }
            ],
            "next": "standard_path"
        });

        let rules = stage_transitions(&stage);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].target, "fast_track");
        assert!(matches!(
            rules[0].when,
            Condition::Eq { ref field, .. } if field == "priority"
        ));
    }

    #[test]
    fn stage_transitions_empty_for_legacy_next_only() {
        let stage = json!({
            "id": "collect",
            "next": "done"
        });
        assert!(stage_transitions(&stage).is_empty());
    }

    #[test]
    fn stage_transitions_parses_reset_array() {
        let stage = json!({
            "id": "review_summary",
            "transitions": [
                {
                    "target": "submit",
                    "when": { "eq": { "field": "reviewConfirmed", "value": true } }
                },
                {
                    "target": "collect_line_items",
                    "when": { "eq": { "field": "reviewConfirmed", "value": false } },
                    "reset": ["reviewConfirmed", "lineItems"]
                }
            ]
        });

        let rules = stage_transitions(&stage);
        assert_eq!(rules.len(), 2);
        assert!(rules[0].reset.is_none());
        assert_eq!(
            rules[1].reset.as_deref(),
            Some(["reviewConfirmed".to_string(), "lineItems".to_string()].as_slice())
        );
        assert_eq!(
            transition_reset_fields(&rules[1]),
            &["reviewConfirmed".to_string(), "lineItems".to_string()]
        );
        assert_eq!(
            transition_ref_value(&rules[1]),
            "eq(reviewConfirmed=false) reset:[reviewConfirmed,lineItems]"
        );
    }

    #[test]
    fn transition_is_back_edge_uses_declaration_order() {
        let stages = vec![
            json!({ "id": "collect_line_items" }),
            json!({ "id": "review_summary" }),
            json!({ "id": "submit" }),
        ];
        let order = stage_declaration_order(&stages);
        assert!(transition_is_back_edge(
            "review_summary",
            "collect_line_items",
            &order
        ));
        assert!(!transition_is_back_edge(
            "collect_line_items",
            "review_summary",
            &order
        ));
        assert!(!transition_is_back_edge("review_summary", "submit", &order));
    }

    #[test]
    fn condition_summary_formats_primitives() {
        assert_eq!(
            condition_summary(&Condition::Eq {
                field: "priority".into(),
                value: json!("high"),
            }),
            "eq(priority=high)"
        );
        assert_eq!(
            condition_summary(&Condition::Gate("is_vip".into())),
            "gate(is_vip)"
        );
    }

    #[test]
    fn bundle_gates_parses_composed_gate_when_tree() {
        let bundle = json!({
            "gates": [
                {
                    "id": "is_high_value",
                    "name": "High Value Requisition",
                    "when": {
                        "eq": { "field": "inputs.totalAmount", "value": 10000 }
                    }
                },
                {
                    "id": "requires_manager_approval",
                    "name": "Requires Manager Approval",
                    "when": {
                        "all": [
                            { "gate": "is_high_value" },
                            { "not": { "present": "inputs.managerApprovalCode" } }
                        ]
                    }
                }
            ]
        });

        let gates = bundle_gates(&bundle);
        assert_eq!(gates.len(), 2);
        assert_eq!(gates[0].id, "is_high_value");
        assert!(matches!(
            gates[1].when,
            Condition::All(ref conds) if conds.len() == 2
                && matches!(&conds[0], Condition::Gate(id) if id == "is_high_value")
                && matches!(&conds[1], Condition::Not(_))
        ));
    }

    #[test]
    fn gate_refs_in_condition_walks_all_any_not() {
        let condition = Condition::All(vec![
            Condition::Gate("outer".into()),
            Condition::Any(vec![
                Condition::Gate("branch_a".into()),
                Condition::Not(Box::new(Condition::Gate("branch_b".into()))),
            ]),
        ]);
        let refs = gate_refs_in_condition(&condition);
        assert_eq!(refs, vec!["outer", "branch_a", "branch_b"]);
    }

    #[test]
    fn gate_composition_edges_emits_referenced_pairs() {
        let gates = bundle_gates(&json!({
            "gates": [
                {
                    "id": "requires_manager_approval",
                    "name": "Requires Manager Approval",
                    "when": {
                        "all": [
                            { "gate": "is_high_value" },
                            { "not": { "present": "inputs.managerApprovalCode" } }
                        ]
                    }
                },
                {
                    "id": "is_high_value",
                    "name": "High Value Requisition",
                    "when": {
                        "eq": { "field": "inputs.totalAmount", "value": 10000 }
                    }
                }
            ]
        }));

        let edges = gate_composition_edges(&gates);
        assert_eq!(
            edges,
            vec![(
                "requires_manager_approval".to_string(),
                "is_high_value".to_string()
            )]
        );
    }
}
