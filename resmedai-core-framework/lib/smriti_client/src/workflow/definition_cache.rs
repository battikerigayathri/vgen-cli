use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::error::Result;
use crate::workflow::client::WorkflowDefinitionClient;
use crate::workflow::types::WorkflowDefinition;

type CachedDefinition = (Arc<WorkflowDefinition>, Instant);
type DefinitionCacheMap = HashMap<(String, u32), CachedDefinition>;

/// In-memory TTL cache for pinned `(slug, version)` definitions.
pub struct WorkflowDefinitionCache {
    inner: Mutex<DefinitionCacheMap>,
    ttl: Duration,
}

impl WorkflowDefinitionCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            ttl,
        }
    }

    pub fn from_env() -> Self {
        let ttl_secs = std::env::var("WORKFLOW_DEFINITION_CACHE_TTL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(300);
        Self::new(Duration::from_secs(ttl_secs))
    }

    pub async fn get_or_load<C: WorkflowDefinitionClient + ?Sized>(
        &self,
        client: &C,
        slug: &str,
        version: u32,
    ) -> Result<Arc<WorkflowDefinition>> {
        let key = (slug.to_string(), version);
        if let Some(cached) = self.get_if_fresh(&key) {
            return Ok(cached);
        }

        let def = client.get_workflow_definition(slug, Some(version)).await?;
        let arc = Arc::new(def);
        self.inner
            .lock()
            .expect("definition cache lock")
            .insert(key, (Arc::clone(&arc), Instant::now()));
        Ok(arc)
    }

    fn get_if_fresh(&self, key: &(String, u32)) -> Option<Arc<WorkflowDefinition>> {
        let guard = self.inner.lock().expect("definition cache lock");
        guard.get(key).and_then(|(def, inserted)| {
            if inserted.elapsed() <= self.ttl {
                Some(Arc::clone(def))
            } else {
                None
            }
        })
    }

    #[allow(dead_code)]
    pub fn invalidate(&self, slug: &str, version: u32) {
        self.inner
            .lock()
            .expect("definition cache lock")
            .remove(&(slug.to_string(), version));
    }
}

impl Default for WorkflowDefinitionCache {
    fn default() -> Self {
        Self::from_env()
    }
}
