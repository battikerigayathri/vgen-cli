//! Workflow instance integration tests — wiremock HTTP contract + optional live Smriti.

use smriti_client::{
    HttpSmritiClient, SmritiClientConfig, SmritiError, WorkflowAuthz, WorkflowDefinitionLoader,
    WorkflowInstanceClient, WorkflowPatch, WorkflowStatus, guard_authz, push_workflow_definition,
};
use std::path::PathBuf;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn test_authz() -> WorkflowAuthz {
    WorkflowAuthz {
        user_id: "user-test-1".into(),
        session_id: Some("sess-test".into()),
        assistant_id: None,
    }
}

fn sample_instance_doc(workflow_id: &str, version: u64, stage: &str) -> serde_json::Value {
    serde_json::json!({
        "workflowId": workflow_id,
        "definitionSlug": "purchase-requisition-v1",
        "definitionVersion": 1,
        "workflowType": "purchase_requisition",
        "userId": "user-test-1",
        "sessionId": "sess-test",
        "stage": stage,
        "status": "in_progress",
        "inputs": {},
        "artifacts": {},
        "validationErrors": [],
        "missingRequired": ["vendorId"],
        "version": version,
        "externalRefs": {},
        "audit": [],
        "createdAt": "2026-06-26T11:59:00Z",
        "updatedAt": "2026-06-26T11:59:00Z"
    })
}

fn definition_doc() -> serde_json::Value {
    let dir = fixtures_dir().join("purchase-requisition-split");
    let def = WorkflowDefinitionLoader::load_from_dir(&dir).unwrap();
    let fields = serde_json::to_value(&def.fields).unwrap();
    let stages = serde_json::to_value(&def.stages).unwrap();
    serde_json::json!({
        "_id": "def-1",
        "slug": def.slug,
        "name": def.name,
        "workflowType": def.workflow_type,
        "version": def.version,
        "fields": fields,
        "stages": stages,
        "gates": [],
        "initialStage": def.initial_stage,
        "sourceFormat": "split_yaml",
        "createdAt": "2026-06-26T12:00:00Z",
        "updatedAt": "2026-06-26T12:00:00Z"
    })
}

fn test_client(base_url: &str) -> HttpSmritiClient {
    HttpSmritiClient::new(SmritiClientConfig {
        base_url: base_url.to_string(),
        timeout: std::time::Duration::from_secs(5),
    })
    .expect("client")
}

#[tokio::test]
async fn wiremock_patch_cas_conflict_returns_workflow_conflict() {
    let server = MockServer::start().await;
    let workflow_id = "wf-cas-test";

    Mock::given(method("POST"))
        .and(path("/workflow/get"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": sample_instance_doc(workflow_id, 1, "collect_vendor")
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/db/query-records"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [[definition_doc()]]
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/workflow/patch"))
        .respond_with(ResponseTemplate::new(409).set_body_json(serde_json::json!({
            "code": "version_conflict",
            "expectedVersion": 1,
            "actualVersion": 2
        })))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let authz = test_authz();

    let err = client
        .patch_workflow(
            &authz,
            workflow_id,
            1,
            WorkflowPatch {
                inputs: Some(serde_json::json!({ "vendorId": "V-1" })),
                artifacts: None,
                external_refs: None,
            },
            "wiremock_test",
        )
        .await
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

#[tokio::test]
async fn wiremock_create_patch_auto_advance_happy_path() {
    let server = MockServer::start().await;
    let workflow_id = "wf-happy";

    Mock::given(method("POST"))
        .and(path("/db/query-records"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [[definition_doc()]]
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/workflow/create"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": sample_instance_doc(workflow_id, 1, "collect_vendor")
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/workflow/get"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": sample_instance_doc(workflow_id, 1, "collect_vendor")
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/workflow/patch"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": sample_instance_doc(workflow_id, 2, "collect_line_items")
        })))
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let authz = test_authz();

    let created = client
        .create_workflow_instance(&authz, "purchase-requisition-v1", Some(1), None)
        .await
        .unwrap();
    assert_eq!(created.stage, "collect_vendor");
    assert_eq!(created.version, 1);

    let patched = client
        .patch_workflow(
            &authz,
            workflow_id,
            1,
            WorkflowPatch {
                inputs: Some(serde_json::json!({ "vendorId": "V-1" })),
                artifacts: None,
                external_refs: None,
            },
            "wiremock_test",
        )
        .await
        .unwrap();
    assert_eq!(patched.stage, "collect_line_items");
    assert_eq!(patched.version, 2);
}

#[tokio::test]
async fn guard_authz_rejects_empty_user_id() {
    let err = guard_authz(&WorkflowAuthz {
        user_id: String::new(),
        session_id: None,
        assistant_id: None,
    })
    .unwrap_err();
    assert!(matches!(err, SmritiError::EmptyUserId));
}

#[tokio::test]
#[ignore = "requires live Smriti + indexes + pushed definition"]
async fn live_create_patch_auto_advance_and_cas_conflict() {
    let client = HttpSmritiClient::new(SmritiClientConfig::from_env()).expect("client");
    let authz = test_authz();

    let split_dir = fixtures_dir().join("purchase-requisition-split");
    let mut def = WorkflowDefinitionLoader::load_from_dir(&split_dir).expect("load");
    def.version = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_secs() as u32;

    push_workflow_definition(&client, def.clone())
        .await
        .expect("push definition");

    let instance = client
        .create_workflow_instance(&authz, &def.slug, Some(def.version), None)
        .await
        .expect("create instance");

    assert_eq!(instance.stage, "collect_vendor");
    assert_eq!(instance.status, WorkflowStatus::InProgress);
    assert!(instance.missing_required.contains(&"vendorId".to_string()));

    let version_before = instance.version;
    let advanced = client
        .patch_workflow(
            &authz,
            &instance.workflow_id,
            version_before,
            WorkflowPatch {
                inputs: Some(serde_json::json!({ "vendorId": "V-1" })),
                artifacts: None,
                external_refs: None,
            },
            "integration_test",
        )
        .await
        .expect("patch vendorId");

    assert_eq!(advanced.stage, "collect_line_items");
    assert_eq!(advanced.version, version_before + 1);

    let conflict = client
        .patch_workflow(
            &authz,
            &instance.workflow_id,
            version_before,
            WorkflowPatch {
                inputs: Some(serde_json::json!({ "vendorId": "V-stale" })),
                artifacts: None,
                external_refs: None,
            },
            "integration_test",
        )
        .await
        .unwrap_err();

    assert!(matches!(conflict, SmritiError::WorkflowConflict { .. }));
}
