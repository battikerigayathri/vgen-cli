//! Typed HTTP client for Smriti generic Mongo CRUD routes (`/db/*`).
//!
//! Phase 9.1 adds workflow definition storage (`workflowDefinitions` collection).
//! Phase 9.2 adds workflow instance CRUD via `/workflow/*` routes.

mod client;
mod config;
mod db;
mod error;
mod http;
mod mock;
mod workflow;

pub use client::SmritiDbClient;
pub use config::SmritiClientConfig;
pub use db::{
    CreateRecordParams, DocumentQuery, GetRecordParams, PopulateOptions, QueryOptions, SortOrder,
    UpdateRecordParams,
};
pub use error::{Result, SmritiError};
pub use http::HttpSmritiClient;
pub use mock::MockSmritiDbClient;
pub use workflow::{
    ActiveWorkflowPolicy, AuthorFormat, CompletenessResult, Condition, CreateWorkflowDefinitionRequest,
    FieldBag, FieldError, GetWorkflowDefinitionRequest, LIST_WORKFLOW_DEFINITIONS_DEFAULT_LIMIT,
    MockWorkflowStore, SmritiClient, SourceFormat, StageDoneWhen, TransitionRule,
    WORKFLOW_DEFINITIONS_COLLECTION, WORKFLOW_INSTANCES_COLLECTION, WorkflowAuditEntry,
    WorkflowAuthz, WorkflowDefinition, WorkflowDefinitionCache, WorkflowDefinitionClient,
    WorkflowDefinitionLoader, WorkflowFieldDef, WorkflowFieldType, WorkflowGateDef,
    WorkflowInstance, WorkflowInstanceClient, WorkflowListFilter, WorkflowPatch, WorkflowSnapshot,
    WorkflowStageDef, WorkflowStageKind, WorkflowStatus, completeness_check,
    compute_missing_required, deep_merge_object, evaluate_done_when,
    evaluate_done_when_with_validator, guard_authz, maybe_auto_advance,
    maybe_auto_advance_with_validator, normalize_to_mongo, parse_create_definition_request,
    prepare_for_push, project_workflow_snapshot, push_workflow_definition, resolve_workflow_path,
};
