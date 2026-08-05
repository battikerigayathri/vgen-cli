use serde::{Deserialize, Serialize};

/// Parsed from assistant `systemContext` ## Orchestration Contract block.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct OrchestrationContract {
    #[serde(default)]
    pub outcome_profile: String,
    #[serde(default)]
    pub orchestrator: String,
    #[serde(default)]
    pub validator: String,
    #[serde(default)]
    pub compose: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deliverable_fields: Vec<String>,
    #[serde(default)]
    pub workflow_type: String,
    #[serde(default)]
    pub workflow_definition_slug: String,
}

impl OrchestrationContract {
    pub fn has_workflow_binding(&self) -> bool {
        !self.workflow_definition_slug.trim().is_empty()
    }

    pub fn is_deliverable_codegen(&self) -> bool {
        self.outcome_profile
            .eq_ignore_ascii_case("deliverable_codegen")
    }

    pub fn has_validator_guidance(&self) -> bool {
        !self.validator.trim().is_empty()
    }

    pub fn has_compose_guidance(&self) -> bool {
        !self.compose.trim().is_empty()
    }
}

const CONTRACT_HEADING: &str = "## orchestration contract";

/// Parse optional Orchestration Contract from assistant systemContext.
/// Returns the full systemContext unchanged (orchestrator still sees the block).
pub fn parse_orchestration_contract(system_context: &str) -> Option<OrchestrationContract> {
    let lower = system_context.to_lowercase();
    let start = lower.find(CONTRACT_HEADING)?;
    let section = &system_context[start..];
    parse_contract_section(section)
}

fn parse_contract_section(section: &str) -> Option<OrchestrationContract> {
    let mut contract = OrchestrationContract::default();
    let mut lines = section.lines();
    lines.next(); // skip heading

    let mut current_key: Option<String> = None;
    let mut current_value = String::new();
    let mut in_multiline = false;

    let flush = |key: &str, value: &str, contract: &mut OrchestrationContract| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return;
        }
        match key {
            "outcome_profile" => contract.outcome_profile = trimmed.to_string(),
            "orchestrator" => contract.orchestrator = trimmed.to_string(),
            "validator" => contract.validator = trimmed.to_string(),
            "compose" => contract.compose = trimmed.to_string(),
            "deliverable_fields" => {
                contract.deliverable_fields = trimmed
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            "workflow_type" => contract.workflow_type = trimmed.to_string(),
            "workflow_definition_slug" => {
                contract.workflow_definition_slug = trimmed.to_string();
            }
            _ => {}
        }
    };

    for line in lines {
        if !in_multiline {
            if let Some((key, rest)) = line.split_once(':') {
                if let Some(prev) = current_key.take() {
                    flush(&prev, &current_value, &mut contract);
                    current_value.clear();
                }
                let key = key.trim().to_lowercase();
                let rest = rest.trim();
                if rest == "|" {
                    current_key = Some(key);
                    in_multiline = true;
                    continue;
                }
                flush(&key, rest, &mut contract);
                continue;
            }
        } else if let Some(key) = current_key.as_ref() {
            if !line.is_empty()
                && !line.starts_with(' ')
                && !line.starts_with('\t')
                && line.contains(':')
                && !line.starts_with("- ")
            {
                let candidate = line.split(':').next().unwrap_or("").trim().to_lowercase();
                if matches!(
                    candidate.as_str(),
                    "outcome_profile"
                        | "orchestrator"
                        | "validator"
                        | "compose"
                        | "deliverable_fields"
                        | "workflow_type"
                        | "workflow_definition_slug"
                ) {
                    flush(key, &current_value, &mut contract);
                    current_value.clear();
                    in_multiline = false;
                    if let Some((k, rest)) = line.split_once(':') {
                        let k = k.trim().to_lowercase();
                        let rest = rest.trim();
                        if rest == "|" {
                            current_key = Some(k);
                            in_multiline = true;
                        } else {
                            flush(&k, rest, &mut contract);
                            current_key = None;
                        }
                    }
                    continue;
                }
            }
            if !current_value.is_empty() {
                current_value.push('\n');
            }
            current_value.push_str(line.trim_start());
        }
    }
    if let Some(prev) = current_key.take() {
        flush(&prev, &current_value, &mut contract);
    }

    if contract.outcome_profile.is_empty()
        && contract.orchestrator.is_empty()
        && contract.validator.is_empty()
        && contract.compose.is_empty()
        && contract.deliverable_fields.is_empty()
        && contract.workflow_type.is_empty()
        && contract.workflow_definition_slug.is_empty()
    {
        None
    } else {
        Some(contract)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_orchestration_contract() {
        let ctx = r#"OraMate scope rules here.

## Orchestration Contract

outcome_profile: deliverable_codegen

orchestrator: |
  Delegate to Interface Agent only.
  expected_outcome means tool success.

validator: |
  Compare user ask to agent attestation.

compose: |
  Present PL/SQL from tool deliverable.

deliverable_fields: apiCode, fallback
"#;
        let contract = parse_orchestration_contract(ctx).expect("contract");
        assert_eq!(contract.outcome_profile, "deliverable_codegen");
        assert!(contract.orchestrator.contains("Interface Agent"));
        assert!(contract.validator.contains("attestation"));
        assert!(contract.compose.contains("PL/SQL"));
        assert_eq!(contract.deliverable_fields, vec!["apiCode", "fallback"]);
    }

    #[test]
    fn parse_returns_none_without_heading() {
        assert!(parse_orchestration_contract("plain context").is_none());
    }

    #[test]
    fn parse_workflow_keys_with_multiline_orchestrator() {
        let ctx = r#"Scope rules.

## Orchestration Contract

workflow_type: purchase_requisition
workflow_definition_slug: purchase-requisition-v1

outcome_profile: lookup_facts

orchestrator: |
  Delegate vendor collection to procurement agent.

validator: |
  Check vendor name present.

compose: |
  Summarize requisition status.
"#;
        let contract = parse_orchestration_contract(ctx).expect("contract");
        assert_eq!(contract.workflow_type, "purchase_requisition");
        assert_eq!(contract.workflow_definition_slug, "purchase-requisition-v1");
        assert!(contract.has_workflow_binding());
        assert!(contract.orchestrator.contains("procurement"));
    }

    #[test]
    fn parse_workflow_keys_only_returns_some() {
        let ctx = r#"## Orchestration Contract

workflow_type: purchase_requisition
workflow_definition_slug: purchase-requisition-v1
"#;
        let contract = parse_orchestration_contract(ctx).expect("contract");
        assert_eq!(contract.workflow_type, "purchase_requisition");
        assert!(contract.has_workflow_binding());
    }
}
