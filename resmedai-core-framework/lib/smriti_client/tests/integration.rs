//! Live Smriti round-trip — run with `just infra-up` and:
//! `cargo test -p smriti_client --test integration -- --ignored`

use smriti_client::{
    CreateRecordParams, DocumentQuery, GetRecordParams, HttpSmritiClient, SmritiClientConfig,
    SmritiDbClient, SmritiError, UpdateRecordParams, WorkflowDefinitionClient,
    WorkflowDefinitionLoader, push_workflow_definition,
};
use std::path::PathBuf;

#[tokio::test]
#[ignore = "requires live Smriti at SMRITI_URL"]
async fn live_create_get_update_query_round_trip() {
    let client = HttpSmritiClient::new(SmritiClientConfig::from_env()).expect("client");

    let collection = format!(
        "_smriti_client_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    );

    let created = client
        .create_record(&CreateRecordParams {
            collection: collection.clone(),
            payload: serde_json::json!({ "marker": "phase-9.0" }),
        })
        .await
        .expect("create");

    let record_id = created["_id"].as_str().expect("_id").to_string();

    let fetched = client
        .get_record(&GetRecordParams {
            collection: collection.clone(),
            record_id: record_id.clone(),
        })
        .await
        .expect("get");
    assert_eq!(fetched["marker"], "phase-9.0");

    client
        .update_record(&UpdateRecordParams {
            collection: collection.clone(),
            record_id: record_id.clone(),
            document: serde_json::json!({ "marker": "updated" }),
        })
        .await
        .expect("update");

    let results = client
        .query_records(&[DocumentQuery {
            collection_name: collection.clone(),
            query: serde_json::json!({ "_id": record_id }),
            options: None,
            select: None,
        }])
        .await
        .expect("query");

    assert!(!results.is_empty());
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[tokio::test]
#[ignore = "requires live Smriti at SMRITI_URL and workflowDefinitions indexes"]
async fn live_workflow_definition_push_and_read_round_trip() {
    let client = HttpSmritiClient::new(SmritiClientConfig::from_env()).expect("client");

    let split_dir = fixtures_dir().join("purchase-requisition-split");
    let mut def = WorkflowDefinitionLoader::load_from_dir(&split_dir).expect("load split");
    def.version = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_secs() as u32;

    let pushed = push_workflow_definition(&client, def.clone())
        .await
        .expect("push split fixture");

    let fetched = client
        .get_workflow_definition(&pushed.slug, Some(pushed.version))
        .await
        .expect("get by slug+version");
    assert_eq!(fetched.slug, pushed.slug);
    assert_eq!(fetched.version, pushed.version);
    assert_eq!(fetched.initial_stage, "collect_vendor");

    let bundle_dir = fixtures_dir().join("github-pr-create-bundle");
    let mut bundle_def = WorkflowDefinitionLoader::load_from_dir(&bundle_dir).expect("load bundle");
    bundle_def.version = pushed.version.saturating_add(1);

    push_workflow_definition(&client, bundle_def.clone())
        .await
        .expect("push bundle fixture");

    let listed = client
        .list_workflow_definitions(Some("github_pr_create"))
        .await
        .expect("list by type");
    assert!(listed.iter().any(|d| d.slug == bundle_def.slug));

    let dup_err = push_workflow_definition(&client, bundle_def)
        .await
        .unwrap_err();
    assert!(matches!(dup_err, SmritiError::DuplicateDefinition { .. }));
}
