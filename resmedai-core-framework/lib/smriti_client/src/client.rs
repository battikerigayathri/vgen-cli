use async_trait::async_trait;

use crate::db::{CreateRecordParams, DocumentQuery, GetRecordParams, UpdateRecordParams};
use crate::error::SmritiError;

/// Generic Mongo CRUD boundary for Smriti `/db/*` routes.
///
/// Named `SmritiDbClient` in Phase 9.0 to avoid collision with the full workflow
/// `SmritiClient` trait planned for 9.1+.
#[async_trait]
pub trait SmritiDbClient: Send + Sync {
    async fn query_records(
        &self,
        queries: &[DocumentQuery],
    ) -> Result<Vec<serde_json::Value>, SmritiError>;

    async fn create_record(
        &self,
        params: &CreateRecordParams,
    ) -> Result<serde_json::Value, SmritiError>;

    async fn get_record(&self, params: &GetRecordParams) -> Result<serde_json::Value, SmritiError>;

    async fn update_record(
        &self,
        params: &UpdateRecordParams,
    ) -> Result<serde_json::Value, SmritiError>;
}
