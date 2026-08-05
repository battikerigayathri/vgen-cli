use chrono::Utc;
use serde_json::{Value, json};

use crate::error::Result;
use crate::workflow::instance::{
    FieldError, WorkflowAuditEntry, WorkflowAuthz, WorkflowInstance, WorkflowStatus,
};
use crate::workflow::normalize::flatten_mongo_extended_json;
use crate::workflow::types::WorkflowDefinition;

pub const WORKFLOW_INSTANCES_COLLECTION: &str = "workflowInstances";

/// Returns true when `s` is a 24-char hex string (Mongo ObjectId shape).
pub fn looks_like_object_id_hex(s: &str) -> bool {
    s.len() == 24 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Encode workflow identity fields for Smriti `/workflow/*` HTTP payloads.
///
/// **Do not use on `/workflow/create` JSON bodies** — Smriti deserializes `userId` /
/// `sessionId` as plain strings; extended JSON objects fail request parsing. Send plain
/// strings on create; Smriti's Mongo layer stores 24-char hex ids as ObjectId. On read,
/// [`normalize_workflow_instance_document`] flattens extended JSON before deserializing
/// [`WorkflowInstance`].
#[allow(dead_code)]
pub fn encode_smriti_identity_field(value: &str) -> Value {
    if looks_like_object_id_hex(value) {
        json!({ "$oid": value })
    } else {
        json!(value)
    }
}

/// Flatten Mongo extended JSON (`$oid`, `$date`) from a Smriti workflow instance document.
pub fn normalize_workflow_instance_document(value: Value) -> Value {
    flatten_mongo_extended_json(value)
}

/// Build the Mongo document payload for `POST /workflow/create`.
pub fn build_create_payload(
    authz: &WorkflowAuthz,
    definition: &WorkflowDefinition,
    initial_inputs: Option<Value>,
    missing_required: Vec<String>,
    source: &str,
) -> Value {
    let now = Utc::now().to_rfc3339();
    let mut payload = json!({
        "userId": authz.user_id,
        "definitionSlug": definition.slug,
        "definitionVersion": definition.version,
        "workflowType": definition.workflow_type,
        "stage": definition.initial_stage,
        "status": WorkflowStatus::InProgress.as_str(),
        "inputs": initial_inputs.unwrap_or_else(|| json!({})),
        "artifacts": json!({}),
        "externalRefs": json!({}),
        "validationErrors": [],
        "missingRequired": missing_required,
        "audit": [{
            "timestamp": now,
            "source": source,
            "action": "create",
            "details": null
        }],
    });

    if let Some(session_id) = &authz.session_id {
        payload["sessionId"] = json!(session_id);
    }
    if let Some(assistant_id) = &authz.assistant_id {
        payload["assistantId"] = json!(assistant_id);
    }

    payload
}

/// Serialize instance fields for Smriti `POST /workflow/patch` `$set`.
pub fn patch_set_payload(instance: &WorkflowInstance) -> Value {
    json!({
        "inputs": instance.inputs,
        "artifacts": instance.artifacts,
        "externalRefs": instance.external_refs,
        "stage": instance.stage,
        "status": instance.status.as_str(),
        "validationErrors": instance.validation_errors,
        "missingRequired": instance.missing_required,
        "updatedAt": instance.updated_at.to_rfc3339(),
    })
}

pub fn audit_entry_to_json(entry: &WorkflowAuditEntry) -> Value {
    json!({
        "timestamp": entry.timestamp.to_rfc3339(),
        "source": entry.source,
        "action": entry.action,
        "details": entry.details,
    })
}

#[allow(dead_code)]
pub fn field_errors_to_json(errors: &[FieldError]) -> Vec<Value> {
    errors
        .iter()
        .map(|e| json!({ "field": e.field, "message": e.message }))
        .collect()
}

#[allow(dead_code)]
pub fn instance_to_mongo(instance: &WorkflowInstance) -> Result<Value> {
    serde_json::to_value(instance).map_err(|e| {
        crate::error::SmritiError::InvalidResponse(format!(
            "failed to serialize WorkflowInstance: {e}"
        ))
    })
}

pub fn instance_from_mongo(value: Value) -> Result<WorkflowInstance> {
    WorkflowInstance::from_mongo_value(normalize_workflow_instance_document(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::instance::WorkflowInstance;

    #[test]
    fn build_create_payload_uses_plain_string_identities() {
        let authz = crate::workflow::instance::WorkflowAuthz {
            user_id: "64da57e1b6617711b67bba48".into(),
            session_id: Some("6a466fb2cf45701094745cad".into()),
            assistant_id: None,
        };
        let def = crate::workflow::loader::WorkflowDefinitionLoader::load_from_dir(
            &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/purchase-requisition-split"),
        )
        .unwrap();
        let payload = build_create_payload(&authz, &def, None, vec![], "test");
        assert_eq!(payload["userId"], json!("64da57e1b6617711b67bba48"));
        assert_eq!(payload["sessionId"], json!("6a466fb2cf45701094745cad"));
    }

    #[test]
    fn encode_smriti_identity_field_uses_oid_for_hex24() {
        let encoded = encode_smriti_identity_field("64da57e1b6617711b67bba48");
        assert_eq!(encoded, json!({ "$oid": "64da57e1b6617711b67bba48" }));
    }

    #[test]
    fn encode_smriti_identity_field_plain_for_non_oid() {
        let encoded = encode_smriti_identity_field("wf-c022d42d-a74c-4d73-b9d2-2f00e036eb29");
        assert_eq!(encoded, json!("wf-c022d42d-a74c-4d73-b9d2-2f00e036eb29"));
    }

    #[test]
    fn instance_from_mongo_flattens_user_id_object_id() {
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
            "audit": [],
            "createdAt": "2026-07-02T14:06:56.765480821+00:00",
            "updatedAt": "2026-07-02T14:06:56.765480821+00:00",
            "sessionId": "test-session"
        });

        let instance = instance_from_mongo(doc).unwrap();
        assert_eq!(instance.user_id, "64da57e1b6617711b67bba48");
    }
}
