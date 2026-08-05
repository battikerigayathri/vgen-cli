use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::workflow::types::WorkflowStageKind;

/// Authorization context required on every workflow SDK call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowAuthz {
    pub user_id: String,
    pub session_id: Option<String>,
    pub assistant_id: Option<String>,
}

/// Runtime workflow instance (Mongo `workflowInstances`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowInstance {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none", default)]
    pub mongo_id: Option<String>,
    pub workflow_id: String,
    pub definition_slug: String,
    pub definition_version: u32,
    pub workflow_type: String,
    pub user_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant_id: Option<String>,
    pub stage: String,
    pub status: WorkflowStatus,
    #[serde(default)]
    pub inputs: Value,
    #[serde(default)]
    pub artifacts: Value,
    #[serde(default)]
    pub validation_errors: Vec<FieldError>,
    #[serde(default)]
    pub missing_required: Vec<String>,
    pub version: u64,
    #[serde(default)]
    pub external_refs: Value,
    #[serde(default)]
    pub audit: Vec<WorkflowAuditEntry>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStatus {
    Draft,
    InProgress,
    PendingApproval,
    Submitted,
    Cancelled,
    Failed,
}

impl WorkflowStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::InProgress => "in_progress",
            Self::PendingApproval => "pending_approval",
            Self::Submitted => "submitted",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_refs: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAuditEntry {
    pub timestamp: DateTime<Utc>,
    pub source: String,
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

#[derive(Debug, Clone, Default)]
pub struct WorkflowListFilter {
    pub workflow_type: Option<String>,
    pub status: Option<WorkflowStatus>,
    pub session_id: Option<String>,
    pub limit: Option<u64>,
    pub skip: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveWorkflowPolicy {
    SessionBound,
    AlwaysNew,
    ExplicitWorkflowId(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSnapshot {
    pub workflow_id: String,
    pub workflow_type: String,
    pub stage: String,
    pub stage_label: String,
    pub stage_kind: WorkflowStageKind,
    pub status: WorkflowStatus,
    pub inputs_summary: Value,
    pub artifacts_summary: Value,
    pub missing_required: Vec<String>,
    pub validation_errors: Vec<FieldError>,
    pub completeness_pct: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_hitl_slug: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_outcome: Option<String>,
    pub allowed_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletenessResult {
    pub complete: bool,
    pub completeness_pct: u8,
    pub missing_required: Vec<String>,
    pub validation_errors: Vec<FieldError>,
}

impl WorkflowInstance {
    /// Parse from Smriti `/workflow/*` JSON. Prefer [`crate::workflow::instance_normalize::instance_from_mongo`].
    pub fn from_mongo_value(value: Value) -> crate::error::Result<Self> {
        let value = crate::workflow::normalize::flatten_mongo_extended_json(value);
        serde_json::from_value(value).map_err(|e| {
            crate::error::SmritiError::InvalidResponse(format!(
                "failed to parse WorkflowInstance from Mongo: {e}"
            ))
        })
    }
}
