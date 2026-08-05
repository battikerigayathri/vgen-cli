use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Mirrors Smriti `DocumentQuery` (`smriti/src/routes.rs`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentQuery {
    #[serde(rename = "collectionName")]
    pub collection_name: String,
    pub query: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<QueryOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub select: Option<Vec<String>>,
}

/// Mirrors Smriti `QueryOptions` (`smriti/src/db/db_impl.rs`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub populate: Option<Vec<PopulateOptions>>,
    #[serde(rename = "sortBy", skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<HashMap<String, SortOrder>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub select: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SortOrder {
    Desc,
    Asc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopulateOptions {
    pub path: String,
    pub from: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub select: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Box<QueryOptions>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRecordParams {
    pub collection: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetRecordParams {
    pub collection: String,
    pub record_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRecordParams {
    pub collection: String,
    pub record_id: String,
    pub document: serde_json::Value,
}
