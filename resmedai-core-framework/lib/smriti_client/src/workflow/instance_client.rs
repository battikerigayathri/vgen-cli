use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::OnceLock;

use crate::error::{Result, SmritiError};
use crate::http::HttpSmritiClient;
use crate::workflow::advance::{
    compute_missing_required, evaluate_done_when_with_validator, maybe_auto_advance,
    maybe_auto_advance_with_validator,
};
use crate::workflow::client::WorkflowDefinitionClient;
use crate::workflow::completeness::completeness_check;
use crate::workflow::definition_cache::WorkflowDefinitionCache;
use crate::workflow::instance::{
    ActiveWorkflowPolicy, CompletenessResult, WorkflowAuthz, WorkflowInstance, WorkflowListFilter,
    WorkflowPatch, WorkflowSnapshot, WorkflowStatus,
};
use crate::workflow::instance_normalize::{
    audit_entry_to_json, build_create_payload, instance_from_mongo, patch_set_payload,
};
use crate::workflow::merge::validate_and_merge_patch;
use crate::workflow::snapshot::project_workflow_snapshot;
use crate::workflow::types::StageDoneWhen;

fn shared_definition_cache() -> &'static WorkflowDefinitionCache {
    static CACHE: OnceLock<WorkflowDefinitionCache> = OnceLock::new();
    CACHE.get_or_init(WorkflowDefinitionCache::from_env)
}

pub fn guard_authz(authz: &WorkflowAuthz) -> Result<()> {
    if authz.user_id.is_empty() {
        return Err(SmritiError::EmptyUserId);
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowGetBody<'a> {
    workflow_id: &'a str,
    user_id: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowListBody {
    user_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    workflow_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skip: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowPatchBody {
    workflow_id: String,
    user_id: String,
    expected_version: u64,
    set: Value,
    audit_append: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowListData {
    #[allow(dead_code)]
    total: u64,
    items: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowConflictResponse {
    #[allow(dead_code)]
    code: String,
    expected_version: u64,
    actual_version: u64,
}

#[async_trait]
pub trait WorkflowInstanceClient: WorkflowDefinitionClient {
    async fn create_workflow_instance(
        &self,
        authz: &WorkflowAuthz,
        definition_slug: &str,
        definition_version: Option<u32>,
        initial_inputs: Option<Value>,
    ) -> Result<WorkflowInstance>;

    async fn get_workflow_instance(
        &self,
        authz: &WorkflowAuthz,
        workflow_id: &str,
    ) -> Result<WorkflowInstance>;

    async fn get_or_create_active_workflow(
        &self,
        authz: &WorkflowAuthz,
        definition_slug: &str,
        definition_version: Option<u32>,
        policy: ActiveWorkflowPolicy,
    ) -> Result<WorkflowInstance>;

    async fn list_workflow_instances(
        &self,
        authz: &WorkflowAuthz,
        filter: WorkflowListFilter,
    ) -> Result<Vec<WorkflowInstance>>;

    async fn patch_workflow(
        &self,
        authz: &WorkflowAuthz,
        workflow_id: &str,
        expected_version: u64,
        patch: WorkflowPatch,
        source: &str,
    ) -> Result<WorkflowInstance>;

    async fn workflow_snapshot(
        &self,
        authz: &WorkflowAuthz,
        workflow_id: &str,
    ) -> Result<WorkflowSnapshot>;

    async fn completeness_check(
        &self,
        authz: &WorkflowAuthz,
        workflow_id: &str,
        for_stage: Option<&str>,
    ) -> Result<CompletenessResult>;

    /// Platform-only: current stage must have `done_when = ValidatorPass`.
    /// Runs completeness_check; on success advances one hop via validator pass signal.
    async fn confirm_validator_pass_and_advance(
        &self,
        authz: &WorkflowAuthz,
        workflow_id: &str,
        expected_version: u64,
        source: &str,
    ) -> Result<WorkflowInstance>;
}

#[async_trait]
impl WorkflowInstanceClient for HttpSmritiClient {
    async fn create_workflow_instance(
        &self,
        authz: &WorkflowAuthz,
        definition_slug: &str,
        definition_version: Option<u32>,
        initial_inputs: Option<Value>,
    ) -> Result<WorkflowInstance> {
        guard_authz(authz)?;

        let definition = self
            .get_workflow_definition(definition_slug, definition_version)
            .await?;

        let mut preview = WorkflowInstance {
            mongo_id: None,
            workflow_id: String::new(),
            definition_slug: definition.slug.clone(),
            definition_version: definition.version,
            workflow_type: definition.workflow_type.clone(),
            user_id: authz.user_id.clone(),
            session_id: authz.session_id.clone(),
            assistant_id: authz.assistant_id.clone(),
            stage: definition.initial_stage.clone(),
            status: WorkflowStatus::InProgress,
            inputs: initial_inputs.clone().unwrap_or_else(|| json!({})),
            artifacts: json!({}),
            validation_errors: vec![],
            missing_required: vec![],
            version: 1,
            external_refs: json!({}),
            audit: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        preview.missing_required = compute_missing_required(&definition, &preview, &preview.stage);

        let payload = build_create_payload(
            authz,
            &definition,
            initial_inputs,
            preview.missing_required.clone(),
            "sdk",
        );

        let data = self.post_workflow("/workflow/create", &payload).await?;
        instance_from_mongo(data)
    }

    async fn get_workflow_instance(
        &self,
        authz: &WorkflowAuthz,
        workflow_id: &str,
    ) -> Result<WorkflowInstance> {
        guard_authz(authz)?;
        let data = self
            .post_workflow(
                "/workflow/get",
                &WorkflowGetBody {
                    workflow_id,
                    user_id: &authz.user_id,
                },
            )
            .await?;
        let instance = instance_from_mongo(data)?;
        if instance.user_id != authz.user_id {
            return Err(SmritiError::Unauthorized {
                user_id: authz.user_id.clone(),
            });
        }
        Ok(instance)
    }

    async fn get_or_create_active_workflow(
        &self,
        authz: &WorkflowAuthz,
        definition_slug: &str,
        definition_version: Option<u32>,
        policy: ActiveWorkflowPolicy,
    ) -> Result<WorkflowInstance> {
        guard_authz(authz)?;

        match policy {
            ActiveWorkflowPolicy::AlwaysNew => {
                self.create_workflow_instance(authz, definition_slug, definition_version, None)
                    .await
            }
            ActiveWorkflowPolicy::ExplicitWorkflowId(id) => {
                match self.get_workflow_instance(authz, &id).await {
                    Ok(instance) => Ok(instance),
                    Err(SmritiError::WorkflowNotFound { .. }) => {
                        self.create_workflow_instance(
                            authz,
                            definition_slug,
                            definition_version,
                            None,
                        )
                        .await
                    }
                    Err(e) => Err(e),
                }
            }
            ActiveWorkflowPolicy::SessionBound => {
                let session_id = authz.session_id.as_deref().ok_or_else(|| {
                    SmritiError::InvalidRequest(
                        "SessionBound policy requires authz.session_id".into(),
                    )
                })?;

                let definition = self
                    .get_workflow_definition(definition_slug, definition_version)
                    .await?;

                let existing = self
                    .list_workflow_instances(
                        authz,
                        WorkflowListFilter {
                            workflow_type: Some(definition.workflow_type.clone()),
                            status: Some(WorkflowStatus::InProgress),
                            session_id: Some(session_id.to_string()),
                            limit: Some(1),
                            skip: None,
                        },
                    )
                    .await?;

                if let Some(instance) = existing.into_iter().next() {
                    return Ok(instance);
                }

                self.create_workflow_instance(authz, definition_slug, definition_version, None)
                    .await
            }
        }
    }

    async fn list_workflow_instances(
        &self,
        authz: &WorkflowAuthz,
        filter: WorkflowListFilter,
    ) -> Result<Vec<WorkflowInstance>> {
        guard_authz(authz)?;

        let body = WorkflowListBody {
            user_id: authz.user_id.clone(),
            workflow_type: filter.workflow_type,
            status: filter.status.map(|s| s.as_str().to_string()),
            session_id: filter.session_id,
            limit: filter.limit,
            skip: filter.skip,
        };

        let data = self.post_workflow("/workflow/list", &body).await?;
        let list: WorkflowListData = serde_json::from_value(data).map_err(|e| {
            SmritiError::InvalidResponse(format!("failed to parse workflow list: {e}"))
        })?;

        list.items.into_iter().map(instance_from_mongo).collect()
    }

    async fn patch_workflow(
        &self,
        authz: &WorkflowAuthz,
        workflow_id: &str,
        expected_version: u64,
        patch: WorkflowPatch,
        source: &str,
    ) -> Result<WorkflowInstance> {
        guard_authz(authz)?;

        let mut instance = self.get_workflow_instance(authz, workflow_id).await?;

        if instance.version != expected_version {
            return Err(SmritiError::WorkflowConflict {
                workflow_id: workflow_id.to_string(),
                expected: expected_version,
                actual: instance.version,
            });
        }

        let definition = shared_definition_cache()
            .get_or_load(self, &instance.definition_slug, instance.definition_version)
            .await?;

        let (inputs, artifacts, external_refs, validation_errors) = validate_and_merge_patch(
            &definition,
            &instance.inputs,
            &instance.artifacts,
            &instance.external_refs,
            patch.inputs.as_ref(),
            patch.artifacts.as_ref(),
            patch.external_refs.as_ref(),
        )?;

        instance.inputs = inputs;
        instance.artifacts = artifacts;
        instance.external_refs = external_refs;
        instance.validation_errors = validation_errors.clone();
        instance.missing_required =
            compute_missing_required(&definition, &instance, &instance.stage);
        instance.updated_at = Utc::now();

        let mut audit_append = vec![audit_entry_to_json(
            &crate::workflow::instance::WorkflowAuditEntry {
                timestamp: Utc::now(),
                source: source.to_string(),
                action: "patch".into(),
                details: patch_details(&patch),
            },
        )];

        if let Some(advance_audit) = maybe_auto_advance(&mut instance, &definition, "platform") {
            audit_append.push(audit_entry_to_json(&advance_audit));
            instance.missing_required =
                compute_missing_required(&definition, &instance, &instance.stage);
        }

        let persisted = self
            .persist_workflow_patch(authz, &instance, expected_version, audit_append)
            .await?;

        Ok(persisted)
    }

    async fn workflow_snapshot(
        &self,
        authz: &WorkflowAuthz,
        workflow_id: &str,
    ) -> Result<WorkflowSnapshot> {
        guard_authz(authz)?;
        let instance = self.get_workflow_instance(authz, workflow_id).await?;
        let definition = shared_definition_cache()
            .get_or_load(self, &instance.definition_slug, instance.definition_version)
            .await?;

        project_workflow_snapshot(&definition, &instance).ok_or_else(|| {
            SmritiError::InvalidResponse(format!(
                "unknown stage {} on workflow {}",
                instance.stage, workflow_id
            ))
        })
    }

    async fn completeness_check(
        &self,
        authz: &WorkflowAuthz,
        workflow_id: &str,
        for_stage: Option<&str>,
    ) -> Result<CompletenessResult> {
        guard_authz(authz)?;
        let instance = self.get_workflow_instance(authz, workflow_id).await?;
        let definition = shared_definition_cache()
            .get_or_load(self, &instance.definition_slug, instance.definition_version)
            .await?;

        Ok(completeness_check(
            &definition,
            &instance,
            for_stage,
            &instance.validation_errors,
        ))
    }

    async fn confirm_validator_pass_and_advance(
        &self,
        authz: &WorkflowAuthz,
        workflow_id: &str,
        expected_version: u64,
        source: &str,
    ) -> Result<WorkflowInstance> {
        guard_authz(authz)?;

        let mut instance = self.get_workflow_instance(authz, workflow_id).await?;

        if instance.version != expected_version {
            return Err(SmritiError::WorkflowConflict {
                workflow_id: workflow_id.to_string(),
                expected: expected_version,
                actual: instance.version,
            });
        }

        let definition = shared_definition_cache()
            .get_or_load(self, &instance.definition_slug, instance.definition_version)
            .await?;

        let stage_def = definition.stage_by_id(&instance.stage).ok_or_else(|| {
            SmritiError::InvalidResponse(format!(
                "unknown stage {} on workflow {}",
                instance.stage, workflow_id
            ))
        })?;

        if !matches!(stage_def.done_when, StageDoneWhen::ValidatorPass) {
            return Err(SmritiError::InvalidRequest(format!(
                "stage {} is not validator_pass (done_when={:?})",
                instance.stage, stage_def.done_when
            )));
        }

        let check = completeness_check(&definition, &instance, None, &instance.validation_errors);
        instance.missing_required = check.missing_required.clone();
        if !check.complete {
            return Err(SmritiError::InvalidRequest(format!(
                "workflow incomplete: missing {}",
                check.missing_required.join(", ")
            )));
        }

        if !evaluate_done_when_with_validator(&instance, stage_def, true) {
            return Err(SmritiError::InvalidRequest(
                "validator pass preconditions not met".into(),
            ));
        }

        let mut audit_append = vec![audit_entry_to_json(
            &crate::workflow::instance::WorkflowAuditEntry {
                timestamp: Utc::now(),
                source: source.to_string(),
                action: "validator_pass".into(),
                details: None,
            },
        )];

        if let Some(advance_audit) =
            maybe_auto_advance_with_validator(&mut instance, &definition, source, true)
        {
            audit_append.push(audit_entry_to_json(&advance_audit));
            instance.missing_required =
                compute_missing_required(&definition, &instance, &instance.stage);
        }

        instance.updated_at = Utc::now();

        self.persist_workflow_patch(authz, &instance, expected_version, audit_append)
            .await
    }
}

impl HttpSmritiClient {
    async fn post_workflow(&self, path: &str, body: &impl Serialize) -> Result<Value> {
        match self.post_workflow_json(path, body).await {
            Ok(data) => Ok(data),
            Err(SmritiError::Http { status: 404, .. }) => Err(SmritiError::WorkflowNotFound {
                workflow_id: extract_workflow_id(body),
            }),
            Err(e) => Err(e),
        }
    }

    async fn persist_workflow_patch(
        &self,
        authz: &WorkflowAuthz,
        instance: &WorkflowInstance,
        expected_version: u64,
        audit_append: Vec<Value>,
    ) -> Result<WorkflowInstance> {
        let body = WorkflowPatchBody {
            workflow_id: instance.workflow_id.clone(),
            user_id: authz.user_id.clone(),
            expected_version,
            set: patch_set_payload(instance),
            audit_append,
        };

        match self.post_workflow_json("/workflow/patch", &body).await {
            Ok(data) => instance_from_mongo(data),
            Err(SmritiError::Http {
                status: 409, body, ..
            }) => {
                if let Ok(conflict) = serde_json::from_str::<WorkflowConflictResponse>(&body) {
                    return Err(SmritiError::WorkflowConflict {
                        workflow_id: instance.workflow_id.clone(),
                        expected: conflict.expected_version,
                        actual: conflict.actual_version,
                    });
                }
                Err(SmritiError::WorkflowConflict {
                    workflow_id: instance.workflow_id.clone(),
                    expected: expected_version,
                    actual: instance.version,
                })
            }
            Err(SmritiError::Http { status: 404, .. }) => Err(SmritiError::WorkflowNotFound {
                workflow_id: instance.workflow_id.clone(),
            }),
            Err(e) => Err(e),
        }
    }
}

fn patch_details(patch: &WorkflowPatch) -> Option<Value> {
    let mut keys = Vec::new();
    if let Some(obj) = patch.inputs.as_ref().and_then(|v| v.as_object()) {
        keys.extend(obj.keys().cloned());
    }
    if let Some(obj) = patch.artifacts.as_ref().and_then(|v| v.as_object()) {
        keys.extend(obj.keys().cloned());
    }
    if keys.is_empty() {
        None
    } else {
        Some(json!({ "keys": keys }))
    }
}

fn extract_workflow_id(body: &impl Serialize) -> String {
    serde_json::to_value(body)
        .ok()
        .and_then(|v| {
            v.get("workflowId")
                .and_then(|id| id.as_str())
                .map(String::from)
        })
        .unwrap_or_else(|| "unknown".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_authz_rejects_empty_user() {
        let err = guard_authz(&WorkflowAuthz {
            user_id: String::new(),
            session_id: None,
            assistant_id: None,
        })
        .unwrap_err();
        assert!(matches!(err, SmritiError::EmptyUserId));
    }
}
