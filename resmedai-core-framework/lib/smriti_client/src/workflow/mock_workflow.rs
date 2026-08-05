use std::collections::HashMap;
use std::sync::Mutex;

use crate::error::{Result, SmritiError};
use crate::workflow::instance::WorkflowInstance;
use crate::workflow::instance_normalize::instance_from_mongo;

type InstanceKey = (String, String);

/// In-memory workflow instance store with CAS semantics for unit tests.
#[derive(Default)]
pub struct MockWorkflowStore {
    instances: Mutex<HashMap<InstanceKey, WorkflowInstance>>,
}

impl MockWorkflowStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, instance: WorkflowInstance) {
        let key = (instance.user_id.clone(), instance.workflow_id.clone());
        self.instances
            .lock()
            .expect("mock workflow lock")
            .insert(key, instance);
    }

    pub fn get(&self, user_id: &str, workflow_id: &str) -> Option<WorkflowInstance> {
        self.instances
            .lock()
            .expect("mock workflow lock")
            .get(&(user_id.to_string(), workflow_id.to_string()))
            .cloned()
    }

    pub fn create_from_mongo(&self, doc: serde_json::Value) -> Result<WorkflowInstance> {
        let instance = instance_from_mongo(doc)?;
        self.insert(instance.clone());
        Ok(instance)
    }

    pub fn patch_cas(
        &self,
        user_id: &str,
        workflow_id: &str,
        expected_version: u64,
        updated: WorkflowInstance,
        audit_append: Vec<serde_json::Value>,
    ) -> Result<WorkflowInstance> {
        let key = (user_id.to_string(), workflow_id.to_string());
        let mut guard = self.instances.lock().expect("mock workflow lock");

        let Some(current) = guard.get(&key) else {
            return Err(SmritiError::WorkflowNotFound {
                workflow_id: workflow_id.to_string(),
            });
        };

        if current.version != expected_version {
            return Err(SmritiError::WorkflowConflict {
                workflow_id: workflow_id.to_string(),
                expected: expected_version,
                actual: current.version,
            });
        }

        let mut next = updated;
        next.version = expected_version + 1;
        for entry in audit_append {
            if let Ok(audit) = serde_json::from_value(entry) {
                next.audit.push(audit);
            }
        }
        guard.insert(key, next.clone());
        Ok(next)
    }

    pub fn list(
        &self,
        user_id: &str,
        workflow_type: Option<&str>,
        status: Option<&str>,
        session_id: Option<&str>,
        limit: u64,
    ) -> Vec<WorkflowInstance> {
        let guard = self.instances.lock().expect("mock workflow lock");
        let mut items: Vec<WorkflowInstance> = guard
            .values()
            .filter(|i| i.user_id == user_id)
            .filter(|i| {
                workflow_type.is_none_or(|t| i.workflow_type == t)
                    && status.is_none_or(|s| i.status.as_str() == s)
                    && session_id.is_none_or(|sid| i.session_id.as_deref() == Some(sid))
            })
            .cloned()
            .collect();
        items.sort_by_key(|b| std::cmp::Reverse(b.updated_at));
        items.truncate(limit as usize);
        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::instance::WorkflowStatus;
    use chrono::Utc;
    use serde_json::json;

    fn sample_instance(version: u64) -> WorkflowInstance {
        WorkflowInstance {
            mongo_id: None,
            workflow_id: "wf-1".into(),
            definition_slug: "test".into(),
            definition_version: 1,
            workflow_type: "test".into(),
            user_id: "user-1".into(),
            session_id: None,
            assistant_id: None,
            stage: "collect".into(),
            status: WorkflowStatus::InProgress,
            inputs: json!({}),
            artifacts: json!({}),
            validation_errors: vec![],
            missing_required: vec![],
            version,
            external_refs: json!({}),
            audit: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn cas_patch_bumps_version() {
        let store = MockWorkflowStore::new();
        store.insert(sample_instance(1));

        let mut updated = sample_instance(1);
        updated.inputs = json!({ "vendorId": "V-1" });
        let result = store
            .patch_cas("user-1", "wf-1", 1, updated, vec![])
            .unwrap();
        assert_eq!(result.version, 2);
        assert_eq!(result.inputs["vendorId"], "V-1");
    }

    #[test]
    fn cas_patch_rejects_stale_version() {
        let store = MockWorkflowStore::new();
        store.insert(sample_instance(2));

        let updated = sample_instance(2);
        let err = store
            .patch_cas("user-1", "wf-1", 1, updated, vec![])
            .unwrap_err();
        assert!(matches!(
            err,
            SmritiError::WorkflowConflict {
                expected: 1,
                actual: 2,
                ..
            }
        ));
    }
}
