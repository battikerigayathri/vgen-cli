use std::time::Duration;

pub type Result<T> = std::result::Result<T, SmritiError>;

#[derive(Debug, thiserror::Error)]
pub enum SmritiError {
    #[error("HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("request timeout after {0:?}")]
    Timeout(Duration),
    #[error("connection failed: {0}")]
    Connection(#[from] reqwest::Error),
    #[error("invalid response: {0}")]
    InvalidResponse(String),
    #[error("record not found: collection={collection} id={id}")]
    NotFound { collection: String, id: String },
    #[error("duplicate workflow definition: slug={slug} version={version}")]
    DuplicateDefinition { slug: String, version: u32 },
    #[error("JSON Schema validation failed: {0}")]
    SchemaValidation(String),
    #[error("semantic validation failed at {path}: {message}")]
    SemanticValidation { path: String, message: String },
    #[error("invalid author layout at {path}: {reason}")]
    InvalidAuthorLayout { path: String, reason: String },
    #[error("YAML parse error: {0}")]
    YamlParse(String),
    #[error("JSON parse error: {0}")]
    JsonParse(String),
    #[error("IO error at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error(
        "workflow version conflict: workflow_id={workflow_id} expected={expected} actual={actual}"
    )]
    WorkflowConflict {
        workflow_id: String,
        expected: u64,
        actual: u64,
    },
    #[error("unauthorized workflow access for user_id={user_id}")]
    Unauthorized { user_id: String },
    #[error("WorkflowAuthz.user_id must not be empty")]
    EmptyUserId,
    #[error("invalid workflow request: {0}")]
    InvalidRequest(String),
    #[error("workflow instance not found: workflow_id={workflow_id}")]
    WorkflowNotFound { workflow_id: String },
}

impl SmritiError {
    pub fn io(path: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
