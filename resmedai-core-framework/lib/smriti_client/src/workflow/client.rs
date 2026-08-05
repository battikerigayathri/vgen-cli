use async_trait::async_trait;
use std::collections::HashMap;

use crate::client::SmritiDbClient;
use crate::db::{DocumentQuery, QueryOptions, SortOrder};
use crate::error::{Result, SmritiError};
use crate::workflow::normalize::WORKFLOW_DEFINITIONS_COLLECTION;
use crate::workflow::types::WorkflowDefinition;

/// Default cap for untyped definition listing.
pub const LIST_WORKFLOW_DEFINITIONS_DEFAULT_LIMIT: u64 = 100;

#[async_trait]
pub trait WorkflowDefinitionClient: SmritiDbClient {
    async fn get_workflow_definition(
        &self,
        slug: &str,
        version: Option<u32>,
    ) -> Result<WorkflowDefinition>;

    async fn list_workflow_definitions(
        &self,
        workflow_type: Option<&str>,
    ) -> Result<Vec<WorkflowDefinition>>;
}

#[async_trait]
impl<T: SmritiDbClient + ?Sized> WorkflowDefinitionClient for T {
    async fn get_workflow_definition(
        &self,
        slug: &str,
        version: Option<u32>,
    ) -> Result<WorkflowDefinition> {
        let mut query = serde_json::json!({ "slug": slug });
        let mut sort_by = None;
        if let Some(v) = version {
            query["version"] = serde_json::json!(v);
        } else {
            let mut sort = HashMap::new();
            sort.insert("version".to_string(), SortOrder::Desc);
            sort_by = Some(sort);
        }

        let results = self
            .query_records(&[DocumentQuery {
                collection_name: WORKFLOW_DEFINITIONS_COLLECTION.to_string(),
                query,
                options: Some(QueryOptions {
                    populate: None,
                    sort_by,
                    limit: Some(1),
                    skip: None,
                    select: None,
                }),
                select: None,
            }])
            .await?;

        let doc = first_query_document(&results).ok_or_else(|| SmritiError::NotFound {
            collection: WORKFLOW_DEFINITIONS_COLLECTION.to_string(),
            id: format!("{slug}:v{version:?}"),
        })?;

        WorkflowDefinition::from_mongo_value(doc)
    }

    async fn list_workflow_definitions(
        &self,
        workflow_type: Option<&str>,
    ) -> Result<Vec<WorkflowDefinition>> {
        let query = match workflow_type {
            Some(t) => serde_json::json!({ "workflowType": t }),
            None => serde_json::json!({}),
        };

        let mut sort_by = HashMap::new();
        if workflow_type.is_some() {
            sort_by.insert("version".to_string(), SortOrder::Desc);
        } else {
            sort_by.insert("slug".to_string(), SortOrder::Asc);
            sort_by.insert("version".to_string(), SortOrder::Desc);
        }

        let results = self
            .query_records(&[DocumentQuery {
                collection_name: WORKFLOW_DEFINITIONS_COLLECTION.to_string(),
                query,
                options: Some(QueryOptions {
                    populate: None,
                    sort_by: Some(sort_by),
                    limit: Some(LIST_WORKFLOW_DEFINITIONS_DEFAULT_LIMIT),
                    skip: None,
                    select: None,
                }),
                select: None,
            }])
            .await?;

        let docs = query_documents(&results);
        docs.into_iter()
            .map(WorkflowDefinition::from_mongo_value)
            .collect()
    }
}

fn first_query_document(results: &[serde_json::Value]) -> Option<serde_json::Value> {
    query_documents(results).into_iter().next()
}

fn query_documents(results: &[serde_json::Value]) -> Vec<serde_json::Value> {
    results
        .first()
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SmritiClientConfig;
    use crate::http::HttpSmritiClient;
    use crate::mock::MockSmritiDbClient;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn sample_definition_doc() -> serde_json::Value {
        serde_json::json!({
            "_id": "507f1f77bcf86cd799439011",
            "slug": "purchase-requisition-v1",
            "name": "Purchase Requisition",
            "workflowType": "purchase_requisition",
            "version": 1,
            "fields": [],
            "stages": [],
            "gates": [],
            "initialStage": "collect_vendor",
            "sourceFormat": "split_yaml",
            "createdAt": "2026-06-26T12:00:00Z",
            "updatedAt": "2026-06-26T12:00:00Z"
        })
    }

    #[tokio::test]
    async fn mock_get_latest_by_slug() {
        let client = MockSmritiDbClient::new()
            .with_query_response(vec![serde_json::json!([sample_definition_doc()])]);

        let def = client
            .get_workflow_definition("purchase-requisition-v1", None)
            .await
            .unwrap();
        assert_eq!(def.slug, "purchase-requisition-v1");
        assert_eq!(def.version, 1);
    }

    #[tokio::test]
    async fn mock_get_specific_version_not_found() {
        let client = MockSmritiDbClient::new().with_query_response(vec![serde_json::json!([])]);
        let err = client
            .get_workflow_definition("missing", Some(1))
            .await
            .unwrap_err();
        assert!(matches!(err, SmritiError::NotFound { .. }));
    }

    #[tokio::test]
    async fn http_get_workflow_definition_maps_query() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/db/query-records"))
            .and(body_json(serde_json::json!({
                "documentQueries": [{
                    "collectionName": "workflowDefinitions",
                    "query": { "slug": "purchase-requisition-v1", "version": 2 },
                    "options": { "limit": 1 }
                }]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [[sample_definition_doc()]]
            })))
            .mount(&server)
            .await;

        let client = HttpSmritiClient::new(SmritiClientConfig {
            base_url: server.uri(),
            timeout: std::time::Duration::from_secs(5),
        })
        .unwrap();

        let def = client
            .get_workflow_definition("purchase-requisition-v1", Some(2))
            .await
            .unwrap();
        assert_eq!(def.workflow_type, "purchase_requisition");
    }

    #[tokio::test]
    async fn http_list_workflow_definitions_by_type() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/db/query-records"))
            .and(body_json(serde_json::json!({
                "documentQueries": [{
                    "collectionName": "workflowDefinitions",
                    "query": { "workflowType": "github_pr_create" },
                    "options": {
                        "sortBy": { "version": "DESC" },
                        "limit": 100
                    }
                }]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [[{
                    "_id": "abc",
                    "slug": "github-pr-create-v1",
                    "name": "GitHub PR Create",
                    "workflowType": "github_pr_create",
                    "version": 1,
                    "fields": [],
                    "stages": [],
                    "gates": [],
                    "initialStage": "collect_intent",
                    "sourceFormat": "bundle_json",
                    "createdAt": "2026-06-26T12:00:00Z",
                    "updatedAt": "2026-06-26T12:00:00Z"
                }]]
            })))
            .mount(&server)
            .await;

        let client = HttpSmritiClient::new(SmritiClientConfig {
            base_url: server.uri(),
            timeout: std::time::Duration::from_secs(5),
        })
        .unwrap();

        let defs = client
            .list_workflow_definitions(Some("github_pr_create"))
            .await
            .unwrap();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].slug, "github-pr-create-v1");
    }
}
