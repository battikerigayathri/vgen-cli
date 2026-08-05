use chrono::Utc;
use serde_json::{Value, json};

use crate::error::Result;
use crate::workflow::types::WorkflowDefinition;

/// Mongo collection for immutable workflow templates.
pub const WORKFLOW_DEFINITIONS_COLLECTION: &str = "workflowDefinitions";

/// Flatten Mongo extended JSON fields before serde deserialize.
///
/// Handles `_id` / `$oid` ObjectIds and `$date` timestamps anywhere in the tree.
/// Smriti may store 24-char hex `userId` values as BSON ObjectId; without flattening,
/// `WorkflowInstance.user_id` deserialization fails with "map, expected string".
pub fn flatten_mongo_extended_json(value: Value) -> Value {
    flatten_mongo_extended_json_inner(value)
}

fn flatten_mongo_extended_json_inner(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            if let Some(oid) = map.get("$oid") {
                if let Some(s) = oid.as_str() {
                    return Value::String(s.to_string());
                }
                if let Some(n) = oid.as_i64() {
                    return Value::String(n.to_string());
                }
            }
            if let Some(date) = map.get("$date") {
                if let Some(s) = date.as_str() {
                    return Value::String(s.to_string());
                }
                if let Some(n) = date.as_i64() {
                    if let Some(dt) = chrono::DateTime::from_timestamp_millis(n) {
                        return Value::String(dt.to_rfc3339());
                    }
                }
                if let Some(obj) = date.as_object() {
                    if let Some(n) = obj.get("$numberLong").and_then(|v| v.as_str()) {
                        if let Ok(ms) = n.parse::<i64>() {
                            if let Some(dt) = chrono::DateTime::from_timestamp_millis(ms) {
                                return Value::String(dt.to_rfc3339());
                            }
                        }
                    }
                }
            }
            if let Some(n) = map.get("$numberLong").and_then(|v| v.as_str()) {
                if let Ok(n) = n.parse::<i64>() {
                    return json!(n);
                }
            }
            if let Some(n) = map.get("$numberInt").and_then(|v| v.as_i64()) {
                return json!(n);
            }
            if let Some(n) = map.get("$numberInt").and_then(|v| v.as_str()) {
                if let Ok(n) = n.parse::<i64>() {
                    return json!(n);
                }
            }

            let mut out = serde_json::Map::new();
            for (k, v) in map {
                out.insert(k, flatten_mongo_extended_json_inner(v));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(flatten_mongo_extended_json_inner)
                .collect(),
        ),
        other => other,
    }
}

/// Normalize a validated definition into canonical Mongo JSON (camelCase keys).
pub fn normalize_to_mongo(def: &WorkflowDefinition) -> Result<Value> {
    let now = Utc::now();
    let mut payload = serde_json::to_value(def).map_err(|e| {
        crate::error::SmritiError::InvalidResponse(format!(
            "failed to serialize WorkflowDefinition: {e}"
        ))
    })?;

    if let Value::Object(map) = &mut payload {
        map.insert(
            "createdAt".to_string(),
            json!(def.created_at.unwrap_or(now).to_rfc3339()),
        );
        map.insert(
            "updatedAt".to_string(),
            json!(def.updated_at.unwrap_or(now).to_rfc3339()),
        );
        map.insert(
            "sourceFormat".to_string(),
            json!(serde_json::to_value(def.source_format).map_err(|e| {
                crate::error::SmritiError::InvalidResponse(format!(
                    "failed to serialize sourceFormat: {e}"
                ))
            })?),
        );
    }

    Ok(payload)
}

/// Apply push timestamps on the in-memory model and return Mongo payload.
pub fn prepare_for_push(mut def: WorkflowDefinition) -> Result<(WorkflowDefinition, Value)> {
    let now = Utc::now();
    def.created_at = Some(now);
    def.updated_at = Some(now);
    let payload = normalize_to_mongo(&def)?;
    Ok((def, payload))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::loader::WorkflowDefinitionLoader;
    use crate::workflow::types::SourceFormat;
    use std::path::PathBuf;

    fn split_fixture() -> WorkflowDefinition {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/purchase-requisition-split");
        WorkflowDefinitionLoader::load_from_dir(&dir).unwrap()
    }

    #[test]
    fn flatten_mongo_extended_json_object_id() {
        use crate::workflow::types::WorkflowDefinition;

        let doc = json!({
            "_id": { "$oid": "6a43b5549aa5ea97cdfaa364" },
            "slug": "oracle-purchase-requisition-v1",
            "name": "Oracle Purchase Requisition",
            "workflowType": "oracle_purchase_requisition",
            "version": 4,
            "fields": [],
            "stages": [{
                "id": "collect_requester",
                "label": "Collect Requester",
                "kind": "collect",
                "doneWhen": ["requesterId"],
                "isTerminal": false
            }],
            "gates": [],
            "initialStage": "collect_requester",
            "sourceFormat": "split_yaml",
            "createdAt": "2026-06-30T12:23:48.522602+00:00",
            "updatedAt": "2026-06-30T12:23:48.522602+00:00"
        });

        let def = WorkflowDefinition::from_mongo_value(doc).unwrap();
        assert_eq!(def.id.as_deref(), Some("6a43b5549aa5ea97cdfaa364"));
        assert_eq!(def.slug, "oracle-purchase-requisition-v1");
    }

    #[test]
    fn flatten_mongo_user_id_object_id_for_workflow_instance() {
        use crate::workflow::instance::WorkflowInstance;

        // Live Smriti /workflow/create response shape when userId is 24-char hex.
        let doc = json!({
            "_id": { "$oid": "6a467080cf45701094745cb2" },
            "workflowId": "wf-c022d42d-a74c-4d73-b9d2-2f00e036eb29",
            "definitionSlug": "oracle-purchase-requisition-v1",
            "definitionVersion": 1,
            "workflowType": "oracle_purchase_requisition",
            "userId": { "$oid": "64da57e1b6617711b67bba48" },
            "stage": "collect_requester",
            "status": "in_progress",
            "inputs": {},
            "artifacts": {},
            "externalRefs": {},
            "validationErrors": [],
            "missingRequired": ["requesterId"],
            "version": 1,
            "audit": [{
                "timestamp": "2026-07-02T13:41:33.182Z",
                "source": "test",
                "action": "create",
                "details": null
            }],
            "createdAt": "2026-07-02T14:06:56.765480821+00:00",
            "updatedAt": "2026-07-02T14:06:56.765480821+00:00",
            "sessionId": "test-session"
        });

        let instance = WorkflowInstance::from_mongo_value(doc).unwrap();
        assert_eq!(instance.user_id, "64da57e1b6617711b67bba48");
        assert_eq!(instance.workflow_id, "wf-c022d42d-a74c-4d73-b9d2-2f00e036eb29");
    }

    #[test]
    fn flatten_mongo_number_long_for_version() {
        use crate::workflow::instance::WorkflowInstance;

        let doc = json!({
            "workflowId": "wf-1",
            "definitionSlug": "oracle-purchase-requisition-v1",
            "definitionVersion": 1,
            "workflowType": "oracle_purchase_requisition",
            "userId": "64da57e1b6617711b67bba48",
            "stage": "collect_requester",
            "status": "in_progress",
            "inputs": {},
            "artifacts": {},
            "externalRefs": {},
            "validationErrors": [],
            "missingRequired": [],
            "version": { "$numberLong": "1" },
            "audit": [],
            "createdAt": "2026-07-02T14:06:56.765480821+00:00",
            "updatedAt": "2026-07-02T14:06:56.765480821+00:00"
        });

        let instance = WorkflowInstance::from_mongo_value(doc).unwrap();
        assert_eq!(instance.version, 1);
    }

    #[test]
    fn normalize_uses_camel_case_keys() {
        let def = split_fixture();
        let mongo = normalize_to_mongo(&def).unwrap();
        assert_eq!(mongo["slug"], "purchase-requisition-v1");
        assert_eq!(mongo["workflowType"], "purchase_requisition");
        assert_eq!(mongo["initialStage"], "collect_vendor");
        assert!(mongo.get("createdAt").is_some());
        assert!(mongo.get("updatedAt").is_some());
        assert_eq!(mongo["sourceFormat"], "split_yaml");
    }

    #[test]
    fn prepare_for_push_sets_timestamps() {
        let def = split_fixture();
        let (prepared, payload) = prepare_for_push(def).unwrap();
        assert!(prepared.created_at.is_some());
        assert!(prepared.updated_at.is_some());
        assert_eq!(prepared.source_format, SourceFormat::SplitYaml);
        assert_eq!(payload["slug"], "purchase-requisition-v1");
    }
}
