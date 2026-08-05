//! Published JSON Schema for workflow definition bundle shape (Phase 9.1).

/// Canonical workflow definition JSON Schema (bundle: meta + schema + flow + gates).
pub const WORKFLOW_DEFINITION_SCHEMA: &str = include_str!("../workflow-definition.schema.json");

/// Parsed schema as JSON value (for tooling / tests).
pub fn workflow_definition_schema_value() -> serde_json::Value {
    serde_json::from_str(WORKFLOW_DEFINITION_SCHEMA)
        .expect("workflow-definition.schema.json must be valid JSON")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_parses_as_json() {
        let value = workflow_definition_schema_value();
        assert_eq!(
            value["$id"],
            "https://resmed.ai/schemas/workflow-definition/v1"
        );
        assert!(
            value["required"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("meta"))
        );
    }
}
