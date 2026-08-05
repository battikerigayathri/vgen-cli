use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::client::SmritiDbClient;
use crate::db::{CreateRecordParams, DocumentQuery, GetRecordParams, UpdateRecordParams};
use crate::error::{Result, SmritiError};

type RecordKey = (String, String);

/// In-memory [`SmritiDbClient`] for unit tests.
pub struct MockSmritiDbClient {
    records: Mutex<HashMap<RecordKey, serde_json::Value>>,
    query_responses: Mutex<Vec<Vec<serde_json::Value>>>,
}

impl Default for MockSmritiDbClient {
    fn default() -> Self {
        Self::new()
    }
}

impl MockSmritiDbClient {
    pub fn new() -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
            query_responses: Mutex::new(Vec::new()),
        }
    }

    pub fn with_record(
        self,
        collection: impl Into<String>,
        record_id: impl Into<String>,
        document: serde_json::Value,
    ) -> Self {
        self.records
            .lock()
            .expect("mock records lock")
            .insert((collection.into(), record_id.into()), document);
        self
    }

    pub fn with_query_response(self, response: Vec<serde_json::Value>) -> Self {
        self.query_responses
            .lock()
            .expect("mock query lock")
            .push(response);
        self
    }
}

#[async_trait]
impl SmritiDbClient for MockSmritiDbClient {
    async fn query_records(&self, _queries: &[DocumentQuery]) -> Result<Vec<serde_json::Value>> {
        let mut responses = self.query_responses.lock().expect("mock query lock");
        if responses.is_empty() {
            return Ok(Vec::new());
        }
        Ok(responses.remove(0))
    }

    async fn create_record(&self, params: &CreateRecordParams) -> Result<serde_json::Value> {
        let record_id = params
            .payload
            .get("_id")
            .and_then(|v| v.as_str())
            .unwrap_or("mock-id")
            .to_string();
        let mut document = params.payload.clone();
        if document.get("_id").is_none() {
            document["_id"] = serde_json::Value::String(record_id.clone());
        }
        self.records.lock().expect("mock records lock").insert(
            (params.collection.clone(), record_id.clone()),
            document.clone(),
        );
        Ok(document)
    }

    async fn get_record(&self, params: &GetRecordParams) -> Result<serde_json::Value> {
        let records = self.records.lock().expect("mock records lock");
        records
            .get(&(params.collection.clone(), params.record_id.clone()))
            .cloned()
            .ok_or_else(|| SmritiError::NotFound {
                collection: params.collection.clone(),
                id: params.record_id.clone(),
            })
    }

    async fn update_record(&self, params: &UpdateRecordParams) -> Result<serde_json::Value> {
        let key = (params.collection.clone(), params.record_id.clone());
        let mut records = self.records.lock().expect("mock records lock");
        if !records.contains_key(&key) {
            return Err(SmritiError::NotFound {
                collection: params.collection.clone(),
                id: params.record_id.clone(),
            });
        }
        records.insert(key, params.document.clone());
        Ok(serde_json::json!({
            "success": true,
            "message": "updated"
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_create_and_get_round_trip() {
        let client = MockSmritiDbClient::new();
        let created = client
            .create_record(&CreateRecordParams {
                collection: "sessions".to_string(),
                payload: serde_json::json!({ "_id": "abc", "name": "demo" }),
            })
            .await
            .unwrap();
        assert_eq!(created["name"], "demo");

        let fetched = client
            .get_record(&GetRecordParams {
                collection: "sessions".to_string(),
                record_id: "abc".to_string(),
            })
            .await
            .unwrap();
        assert_eq!(fetched["name"], "demo");
    }

    #[tokio::test]
    async fn mock_query_returns_scripted_response() {
        let client = MockSmritiDbClient::new().with_query_response(vec![serde_json::json!([
            { "slug": "agent-a" }
        ])]);

        let results = client
            .query_records(&[DocumentQuery {
                collection_name: "agents".to_string(),
                query: serde_json::json!({}),
                options: None,
                select: None,
            }])
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0][0]["slug"], "agent-a");
    }
}
