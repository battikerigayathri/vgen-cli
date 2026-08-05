use crate::client::SmritiDbClient;
use crate::db::{CreateRecordParams, DocumentQuery, QueryOptions};
use crate::error::{Result, SmritiError};
use crate::workflow::normalize::{WORKFLOW_DEFINITIONS_COLLECTION, prepare_for_push};
use crate::workflow::types::WorkflowDefinition;

/// Append-only push: reject duplicate `(slug, version)` before Mongo insert.
pub async fn push_workflow_definition(
    client: &dyn SmritiDbClient,
    def: WorkflowDefinition,
) -> Result<WorkflowDefinition> {
    let slug = def.slug.clone();
    let version = def.version;

    let existing = client
        .query_records(&[DocumentQuery {
            collection_name: WORKFLOW_DEFINITIONS_COLLECTION.to_string(),
            query: serde_json::json!({ "slug": slug, "version": version }),
            options: Some(QueryOptions {
                populate: None,
                sort_by: None,
                limit: Some(1),
                skip: None,
                select: None,
            }),
            select: None,
        }])
        .await?;

    if !existing.is_empty() && !existing[0].as_array().is_none_or(|a| a.is_empty()) {
        return Err(SmritiError::DuplicateDefinition { slug, version });
    }

    let (mut prepared, payload) = prepare_for_push(def)?;
    let inserted = client
        .create_record(&CreateRecordParams {
            collection: WORKFLOW_DEFINITIONS_COLLECTION.to_string(),
            payload,
        })
        .await?;

    prepared.id = inserted
        .get("_id")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    Ok(prepared)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockSmritiDbClient;
    use crate::workflow::loader::WorkflowDefinitionLoader;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    fn split_definition() -> WorkflowDefinition {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/purchase-requisition-split");
        WorkflowDefinitionLoader::load_from_dir(&dir).unwrap()
    }

    #[derive(Default)]
    struct QueryingMock {
        inner: MockSmritiDbClient,
        definitions: Arc<Mutex<Vec<serde_json::Value>>>,
    }

    #[async_trait::async_trait]
    impl SmritiDbClient for QueryingMock {
        async fn query_records(&self, queries: &[DocumentQuery]) -> Result<Vec<serde_json::Value>> {
            let query = &queries[0];
            if query.collection_name != WORKFLOW_DEFINITIONS_COLLECTION {
                return self.inner.query_records(queries).await;
            }
            let slug = query.query.get("slug").and_then(|v| v.as_str());
            let version = query.query.get("version").and_then(|v| v.as_u64());
            let docs = self.definitions.lock().expect("lock");
            let matched: Vec<serde_json::Value> = docs
                .iter()
                .filter(|doc| {
                    slug.is_none_or(|s| doc.get("slug").and_then(|v| v.as_str()) == Some(s))
                        && version.is_none_or(|v| {
                            doc.get("version").and_then(|val| val.as_u64()) == Some(v)
                        })
                })
                .cloned()
                .collect();
            Ok(vec![serde_json::Value::Array(matched)])
        }

        async fn create_record(&self, params: &CreateRecordParams) -> Result<serde_json::Value> {
            let created = self.inner.create_record(params).await?;
            self.definitions.lock().expect("lock").push(created.clone());
            Ok(created)
        }

        async fn get_record(
            &self,
            params: &crate::db::GetRecordParams,
        ) -> Result<serde_json::Value> {
            self.inner.get_record(params).await
        }

        async fn update_record(
            &self,
            params: &crate::db::UpdateRecordParams,
        ) -> Result<serde_json::Value> {
            self.inner.update_record(params).await
        }
    }

    #[tokio::test]
    async fn push_happy_path() {
        let client = QueryingMock::default();
        let def = split_definition();
        let pushed = push_workflow_definition(&client, def).await.unwrap();
        assert_eq!(pushed.slug, "purchase-requisition-v1");
        assert!(pushed.id.is_some());
        assert!(pushed.created_at.is_some());
    }

    #[tokio::test]
    async fn push_rejects_duplicate_slug_version() {
        let client = QueryingMock::default();
        let def = split_definition();
        push_workflow_definition(&client, def.clone())
            .await
            .unwrap();
        let err = push_workflow_definition(&client, def).await.unwrap_err();
        assert!(matches!(
            err,
            SmritiError::DuplicateDefinition {
                slug,
                version: 1
            } if slug == "purchase-requisition-v1"
        ));
    }
}
