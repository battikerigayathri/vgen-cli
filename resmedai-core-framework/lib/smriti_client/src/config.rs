use std::time::Duration;

/// Configuration for [`crate::HttpSmritiClient`].
#[derive(Debug, Clone)]
pub struct SmritiClientConfig {
    pub base_url: String,
    pub timeout: Duration,
}

impl SmritiClientConfig {
    pub fn from_env() -> Self {
        let base_url =
            std::env::var("SMRITI_URL").unwrap_or_else(|_| "http://localhost:6222".to_string());
        let timeout_ms = std::env::var("SMRITI_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10_000);
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            timeout: Duration::from_millis(timeout_ms),
        }
    }
}
