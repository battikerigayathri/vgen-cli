use jsonwebtoken::{encode, EncodingKey, Header};
use serde::Serialize;
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

const ENV_SECRET: &str = "RESMATE_SECRET";

/// EAT payload: expiry timestamp (unix).
#[derive(Debug, Serialize)]
struct EatClaims {
    /// Expiry time as unix timestamp (seconds).
    eat: u64,
}

/// Load JWT secret from environment (after dotenvy has run).
pub fn get_secret() -> Result<String, String> {
    env::var(ENV_SECRET).map_err(|_| {
        format!(
            "{} is not set. Set it in .env at project root or in the environment.",
            ENV_SECRET
        )
    })
}

/// Build a JWT with payload { "eat": now + validity_secs }, signed with HS256 using raw secret.
pub fn build_jwt(validity_secs: u64) -> Result<String, String> {
    let secret = get_secret()?;
    if secret.is_empty() {
        return Err(format!("{} is empty.", ENV_SECRET));
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    let eat = now + validity_secs;

    let claims = EatClaims { eat };
    let key = EncodingKey::from_secret(secret.as_bytes());
    encode(&Header::default(), &claims, &key).map_err(|e| e.to_string())
}
