use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::client::SmritiDbClient;
use crate::config::SmritiClientConfig;
use crate::db::{CreateRecordParams, DocumentQuery, GetRecordParams, UpdateRecordParams};
use crate::error::{Result, SmritiError};

#[derive(Serialize)]
struct QueryRecordsRequest<'a> {
    #[serde(rename = "documentQueries")]
    document_queries: &'a [DocumentQuery],
}

#[derive(Serialize)]
struct CreateRecordRequest<'a> {
    #[serde(rename = "collectionName")]
    collection_name: &'a str,
    payload: &'a serde_json::Value,
}

#[derive(Serialize)]
struct GetRecordRequest<'a> {
    #[serde(rename = "collectionName")]
    collection_name: &'a str,
    #[serde(rename = "recordId")]
    record_id: &'a str,
}

#[derive(Serialize)]
struct UpdateRecordRequest<'a> {
    #[serde(rename = "collectionName")]
    collection_name: &'a str,
    #[serde(rename = "recordId")]
    record_id: &'a str,
    document: &'a serde_json::Value,
}

#[derive(Deserialize)]
struct PayloadResponse {
    data: serde_json::Value,
}

pub struct HttpSmritiClient {
    client: Client,
    base_url: String,
    timeout: std::time::Duration,
}

impl HttpSmritiClient {
    pub fn new(config: SmritiClientConfig) -> Result<Self> {
        let client = Client::builder().timeout(config.timeout).build()?;
        Ok(Self {
            client,
            base_url: config.base_url,
            timeout: config.timeout,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    async fn send_json(
        &self,
        method: reqwest::Method,
        path: &str,
        body: &impl Serialize,
    ) -> Result<serde_json::Value> {
        let url = self.url(path);
        debug!("[smriti_db] {method} {url}");

        let response = self
            .client
            .request(method, &url)
            .json(body)
            .send()
            .await
            .map_err(|e| map_reqwest_error(e, self.timeout))?;

        let status = response.status();
        let body_text = response.text().await.map_err(SmritiError::Connection)?;

        if !status.is_success() {
            return Err(SmritiError::Http {
                status: status.as_u16(),
                body: body_text,
            });
        }

        let envelope: PayloadResponse = serde_json::from_str(&body_text).map_err(|e| {
            SmritiError::InvalidResponse(format!("failed to parse PayloadResponse: {e}"))
        })?;
        Ok(envelope.data)
    }

    /// POST to a Smriti `/workflow/*` route; returns the `data` envelope field.
    pub async fn post_workflow_json(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> Result<serde_json::Value> {
        self.send_json(reqwest::Method::POST, path, body).await
    }
}

fn map_reqwest_error(err: reqwest::Error, timeout: std::time::Duration) -> SmritiError {
    if err.is_timeout() {
        SmritiError::Timeout(timeout)
    } else {
        SmritiError::Connection(err)
    }
}

#[async_trait]
impl SmritiDbClient for HttpSmritiClient {
    async fn query_records(&self, queries: &[DocumentQuery]) -> Result<Vec<serde_json::Value>> {
        let data = self
            .send_json(
                reqwest::Method::POST,
                "/db/query-records",
                &QueryRecordsRequest {
                    document_queries: queries,
                },
            )
            .await?;

        match data {
            serde_json::Value::Array(items) => Ok(items),
            other => Err(SmritiError::InvalidResponse(format!(
                "query_records expected array, got {other}"
            ))),
        }
    }

    async fn create_record(&self, params: &CreateRecordParams) -> Result<serde_json::Value> {
        self.send_json(
            reqwest::Method::POST,
            "/db/create-record",
            &CreateRecordRequest {
                collection_name: &params.collection,
                payload: &params.payload,
            },
        )
        .await
    }

    async fn get_record(&self, params: &GetRecordParams) -> Result<serde_json::Value> {
        match self
            .send_json(
                reqwest::Method::POST,
                "/db/get-record",
                &GetRecordRequest {
                    collection_name: &params.collection,
                    record_id: &params.record_id,
                },
            )
            .await
        {
            Ok(data) => Ok(data),
            Err(SmritiError::Http { status: 404, .. }) => Err(SmritiError::NotFound {
                collection: params.collection.clone(),
                id: params.record_id.clone(),
            }),
            Err(e) => Err(e),
        }
    }

    async fn update_record(&self, params: &UpdateRecordParams) -> Result<serde_json::Value> {
        self.send_json(
            reqwest::Method::PUT,
            "/db/update-record",
            &UpdateRecordRequest {
                collection_name: &params.collection,
                record_id: &params.record_id,
                document: &params.document,
            },
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::QueryOptions;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_client(base_url: &str) -> HttpSmritiClient {
        HttpSmritiClient::new(SmritiClientConfig {
            base_url: base_url.to_string(),
            timeout: std::time::Duration::from_secs(5),
        })
        .expect("client")
    }

    #[tokio::test]
    async fn query_records_maps_request_and_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/db/query-records"))
            .and(body_json(serde_json::json!({
                "documentQueries": [{
                    "collectionName": "agents",
                    "query": { "slug": "test" },
                    "options": { "limit": 1 }
                }]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [[{ "_id": "abc", "slug": "test" }]]
            })))
            .mount(&server)
            .await;

        let client = test_client(&server.uri());
        let results = client
            .query_records(&[DocumentQuery {
                collection_name: "agents".to_string(),
                query: serde_json::json!({ "slug": "test" }),
                options: Some(QueryOptions {
                    populate: None,
                    sort_by: None,
                    limit: Some(1),
                    skip: None,
                    select: None,
                }),
                select: None,
            }])
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].is_array());
    }

    #[tokio::test]
    async fn create_record_maps_request_and_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/db/create-record"))
            .and(body_json(serde_json::json!({
                "collectionName": "sessions",
                "payload": { "name": "demo" }
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": { "_id": "507f1f77bcf86cd799439011", "name": "demo" }
            })))
            .mount(&server)
            .await;

        let client = test_client(&server.uri());
        let record = client
            .create_record(&CreateRecordParams {
                collection: "sessions".to_string(),
                payload: serde_json::json!({ "name": "demo" }),
            })
            .await
            .unwrap();

        assert_eq!(record["name"], "demo");
    }

    #[tokio::test]
    async fn get_record_maps_request_and_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/db/get-record"))
            .and(body_json(serde_json::json!({
                "collectionName": "sessions",
                "recordId": "507f1f77bcf86cd799439011"
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": { "_id": "507f1f77bcf86cd799439011", "name": "demo" }
            })))
            .mount(&server)
            .await;

        let client = test_client(&server.uri());
        let record = client
            .get_record(&GetRecordParams {
                collection: "sessions".to_string(),
                record_id: "507f1f77bcf86cd799439011".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(record["_id"], "507f1f77bcf86cd799439011");
    }

    #[tokio::test]
    async fn get_record_maps_404_to_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/db/get-record"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Document not found"))
            .mount(&server)
            .await;

        let client = test_client(&server.uri());
        let err = client
            .get_record(&GetRecordParams {
                collection: "sessions".to_string(),
                record_id: "missing".to_string(),
            })
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            SmritiError::NotFound { collection, id }
                if collection == "sessions" && id == "missing"
        ));
    }

    #[tokio::test]
    async fn update_record_maps_request_and_response() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/db/update-record"))
            .and(body_json(serde_json::json!({
                "collectionName": "sessions",
                "recordId": "507f1f77bcf86cd799439011",
                "document": { "name": "updated" }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": { "success": true, "message": "updated" }
            })))
            .mount(&server)
            .await;

        let client = test_client(&server.uri());
        let result = client
            .update_record(&UpdateRecordParams {
                collection: "sessions".to_string(),
                record_id: "507f1f77bcf86cd799439011".to_string(),
                document: serde_json::json!({ "name": "updated" }),
            })
            .await
            .unwrap();

        assert_eq!(result["success"], true);
    }

    #[tokio::test]
    async fn http_error_surfaces_status_and_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/db/query-records"))
            .respond_with(ResponseTemplate::new(500).set_body_string("internal error"))
            .mount(&server)
            .await;

        let client = test_client(&server.uri());
        let err = client
            .query_records(&[DocumentQuery {
                collection_name: "agents".to_string(),
                query: serde_json::json!({}),
                options: None,
                select: None,
            }])
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            SmritiError::Http {
                status: 500,
                body
            } if body == "internal error"
        ));
    }
}
