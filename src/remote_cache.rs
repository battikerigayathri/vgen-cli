use crate::api::get_record;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteLookupStatus {
    Found,
    NotFound,
    Error,
}

#[derive(Debug, Clone)]
pub struct RemoteLookupResult {
    pub status: RemoteLookupStatus,
    pub message: Option<String>,
    pub data: Option<Value>,
}

/// In-memory cache for remote record lookups within a single CLI invocation.
#[derive(Debug, Default)]
pub struct RemoteCache {
    entries: HashMap<(String, String), RemoteLookupResult>,
}

impl RemoteCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn get_record(&mut self, collection: &str, record_id: &str) -> RemoteLookupResult {
        let key = (collection.to_string(), record_id.to_string());
        if let Some(cached) = self.entries.get(&key) {
            return cached.clone();
        }

        let lookup_fut = get_record(collection, record_id);
        let result = match tokio::time::timeout(std::time::Duration::from_secs(8), lookup_fut).await
        {
            Ok(Ok(body)) => {
                let data = body.get("data").cloned();
                if data.as_ref().is_some_and(|d| !d.is_null()) {
                    RemoteLookupResult {
                        status: RemoteLookupStatus::Found,
                        message: None,
                        data,
                    }
                } else {
                    RemoteLookupResult {
                        status: RemoteLookupStatus::NotFound,
                        message: Some("get-record returned no data".to_string()),
                        data: None,
                    }
                }
            }
            Ok(Err(err)) => {
                let msg = err.to_string();
                let status =
                    if msg.contains("404") || msg.to_ascii_lowercase().contains("not found") {
                        RemoteLookupStatus::NotFound
                    } else {
                        RemoteLookupStatus::Error
                    };
                RemoteLookupResult {
                    status,
                    message: Some(msg),
                    data: None,
                }
            }
            Err(_) => RemoteLookupResult {
                status: RemoteLookupStatus::Error,
                message: Some("remote lookup timed out".to_string()),
                data: None,
            },
        };

        self.entries.insert(key, result.clone());
        result
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}
