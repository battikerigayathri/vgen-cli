mod agent_job;
mod feed_lint;
mod orchestration_contract;
mod session_context;
mod skill_catalog;
mod workflow_prompt;

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

pub use agent_job::{
    AgentJob, AgentJobLlmRow, AgentJobSnapshot, AgentJobStatus, AgentJobSummary, AgentMicroAttempt,
    AgentPlanOutput, AgentPlannerAttempt, MacroEscalationEntry, MacroValidationEntry,
    OrchestrationLedgerEntry, PlanKind, argument_resolution_exhausted, assemble_delegation_brief,
    build_agent_handoff_context, build_macro_replan_context, build_macro_validation_context,
    build_macro_validation_context_with_completeness, format_agent_summaries_for_compose,
    format_macro_compose_context, format_macro_failure_for_compose,
    format_tool_attempts_for_prompt, format_tool_result_for_prompt, macro_compose_is_failure,
    tools_triggered_from_attempts,
};
pub use feed_lint::{
    FeedValidationIssue, FeedValidationSeverity, lint_assistant_tool_feeds, lint_tool_feeds,
};
pub use orchestration_contract::{OrchestrationContract, parse_orchestration_contract};
pub use session_context::{
    SessionContextBudget, compose_history_turns, format_history_for_orchestrator,
    history_entries_from_turns,
};
pub use skill_catalog::{
    SkillCatalog, SkillCatalogAgent, SkillRecord, SkillRef, SkillResolveError,
    apply_openai_strict_object_required, effective_parameters, normalize_argument_value,
    normalize_skill_parameters,
};
pub use workflow_prompt::{
    WorkflowBlockStyle, format_workflow_block, format_workflow_block_full,
    workflow_validator_summary,
};

pub const WORKFLOW_STATE_VERSION: u32 = 5;

/// Field-level workflow patch validation error (mirrors Smriti SDK shape).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowFieldError {
    pub field: String,
    pub message: String,
}
pub const WORKFLOW_REDIS_TTL_SECS: u64 = 3600;

#[derive(Debug, Error)]
pub enum WorkflowError {
    #[error("legacy payload must have at least {0} elements")]
    LegacyPayloadTooShort(usize),
    #[error("failed to serialize workflow state: {0}")]
    Serialize(String),
    #[error("workflow state not found for session {0}")]
    NotFound(String),
    #[error("redis error: {0}")]
    Redis(String),
    #[error("invalid phase transition from {from:?} to {to:?}")]
    InvalidTransition {
        from: WorkflowPhase,
        to: WorkflowPhase,
    },
}

/// Supervisor chat loop is always active (Phase 6.3).
pub fn supervisor_chat_path_enabled() -> bool {
    true
}

/// Label for structured path logging.
pub fn workflow_path_label() -> &'static str {
    "supervisor"
}

/// Returns true when heuristic fast path is enabled.
pub fn planner_heuristic_fast_path_enabled() -> bool {
    std::env::var("PLANNER_HEURISTIC_FAST_PATH")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Returns true when workflow audit snapshots should be persisted.
pub fn workflow_audit_enabled() -> bool {
    std::env::var("WORKFLOW_AUDIT_ENABLED")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// When true, readers may fall back to legacy `kriya:session:{id}` keys (migration soak).
pub fn workflow_kriya_session_read_through_enabled() -> bool {
    std::env::var("WORKFLOW_KRIYA_SESSION_READ_THROUGH")
        .map(|v| v != "0" && !v.eq_ignore_ascii_case("false"))
        .unwrap_or(true)
}

/// When true, orchestrator v4 produces agent jobs with assistant planner handoff.
pub fn orchestrator_v4_enabled() -> bool {
    std::env::var("WORKFLOW_ORCHESTRATOR_V4")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(true)
}

pub const DEFAULT_AGENT_PLANNER_MAX_TURNS: u32 = 5;

/// Max agent planner LLM turns per macro job before circuit breaker (Phase 7.2).
pub fn agent_planner_max_turns() -> u32 {
    std::env::var("AGENT_PLANNER_MAX_TURNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .or_else(|| {
            std::env::var("AGENT_MICRO_MAX_TURNS")
                .ok()
                .and_then(|v| v.parse().ok())
        })
        .unwrap_or(DEFAULT_AGENT_PLANNER_MAX_TURNS)
}

pub const DEFAULT_AGENT_PLANNER_OVERLAY_MAX_STEPS: u32 = 8;

/// Max tasks in one agent planner `execute_plan` micro-plan (Phase 7.6).
pub fn agent_planner_overlay_max_steps() -> u32 {
    std::env::var("AGENT_PLANNER_OVERLAY_MAX_STEPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_AGENT_PLANNER_OVERLAY_MAX_STEPS)
}

/// When true (default), soft arg-failure retries do not increment `agent_planner_turn_count` (Phase 7.7).
pub fn agent_planner_arg_failure_free_turns() -> bool {
    std::env::var("AGENT_PLANNER_ARG_FAILURE_FREE_TURNS")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(true)
}

/// Max characters for planner/supervisor tools appendix (D7).
pub fn catalog_appendix_max_chars() -> usize {
    std::env::var("CATALOG_APPENDIX_MAX_CHARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(12_000)
}

/// Feature flags resolved from environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkflowFlags {
    pub heuristic_fast_path: bool,
    pub audit: bool,
    pub supervisor_chat: bool,
}

impl WorkflowFlags {
    pub fn from_env() -> Self {
        Self {
            heuristic_fast_path: planner_heuristic_fast_path_enabled(),
            audit: workflow_audit_enabled(),
            supervisor_chat: supervisor_chat_path_enabled(),
        }
    }
}

/// Loop guard configuration and counters (D6).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LoopGuards {
    #[serde(default = "default_max_steps")]
    pub max_steps: u32,
    #[serde(default = "default_max_replans")]
    pub max_replans: u32,
    #[serde(default)]
    pub max_total_tokens: u32,
    #[serde(default)]
    pub replan_count: u32,
}

fn default_max_steps() -> u32 {
    8
}

fn default_max_replans() -> u32 {
    3
}

impl Default for LoopGuards {
    fn default() -> Self {
        Self {
            max_steps: default_max_steps(),
            max_replans: default_max_replans(),
            max_total_tokens: 0,
            replan_count: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardBreachReason {
    MaxSteps,
    MaxReplans,
    MaxTokens,
    Cycle,
}

impl GuardBreachReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MaxSteps => "max_steps",
            Self::MaxReplans => "max_replans",
            Self::MaxTokens => "max_tokens",
            Self::Cycle => "cycle",
        }
    }
}

fn tasks_match_for_cycle(a: &PlannedTask, b: &PlannedTask) -> bool {
    let a_tool = a.tool_id.as_deref().filter(|s| !s.is_empty());
    let b_tool = b.tool_id.as_deref().filter(|s| !s.is_empty());
    let a_skill = a.skill_id.as_deref().filter(|s| !s.is_empty());
    let b_skill = b.skill_id.as_deref().filter(|s| !s.is_empty());

    let id_match = match (a_tool, b_tool) {
        (Some(at), Some(bt)) => at == bt,
        _ => a_skill.is_some() && a_skill == b_skill,
    };
    id_match && a.input_arguments == b.input_arguments
}

/// Returns a guard breach reason when loop limits are exceeded.
pub fn guard_breach(workflow: &WorkflowState) -> Option<GuardBreachReason> {
    let guards = &workflow.guards;
    if workflow.results.len() as u32 >= guards.max_steps {
        return Some(GuardBreachReason::MaxSteps);
    }
    if guards.replan_count >= guards.max_replans {
        return Some(GuardBreachReason::MaxReplans);
    }
    if guards.max_total_tokens > 0 && workflow.usage.total_tokens >= guards.max_total_tokens {
        return Some(GuardBreachReason::MaxTokens);
    }
    if let Some(current) = workflow.current_task()
        && let Some(plan) = &workflow.plan
    {
        for (i, prior) in plan.tasks.iter().enumerate() {
            if i >= workflow.results.len() {
                break;
            }
            if tasks_match_for_cycle(&current, prior) {
                return Some(GuardBreachReason::Cycle);
            }
        }
    }
    None
}

pub fn workflow_flags() -> WorkflowFlags {
    WorkflowFlags::from_env()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WorkflowPhase {
    Planning,
    Executing,
    /// v4 macro plan persisted; agent planner not yet run (Phase 7.1 compose).
    AwaitingAgentDelegation,
    /// Agent job failed or circuit breaker; assistant replan hook (Phase 7.4).
    AwaitingMacroReplan,
    Composing,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PlannerOutcome {
    PlannedTasks,
    DirectAnswer,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SupervisorDecision {
    Continue,
    Replan,
    Compose,
    Fail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupervisorOutcome {
    pub decision: SupervisorDecision,
    pub reasoning: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining_tasks: Option<Vec<PlannedTask>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRef {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub user_message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_response: Option<String>,
    #[serde(default)]
    pub functions: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedTask {
    pub step: String,
    pub description: String,
    #[serde(default, alias = "assigned_agent")]
    pub assigned_agent: String,
    pub reason: String,
    #[serde(default, alias = "tool_id", skip_serializing_if = "Option::is_none")]
    pub tool_id: Option<String>,
    #[serde(alias = "skill_id", skip_serializing_if = "Option::is_none")]
    pub skill_id: Option<String>,
    #[serde(alias = "input_arguments", skip_serializing_if = "Option::is_none")]
    pub input_arguments: Option<Value>,
}

impl PlannedTask {
    /// Kriya wire format (snake_case field names).
    pub fn to_kriya_task_value(&self) -> Value {
        json!({
            "step": self.step,
            "description": self.description,
            "assigned_agent": self.assigned_agent,
            "reason": self.reason,
            "tool_id": self.tool_id,
            "skill_id": self.skill_id,
            "input_arguments": self.input_arguments,
        })
    }
}

/// Read a task JSON field accepting snake_case or camelCase keys.
pub fn task_json_field(task: &Value, snake: &str, camel: &str) -> Option<String> {
    task.get(snake)
        .or_else(|| task.get(camel))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanOutput {
    pub tasks: Vec<PlannedTask>,
    pub notes: String,
    pub analysis: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direct_answer: Option<String>,
    pub planner_outcome: PlannerOutcome,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub completion_tokens: u32,
    #[serde(default)]
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskResult {
    pub skill_id: String,
    pub result: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_slug: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mongo_skill_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionAttachment {
    #[serde(rename = "fileName")]
    pub file_name: String,
    #[serde(rename = "fileId")]
    pub file_id: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
}

/// Human-readable attachment block for agent planner / argument resolver prompts.
pub fn format_session_attachments_for_prompt(attachments: &[SessionAttachment]) -> String {
    if attachments.is_empty() {
        return "(none)".to_string();
    }
    attachments
        .iter()
        .map(|a| {
            format!(
                "- {} | file_id={} | mime={}",
                a.file_name, a.file_id, a.mime_type
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Collect session attachment IDs in upload order.
pub fn session_attachment_file_ids(attachments: &[SessionAttachment]) -> Vec<String> {
    attachments.iter().map(|a| a.file_id.clone()).collect()
}

fn argument_value_present(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(s) => !s.trim().is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
        _ => true,
    }
}

/// Server-side prefill for tool parameters from session context (attachments, user message).
pub fn prefill_tool_arguments(
    record: &SkillRecord,
    attachments: &[SessionAttachment],
    user_message: &str,
    input: &Value,
) -> Value {
    let mut args = normalize_argument_value(input);
    let Some(obj) = args.as_object_mut() else {
        return args;
    };
    let params = effective_parameters(record);
    let Some(props) = params
        .as_ref()
        .and_then(|p| p.get("properties"))
        .and_then(|p| p.as_object())
    else {
        return args;
    };

    if !attachments.is_empty() {
        let ids: Vec<Value> = session_attachment_file_ids(attachments)
            .into_iter()
            .map(Value::String)
            .collect();
        for (key, schema) in props {
            let typ = schema.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if obj.get(key).is_some_and(argument_value_present) {
                continue;
            }
            let key_norm = key.to_lowercase().replace('_', "");
            let is_file_ids = key_norm == "fileids";
            let is_file_id = key_norm == "fileid";
            if is_file_ids && typ == "array" {
                obj.insert(key.clone(), Value::Array(ids.clone()));
            } else if is_file_id && typ == "string" && attachments.len() == 1 {
                obj.insert(key.clone(), Value::String(attachments[0].file_id.clone()));
            }
        }
    }

    let question = user_message.trim();
    if !question.is_empty() {
        for (key, schema) in props {
            if key.to_lowercase() != "question" {
                continue;
            }
            if schema.get("type").and_then(|t| t.as_str()) != Some("string") {
                continue;
            }
            if obj.get(key).is_some_and(argument_value_present) {
                continue;
            }
            obj.insert(key.clone(), Value::String(question.to_string()));
        }
    }

    args
}

/// Evaluate JSON expression references of the form `$.steps[N].result.path` or `$.results[key].result.path`
fn evaluate_json_reference(
    expr: &str,
    results: &[TaskResult],
    overlay_results_start: Option<usize>,
) -> Option<Value> {
    if !expr.starts_with("$.") {
        return None;
    }
    let parts: Vec<&str> = expr[2..].split('.').collect();
    if parts.is_empty() {
        return None;
    }

    let mut current_val = None;

    if parts[0].starts_with("steps[") && parts[0].ends_with(']') {
        let idx_str = &parts[0][6..parts[0].len() - 1];
        if let Ok(overlay_idx) = idx_str.parse::<usize>() {
            let global_idx = overlay_results_start
                .map(|start| start + overlay_idx)
                .unwrap_or(overlay_idx);
            if let Some(task_res) = results.get(global_idx) {
                current_val = Some(task_res.result.clone());
            }
        }
    } else if parts[0].starts_with("results[") && parts[0].ends_with(']') {
        let key = &parts[0][8..parts[0].len() - 1];
        if let Some(task_res) = results
            .iter()
            .find(|r| r.skill_id == key || r.tool_id.as_deref() == Some(key))
        {
            current_val = Some(task_res.result.clone());
        }
    }

    let mut current = current_val?;

    let mut i = 1;
    while i < parts.len() {
        let part = parts[i];
        if part == "result" {
            if let Some(s) = current.as_str()
                && let Ok(parsed) = serde_json::from_str::<Value>(s)
            {
                current = parsed;
            }
        } else {
            current = current.get(part)?.clone();
        }
        i += 1;
    }

    Some(current)
}

fn step_index_from_reference_expr(expr: &str) -> Option<usize> {
    let trimmed = expr.trim();
    if !trimmed.starts_with("$.steps[") {
        return None;
    }
    let rest = &trimmed[8..];
    let end = rest.find(']')?;
    rest[..end].parse().ok()
}

fn visit_step_placeholders(value: &Value, f: &mut dyn FnMut(usize)) {
    match value {
        Value::String(s) => {
            let mut search_from = 0;
            while let Some(start_idx) = s[search_from..].find("{{") {
                let abs_start = search_from + start_idx;
                if let Some(end_rel) = s[abs_start..].find("}}") {
                    let abs_end = abs_start + end_rel;
                    let expr = s[abs_start + 2..abs_end].trim();
                    if let Some(idx) = step_index_from_reference_expr(expr) {
                        f(idx);
                    }
                    search_from = abs_end + 2;
                } else {
                    break;
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                visit_step_placeholders(item, f);
            }
        }
        Value::Object(obj) => {
            for val in obj.values() {
                visit_step_placeholders(val, f);
            }
        }
        _ => {}
    }
}

/// True when any `{{$.steps[N].…}}` placeholder remains in `value`.
pub fn contains_step_placeholder(value: &Value) -> bool {
    let mut found = false;
    visit_step_placeholders(value, &mut |_| found = true);
    found
}

/// True when a step placeholder references a future overlay step (`N >= current_task_index`).
pub fn has_unresolved_future_step_placeholder(value: &Value, current_task_index: usize) -> bool {
    has_unresolved_future_step_placeholder_with_prior(value, current_task_index, 0)
}

/// Like `has_unresolved_future_step_placeholder` but allows `steps[0..prior_completed)` refs
/// from a preserved overlay window (`revise_plan`, Phase 7.8).
pub fn has_unresolved_future_step_placeholder_with_prior(
    value: &Value,
    current_task_index: usize,
    prior_completed_steps: usize,
) -> bool {
    let mut future = false;
    visit_step_placeholders(value, &mut |idx| {
        if idx >= prior_completed_steps + current_task_index {
            future = true;
        }
    });
    future
}

/// Traverse Value to replace dynamic placeholders `{{$.steps[N].result.path}}` with values from prior execution results.
pub fn resolve_parameter_references_for_overlay(
    value: &mut Value,
    results: &[TaskResult],
    overlay_results_start: Option<usize>,
) {
    match value {
        Value::String(s) => {
            if s.starts_with("{{") && s.ends_with("}}") {
                let expr = &s[2..s.len() - 2].trim();
                if let Some(resolved) =
                    evaluate_json_reference(expr, results, overlay_results_start)
                {
                    *value = resolved;
                    return;
                }
            }

            let mut new_s = s.clone();
            while let Some(start_idx) = new_s.find("{{") {
                if let Some(end_idx) = new_s[start_idx..].find("}}") {
                    let absolute_end = start_idx + end_idx;
                    let expr = &new_s[start_idx + 2..absolute_end].trim();
                    if let Some(resolved) =
                        evaluate_json_reference(expr, results, overlay_results_start)
                    {
                        let replacement = match resolved {
                            Value::String(ref val_s) => val_s.clone(),
                            other => other.to_string(),
                        };
                        new_s.replace_range(start_idx..absolute_end + 2, &replacement);
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            *value = Value::String(new_s);
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                resolve_parameter_references_for_overlay(item, results, overlay_results_start);
            }
        }
        Value::Object(obj) => {
            for (_, val) in obj.iter_mut() {
                resolve_parameter_references_for_overlay(val, results, overlay_results_start);
            }
        }
        _ => {}
    }
}

/// Non-overlay callers: `$.steps[N]` indexes the global `results` vector.
pub fn resolve_parameter_references(value: &mut Value, results: &[TaskResult]) {
    resolve_parameter_references_for_overlay(value, results, None);
}

/// Server-side prefill for file-id tool parameters from session attachments.
pub fn prefill_attachment_arguments(
    record: &SkillRecord,
    attachments: &[SessionAttachment],
    input: &Value,
) -> Value {
    prefill_tool_arguments(record, attachments, "", input)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowState {
    pub version: u32,
    pub session_id: String,
    pub correlation_id: String,
    pub phase: WorkflowPhase,
    pub user_message: String,
    /// Uploaded files for this session (compose time); used for file_ids tool args.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<SessionAttachment>,
    pub agents: Vec<AgentRef>,
    pub history: Vec<HistoryEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<PlanOutput>,
    #[serde(default)]
    pub results: Vec<TaskResult>,
    #[serde(default)]
    pub usage: TokenUsage,
    #[serde(default)]
    pub task_index: u32,
    #[serde(default)]
    pub supervisor_history: Vec<SupervisorOutcome>,
    /// Full Kriya agent records for execution (mirrored during migration).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kriya_agents: Option<Vec<Value>>,
    /// Session-scoped skill catalog built at compose time.
    #[serde(default, skip_serializing_if = "SkillCatalog::is_empty")]
    pub skill_catalog: SkillCatalog,
    /// Loop guard limits and counters (Phase 6.2).
    #[serde(default)]
    pub guards: LoopGuards,
    /// Active assistant Mongo id at compose time (Phase 6.4 authz).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_assistant_id: Option<String>,
    /// Active assistant slug at compose time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_assistant_slug: Option<String>,
    /// Agent slugs allowed for this session catalog.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_agent_slugs: Vec<String>,
    /// Macro agent jobs for orchestrator v4 (current turn).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agent_jobs: Vec<AgentJob>,
    /// Cross-turn orchestration memory (Phase 7.1+).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub orchestration_ledger: Vec<OrchestrationLedgerEntry>,
    /// Parallel v4 plan (keeps v3 `plan` unset during v4 compose).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_plan: Option<AgentPlanOutput>,
    #[serde(default)]
    pub plan_kind: PlanKind,
    /// Index of the agent job currently running the agent planner loop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_job_index: Option<u32>,
    /// Escalations when agent planner fails or hits turn limit.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub macro_escalations: Vec<MacroEscalationEntry>,
    /// Macro validator audit trail (Phase 7.4).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub macro_validations: Vec<MacroValidationEntry>,
    /// Latest validator outcome for compose failure context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_validation: Option<MacroValidationEntry>,
    /// Assistant systemContext at compose time (macro validator / compose).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub assistant_system_context: String,
    /// Parsed ## Orchestration Contract from assistant systemContext.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orchestration_contract: Option<OrchestrationContract>,
    /// Mongo business workflow instance id bound for this execution session turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_workflow_id: Option<String>,
    /// Optimistic-lock version of the bound instance (for 9.5 patch expected_version).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_workflow_version: Option<u64>,
    /// Projected snapshot from SmritiClient (PHI-redacted); planner consumes in 9.4.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_workflow_snapshot: Option<serde_json::Value>,
    /// Session user id for WorkflowAuthz on supervisor patch path (set at compose).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_user_id: Option<String>,
    /// Last workflowPatch validation errors for agent planner feedback (cleared on successful patch).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_workflow_patch_errors: Option<Vec<WorkflowFieldError>>,
    /// Result count at the start of the current micro-plan execution (Phase 7.6 / Multi-tool).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_job_results_start: Option<usize>,
    /// Completed steps in the current agent-planner overlay (Phase 7.6).
    /// Tracked separately from `results.len()` so duplicate `skill_id`s in one micro-plan
    /// cannot stall sequential dispatch.
    #[serde(default)]
    pub overlay_steps_completed: u32,
    /// Planner action that initiated the current overlay (`execute_plan` | `revise_plan`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay_init_action: Option<String>,
}

impl WorkflowState {
    pub fn new(session_id: String, correlation_id: String, user_message: String) -> Self {
        Self {
            version: WORKFLOW_STATE_VERSION,
            session_id,
            correlation_id,
            phase: WorkflowPhase::Planning,
            user_message,
            attachments: Vec::new(),
            agents: Vec::new(),
            history: Vec::new(),
            plan: None,
            results: Vec::new(),
            usage: TokenUsage::default(),
            task_index: 0,
            supervisor_history: Vec::new(),
            kriya_agents: None,
            skill_catalog: SkillCatalog::default(),
            guards: LoopGuards::default(),
            active_assistant_id: None,
            active_assistant_slug: None,
            active_agent_slugs: Vec::new(),
            agent_jobs: Vec::new(),
            orchestration_ledger: Vec::new(),
            agent_plan: None,
            plan_kind: PlanKind::ToolPlanV3,
            active_job_index: None,
            macro_escalations: Vec::new(),
            macro_validations: Vec::new(),
            last_validation: None,
            assistant_system_context: String::new(),
            orchestration_contract: None,
            active_workflow_id: None,
            active_workflow_version: None,
            active_workflow_snapshot: None,
            session_user_id: None,
            pending_workflow_patch_errors: None,
            active_job_results_start: None,
            overlay_steps_completed: 0,
            overlay_init_action: None,
        }
    }

    pub fn active_agent_job(&self) -> Option<&AgentJob> {
        let idx = self.active_job_index?;
        self.agent_jobs.get(idx as usize)
    }

    pub fn active_agent_job_mut(&mut self) -> Option<&mut AgentJob> {
        let idx = self.active_job_index?;
        self.agent_jobs.get_mut(idx as usize)
    }

    pub fn session_gist(&self) -> Option<String> {
        self.agent_plan
            .as_ref()
            .and_then(|p| p.session_gist.clone())
            .or_else(|| {
                self.orchestration_ledger
                    .last()
                    .and_then(|e| e.session_gist.clone())
            })
    }

    /// Allowed agent slugs for authz filtering.
    pub fn active_agent_set(&self) -> HashSet<String> {
        self.active_agent_slugs.iter().cloned().collect()
    }

    /// Stable fingerprint of assistant + agent slugs + catalog tool ids (assistant-switch detection).
    pub fn authz_fingerprint(&self) -> String {
        let mut tool_ids = self.skill_catalog.tool_ids();
        tool_ids.sort();
        let mut slugs = self.active_agent_slugs.clone();
        slugs.sort();
        let assistant_id = self.active_assistant_id.as_deref().unwrap_or("");
        let assistant_slug = self.active_assistant_slug.as_deref().unwrap_or("");
        format!(
            "{assistant_id}|{assistant_slug}|{}|{}",
            slugs.join(","),
            tool_ids.join(",")
        )
    }

    pub fn redis_key(session_id: &str) -> String {
        format!("workflow:{session_id}")
    }

    pub fn kriya_session_key(session_id: &str) -> String {
        format!("kriya:session:{session_id}")
    }

    pub fn remaining_tasks(&self) -> Vec<PlannedTask> {
        let completed = self.results.len();
        self.plan
            .as_ref()
            .map(|p| p.tasks.iter().skip(completed).cloned().collect())
            .unwrap_or_default()
    }

    pub fn current_task(&self) -> Option<PlannedTask> {
        let idx = self.results.len();
        self.plan.as_ref().and_then(|p| p.tasks.get(idx).cloned())
    }

    /// Build Kriya-compatible execution state for skill execution.
    pub fn to_kriya_execution_state(&self) -> Value {
        let tasks: Value = self
            .plan
            .as_ref()
            .map(|p| {
                Value::Array(
                    p.tasks
                        .iter()
                        .map(PlannedTask::to_kriya_task_value)
                        .collect(),
                )
            })
            .unwrap_or_else(|| json!([]));
        json!({
            "session": self.session_id,
            "user_message": self.user_message,
            "history": self.history,
            "agent_response": { "tasks": tasks.clone() },
            "agents": self.kriya_agents.clone().unwrap_or_default(),
            "status": "executing",
            "tasks": tasks,
            "results": self.results,
            "usage": self.usage,
        })
    }
}

/// Append a skill result and advance task index.
pub fn append_task_result(
    state: &mut WorkflowState,
    skill_id: String,
    result: Value,
    usage: Option<&TokenUsage>,
) {
    append_task_result_with_record(state, skill_id, result, usage, None);
}

/// Append a skill result with optional resolved catalog metadata (Phase 6.5 audit).
pub fn append_task_result_with_record(
    state: &mut WorkflowState,
    skill_id: String,
    result: Value,
    usage: Option<&TokenUsage>,
    record: Option<&SkillRecord>,
) {
    let mut task_result = TaskResult {
        skill_id,
        result,
        status: Some("completed".to_string()),
        tool_id: None,
        agent_slug: None,
        mongo_skill_id: None,
        skill_version: None,
    };
    if let Some(rec) = record {
        task_result.tool_id = Some(rec.tool_id.clone());
        task_result.agent_slug = Some(rec.skill_ref.agent_slug.clone());
        task_result.mongo_skill_id = Some(rec.mongo_skill_id.clone());
        task_result.skill_version = rec.skill_version.clone();
    }
    state.results.push(task_result);
    state.task_index = state.results.len() as u32;
    if let Some(u) = usage {
        state.usage.prompt_tokens += u.prompt_tokens;
        state.usage.completion_tokens += u.completion_tokens;
        state.usage.total_tokens += u.total_tokens;
    }
}

/// Replace remaining plan tasks after a supervisor replan.
pub fn apply_replan(state: &mut WorkflowState, remaining_tasks: Vec<PlannedTask>) {
    if let Some(plan) = state.plan.as_mut() {
        let done: Vec<PlannedTask> = plan
            .tasks
            .iter()
            .take(state.results.len())
            .cloned()
            .collect();
        plan.tasks = done.into_iter().chain(remaining_tasks).collect();
    }
    state.task_index = state.results.len() as u32;
    state.guards.replan_count += 1;
}

/// Validate and apply a workflow phase transition.
pub fn transition_phase(state: &mut WorkflowState, to: WorkflowPhase) -> Result<(), WorkflowError> {
    let from = state.phase.clone();
    let valid = matches!(
        (&from, &to),
        (WorkflowPhase::Planning, WorkflowPhase::Executing)
            | (
                WorkflowPhase::Planning,
                WorkflowPhase::AwaitingAgentDelegation
            )
            | (WorkflowPhase::Planning, WorkflowPhase::Done)
            | (WorkflowPhase::Planning, WorkflowPhase::Failed)
            | (
                WorkflowPhase::AwaitingAgentDelegation,
                WorkflowPhase::Executing
            )
            | (WorkflowPhase::Executing, WorkflowPhase::Executing)
            | (
                WorkflowPhase::Executing,
                WorkflowPhase::AwaitingAgentDelegation
            )
            | (WorkflowPhase::Executing, WorkflowPhase::AwaitingMacroReplan)
            | (WorkflowPhase::Executing, WorkflowPhase::Composing)
            | (WorkflowPhase::Executing, WorkflowPhase::Failed)
            | (WorkflowPhase::Composing, WorkflowPhase::Done)
            | (WorkflowPhase::Composing, WorkflowPhase::Failed)
            | (
                WorkflowPhase::AwaitingMacroReplan,
                WorkflowPhase::AwaitingAgentDelegation
            )
            | (WorkflowPhase::AwaitingMacroReplan, WorkflowPhase::Composing)
            | (WorkflowPhase::AwaitingMacroReplan, WorkflowPhase::Failed)
    );
    if !valid {
        return Err(WorkflowError::InvalidTransition { from, to });
    }
    state.phase = to;
    Ok(())
}

/// Snapshot event types for Smriti audit trail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotEventType {
    Planned,
    TaskCompleted,
    Supervisor,
    Composed,
    DirectAnswer,
    Failed,
    HeuristicSkip,
    AgentPlanV4,
    AgentPlannerTurn,
    AgentJobSummary,
    MacroEscalation,
    MacroValidation,
    MacroReplan,
}

impl SnapshotEventType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::TaskCompleted => "task_completed",
            Self::Supervisor => "supervisor",
            Self::Composed => "composed",
            Self::DirectAnswer => "direct_answer",
            Self::Failed => "failed",
            Self::HeuristicSkip => "heuristic_skip",
            Self::AgentPlanV4 => "agent_plan_v4",
            Self::AgentPlannerTurn => "agent_planner_turn",
            Self::AgentJobSummary => "agent_job_summary",
            Self::MacroEscalation => "macro_escalation",
            Self::MacroValidation => "macro_validation",
            Self::MacroReplan => "macro_replan",
        }
    }
}

/// Strip heavy skill code from agents before audit persistence.
pub fn snapshot_safe_state(state: &WorkflowState) -> WorkflowState {
    let mut safe = state.clone();
    if let Some(agents) = safe.kriya_agents.as_mut() {
        for agent in agents.iter_mut() {
            if let Some(skills) = agent.get_mut("skills").and_then(|s| s.as_array_mut()) {
                for skill in skills.iter_mut() {
                    if let Some(obj) = skill.as_object_mut() {
                        obj.remove("code");
                    }
                }
            }
        }
    }
    safe
}

/// Compact skill identity block for audit snapshots (Phase 6.5).
pub fn build_skill_context(state: &WorkflowState) -> Value {
    let mut catalog_tool_ids = state.skill_catalog.tool_ids();
    catalog_tool_ids.sort();

    let current_task = state.current_task().map(|t| {
        json!({
            "toolId": t.tool_id,
            "agentSlug": t
                .tool_id
                .as_deref()
                .and_then(|tid| tid.split_once("__").map(|(a, _)| a.to_string()))
                .or_else(|| {
                    state
                        .skill_catalog
                        .resolve_assigned(&t.assigned_agent, t.skill_id.as_deref().unwrap_or(""))
                        .ok()
                        .map(|r| r.skill_ref.agent_slug)
                }),
            "skillKey": t.skill_id,
            "assignedAgent": t.assigned_agent,
        })
    });

    let completed_tools: Vec<Value> = state
        .results
        .iter()
        .map(|r| {
            json!({
                "toolId": r.tool_id,
                "agentSlug": r.agent_slug,
                "mongoSkillId": r.mongo_skill_id,
                "skillVersion": r.skill_version,
                "skillId": r.skill_id,
            })
        })
        .collect();

    json!({
        "catalogToolIds": catalog_tool_ids,
        "currentTask": current_task,
        "completedTools": completed_tools,
    })
}

/// Build a Smriti `workflowSnapshots` create-record payload.
pub fn build_snapshot_record(
    state: &WorkflowState,
    event_type: SnapshotEventType,
    sequence: u32,
) -> Value {
    json!({
        "collectionName": "workflowSnapshots",
        "payload": {
            "sessionId": state.session_id,
            "correlationId": state.correlation_id,
            "sequence": sequence,
            "eventType": event_type.as_str(),
            "phase": serde_json::to_value(&state.phase).unwrap_or(json!("planning")),
            "workflowState": snapshot_safe_state(state),
            "skillContext": build_skill_context(state),
            "createdAt": chrono::Utc::now().to_rfc3339(),
            "agentInteractionId": null
        }
    })
}

/// Build routing event for SaveWorkflowSnapshot → Smriti.
pub fn snapshot_routing_event() -> Value {
    json!({
        "event_key": "SaveWorkflowSnapshot",
        "topics": ["smriti"],
        "condition": "workflow_audit"
    })
}

/// Append snapshot to Redis audit buffer for later flush on terminal events.
pub async fn append_audit_snapshot<C>(
    conn: &mut C,
    state: &WorkflowState,
    event_type: SnapshotEventType,
) -> Result<u32, WorkflowError>
where
    C: redis::AsyncCommands + Send,
{
    let key = format!("workflow:audit:{}", state.session_id);
    let len: i64 = conn
        .llen(&key)
        .await
        .map_err(|e| WorkflowError::Redis(e.to_string()))?;
    let sequence = len as u32;
    let record = build_snapshot_record(state, event_type, sequence + 1);
    let json =
        serde_json::to_string(&record).map_err(|e| WorkflowError::Serialize(e.to_string()))?;
    conn.rpush::<_, _, ()>(&key, json)
        .await
        .map_err(|e| WorkflowError::Redis(e.to_string()))?;
    conn.expire::<_, ()>(&key, WORKFLOW_REDIS_TTL_SECS as i64)
        .await
        .map_err(|e| WorkflowError::Redis(e.to_string()))?;
    Ok(sequence + 1)
}

/// Load all buffered audit snapshots for a session.
pub async fn load_audit_snapshots<C>(
    conn: &mut C,
    session_id: &str,
) -> Result<Vec<Value>, WorkflowError>
where
    C: redis::AsyncCommands + Send,
{
    let key = format!("workflow:audit:{session_id}");
    let items: Vec<String> = conn
        .lrange(&key, 0, -1)
        .await
        .map_err(|e| WorkflowError::Redis(e.to_string()))?;
    Ok(items
        .into_iter()
        .filter_map(|s| serde_json::from_str(&s).ok())
        .collect())
}

/// Load Kriya execution state: prefer workflow key when V3 on.
pub async fn load_execution_state<C>(
    conn: &mut C,
    session_id: &str,
) -> Result<Option<Value>, WorkflowError>
where
    C: redis::AsyncCommands + Send,
{
    if let Some(wf) = load_workflow_state(conn, session_id).await? {
        return Ok(Some(wf.to_kriya_execution_state()));
    }
    if !workflow_kriya_session_read_through_enabled() {
        return Ok(None);
    }
    let key = WorkflowState::kriya_session_key(session_id);
    let json: Option<String> = conn
        .get(&key)
        .await
        .map_err(|e| WorkflowError::Redis(e.to_string()))?;
    match json {
        Some(s) if !s.is_empty() => {
            let state: Value =
                serde_json::from_str(&s).map_err(|e| WorkflowError::Serialize(e.to_string()))?;
            Ok(Some(state))
        }
        _ => Ok(None),
    }
}

/// User-safe message when supervisor cannot proceed.
pub const SUPERVISOR_USER_FAIL_MESSAGE: &str =
    "We couldn't complete your request right now. Please try again.";

/// Deterministic supervisor decision when the LLM call or parse fails.
pub fn supervisor_heuristic_fallback(workflow: &WorkflowState) -> SupervisorOutcome {
    let has_agents = workflow
        .kriya_agents
        .as_ref()
        .map(|a| !a.is_empty())
        .unwrap_or(false);

    if workflow.plan.is_none() || !has_agents {
        return SupervisorOutcome {
            decision: SupervisorDecision::Fail,
            reasoning: SUPERVISOR_USER_FAIL_MESSAGE.to_string(),
            remaining_tasks: None,
            notes: Some("heuristic_fallback".to_string()),
        };
    }

    let remaining = workflow.remaining_tasks();
    let current = workflow.current_task();

    if remaining.is_empty() || current.is_none() {
        return SupervisorOutcome {
            decision: SupervisorDecision::Compose,
            reasoning: "All planned tasks completed; composing answer.".to_string(),
            remaining_tasks: None,
            notes: Some("heuristic_fallback".to_string()),
        };
    }

    if let Some(task) = current
        && let Some(sid) = task.skill_id.as_deref().filter(|s| !s.is_empty())
        && workflow.results.iter().any(|r| r.skill_id == sid)
    {
        return SupervisorOutcome {
            decision: SupervisorDecision::Compose,
            reasoning: "All remaining skills already executed; composing answer.".to_string(),
            remaining_tasks: None,
            notes: Some("heuristic_fallback".to_string()),
        };
    }

    SupervisorOutcome {
        decision: SupervisorDecision::Continue,
        reasoning: "Proceeding with next planned task.".to_string(),
        remaining_tasks: None,
        notes: Some("heuristic_fallback".to_string()),
    }
}

/// Parse supervisor LLM JSON into outcome.
pub fn supervisor_outcome_from_json(value: &Value) -> Result<SupervisorOutcome, String> {
    let decision_str = value
        .get("decision")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "supervisor response missing decision".to_string())?;
    let decision = match decision_str.to_lowercase().as_str() {
        "continue" => SupervisorDecision::Continue,
        "replan" => SupervisorDecision::Replan,
        "compose" => SupervisorDecision::Compose,
        "fail" => SupervisorDecision::Fail,
        other => return Err(format!("unknown supervisor decision: {other}")),
    };

    let remaining_tasks: Option<Vec<PlannedTask>> = value
        .get("remaining_tasks")
        .and_then(|t| t.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| serde_json::from_value(t.clone()).ok())
                .collect()
        });

    Ok(SupervisorOutcome {
        decision,
        reasoning: value
            .get("reasoning")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        remaining_tasks,
        notes: value
            .get("notes")
            .and_then(|v| v.as_str())
            .map(String::from),
    })
}

/// Returns (raw task count, successfully parsed task count) from supervisor JSON.
pub fn supervisor_remaining_tasks_counts(value: &Value) -> (usize, usize) {
    let Some(arr) = value.get("remaining_tasks").and_then(|t| t.as_array()) else {
        return (0, 0);
    };
    let parsed = arr
        .iter()
        .filter(|t| serde_json::from_value::<PlannedTask>((*t).clone()).is_ok())
        .count();
    (arr.len(), parsed)
}

/// Load workflow state from Redis.
pub async fn load_workflow_state<C>(
    conn: &mut C,
    session_id: &str,
) -> Result<Option<WorkflowState>, WorkflowError>
where
    C: redis::AsyncCommands + Send,
{
    let key = WorkflowState::redis_key(session_id);
    let json: Option<String> = conn
        .get(&key)
        .await
        .map_err(|e| WorkflowError::Redis(e.to_string()))?;
    match json {
        Some(s) if !s.is_empty() => {
            let state: WorkflowState =
                serde_json::from_str(&s).map_err(|e| WorkflowError::Serialize(e.to_string()))?;
            Ok(Some(state))
        }
        _ => Ok(None),
    }
}

/// Persist workflow state to Redis (`workflow:{session_id}` is the sole writer path).
pub async fn save_workflow_state<C>(
    conn: &mut C,
    state: &WorkflowState,
) -> Result<(), WorkflowError>
where
    C: redis::AsyncCommands + Send,
{
    let key = WorkflowState::redis_key(&state.session_id);
    let json = serde_json::to_string(state).map_err(|e| WorkflowError::Serialize(e.to_string()))?;
    conn.set_ex::<_, _, ()>(&key, json, WORKFLOW_REDIS_TTL_SECS)
        .await
        .map_err(|e| WorkflowError::Redis(e.to_string()))?;
    Ok(())
}

/// Typed compose-steps request; accepts legacy positional array via adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ComposeStepsInput {
    Typed(ComposeStepsRequest),
    Legacy(Vec<Value>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeStepsRequest {
    #[serde(rename = "chatConfig")]
    pub chat_config: Vec<Value>,
    pub session: Vec<Value>,
    pub history: Vec<Value>,
    #[serde(
        rename = "alternateAgents",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub alternate_agents: Option<Vec<Value>>,
    /// Optional compose-time override for active assistant (Phase 6.4).
    #[serde(
        rename = "activeAssistantId",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub active_assistant_id: Option<String>,
    #[serde(
        rename = "activeAssistantSlug",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub active_assistant_slug: Option<String>,
    /// Explicit business workflow instance id (HITL picker handoff stub).
    #[serde(
        rename = "activeWorkflowId",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub active_workflow_id: Option<String>,
}

impl ComposeStepsRequest {
    /// Maps today's positional indices: [0] chat config, [1] session, [2] history, [3] alternate agents.
    pub fn from_legacy_array(payload: Vec<Value>) -> Result<Self, WorkflowError> {
        if payload.len() < 3 {
            return Err(WorkflowError::LegacyPayloadTooShort(3));
        }
        Ok(Self {
            chat_config: payload[0].as_array().cloned().unwrap_or_default(),
            session: payload[1].as_array().cloned().unwrap_or_default(),
            history: payload[2].as_array().cloned().unwrap_or_default(),
            alternate_agents: payload.get(3).and_then(|v| v.as_array().cloned()),
            active_assistant_id: None,
            active_assistant_slug: None,
            active_workflow_id: None,
        })
    }

    pub fn session_id(&self) -> Option<&str> {
        self.session
            .first()
            .and_then(|v| v.get("_id"))
            .and_then(|v| v.get("$oid"))
            .and_then(|v| v.as_str())
    }

    pub fn system_context(&self) -> &str {
        self.chat_config
            .first()
            .and_then(|item| item.get("systemContext"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
    }

    pub fn session_doc(&self) -> Option<&Value> {
        self.session.first()
    }

    pub fn chat_config_doc(&self) -> Option<&Value> {
        self.chat_config.first()
    }

    /// Raw populated agents from chat config when slugs are present.
    pub fn chat_agents(&self) -> Vec<Value> {
        self.chat_config_doc()
            .and_then(|c| c.get("agents"))
            .and_then(|a| a.as_array())
            .filter(|agents| agents.first().and_then(|agent| agent.get("slug")).is_some())
            .cloned()
            .unwrap_or_default()
    }

    pub fn alternate_agents_list(&self) -> Vec<Value> {
        self.alternate_agents.clone().unwrap_or_default()
    }

    pub fn uses_alternate_agents(&self) -> bool {
        !self.chat_agents().is_empty()
    }

    /// Raw agent pool: chat agents when populated, else alternate agents.
    pub fn raw_agent_pool(&self) -> Vec<Value> {
        let chat = self.chat_agents();
        if !chat.is_empty() {
            chat
        } else {
            self.alternate_agents_list()
        }
    }

    pub fn history_entries(&self) -> &[Value] {
        &self.history
    }
}

impl ComposeStepsInput {
    pub fn into_request(self) -> Result<ComposeStepsRequest, WorkflowError> {
        match self {
            Self::Typed(req) => Ok(req),
            Self::Legacy(arr) => ComposeStepsRequest::from_legacy_array(arr),
        }
    }
}

pub fn agent_planner_breaker_tripped(job: &AgentJob) -> bool {
    job.agent_planner_turn_count >= agent_planner_max_turns()
}

/// Record macro escalation after agent planner failure or circuit breaker.
pub fn append_macro_escalation(state: &mut WorkflowState, entry: MacroEscalationEntry) {
    state.macro_escalations.push(entry);
}

pub fn append_macro_validation(state: &mut WorkflowState, entry: MacroValidationEntry) {
    state.last_validation = Some(entry.clone());
    state.macro_validations.push(entry);
}

pub fn macro_replan_cap_reached(state: &WorkflowState) -> bool {
    state.guards.replan_count >= state.guards.max_replans
}

/// Clear transient execution overlay plan (v4 agent planner).
pub fn clear_execution_plan_overlay(state: &mut WorkflowState) {
    state.plan = None;
    state.active_job_results_start = None;
    state.overlay_steps_completed = 0;
    state.overlay_init_action = None;
}

/// Clear overlay plan/tasks but preserve partial result window for `revise_plan` (Phase 7.8).
pub fn clear_overlay_plan_preserve_results_window(state: &mut WorkflowState) {
    state.plan = None;
    state.overlay_steps_completed = 0;
    state.overlay_init_action = None;
}

/// Find the next OnHold job index, if any.
pub fn next_on_hold_job_index(jobs: &[AgentJob]) -> Option<u32> {
    jobs.iter()
        .find(|j| j.status == AgentJobStatus::OnHold)
        .map(|j| j.job_index)
}

/// Mark validated v4 jobs as on hold (Phase 7.1 compose).
pub fn apply_agent_jobs_on_hold(jobs: &mut [AgentJob]) {
    for job in jobs.iter_mut() {
        job.status = AgentJobStatus::OnHold;
    }
}

/// Append a ledger entry after v4 compose.
pub fn append_orchestration_ledger_entry(
    state: &mut WorkflowState,
    plan_notes: String,
    session_gist: Option<String>,
) {
    let turn_index = state.orchestration_ledger.len() as u32;
    let jobs: Vec<AgentJobSnapshot> = state.agent_jobs.iter().map(AgentJob::snapshot).collect();
    state.orchestration_ledger.push(OrchestrationLedgerEntry {
        correlation_id: state.correlation_id.clone(),
        turn_index,
        user_message: state.user_message.clone(),
        plan_notes,
        jobs,
        session_gist,
    });
}

/// Parse orchestrator v4 JSON into `AgentPlanOutput`.
pub fn agent_plan_from_orchestrator_json(
    value: &Value,
    jobs: Vec<AgentJob>,
    outcome: PlannerOutcome,
) -> AgentPlanOutput {
    AgentPlanOutput {
        jobs,
        notes: value
            .get("notes")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        analysis: value
            .get("analysis")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        session_gist: value
            .get("session_gist")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from),
        direct_answer: value
            .get("direct_answer")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from),
        planner_outcome: outcome,
    }
}

/// Parse orchestrator JSON into `PlanOutput`.
pub fn plan_from_orchestrator_json(
    value: &Value,
    outcome: PlannerOutcome,
) -> Result<PlanOutput, serde_json::Error> {
    let tasks: Vec<PlannedTask> = value
        .get("tasks")
        .and_then(|t| t.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| serde_json::from_value(t.clone()).ok())
                .collect()
        })
        .unwrap_or_default();

    Ok(PlanOutput {
        tasks,
        notes: value
            .get("notes")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        analysis: value
            .get("analysis")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        direct_answer: value
            .get("direct_answer")
            .and_then(|v| v.as_str())
            .map(String::from),
        planner_outcome: outcome,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn from_legacy_array_maps_indices() {
        let payload = vec![
            json!([{"systemContext": "ctx", "agents": []}]),
            json!([{"_id": {"$oid": "sess-1"}}]),
            json!([{"userMessage": "hi", "agentResponse": "hello"}]),
            json!([{"name": "Alt Agent", "slug": "alt"}]),
        ];
        let req = ComposeStepsRequest::from_legacy_array(payload).unwrap();
        assert_eq!(req.session_id(), Some("sess-1"));
        assert_eq!(req.system_context(), "ctx");
        assert_eq!(req.history.len(), 1);
        assert_eq!(req.alternate_agents.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn from_legacy_array_requires_three_elements() {
        let payload = vec![json!([]), json!([])];
        assert!(ComposeStepsRequest::from_legacy_array(payload).is_err());
    }

    #[test]
    fn workflow_state_round_trip() {
        let state = WorkflowState {
            version: WORKFLOW_STATE_VERSION,
            session_id: "s1".to_string(),
            correlation_id: "c1".to_string(),
            phase: WorkflowPhase::Executing,
            user_message: "test".to_string(),
            attachments: vec![],
            agents: vec![AgentRef {
                name: "Agent".to_string(),
                slug: Some("agent".to_string()),
            }],
            history: vec![],
            plan: Some(PlanOutput {
                tasks: vec![PlannedTask {
                    step: "step".to_string(),
                    description: "desc".to_string(),
                    assigned_agent: "Agent".to_string(),
                    reason: "reason".to_string(),
                    tool_id: None,
                    skill_id: Some("skill-1".to_string()),
                    input_arguments: Some(json!({"key": "val"})),
                }],
                notes: "notes".to_string(),
                analysis: "analysis".to_string(),
                direct_answer: None,
                planner_outcome: PlannerOutcome::PlannedTasks,
            }),
            results: vec![],
            usage: TokenUsage::default(),
            task_index: 0,
            supervisor_history: vec![],
            kriya_agents: None,
            skill_catalog: SkillCatalog::default(),
            guards: LoopGuards::default(),
            active_assistant_id: None,
            active_assistant_slug: None,
            active_agent_slugs: Vec::new(),
            agent_jobs: vec![],
            orchestration_ledger: vec![],
            agent_plan: None,
            plan_kind: PlanKind::ToolPlanV3,
            active_job_index: None,
            macro_escalations: vec![],
            macro_validations: vec![],
            last_validation: None,
            assistant_system_context: String::new(),
            orchestration_contract: None,
            active_workflow_id: Some("wf-abc".to_string()),
            active_workflow_version: Some(3),
            active_workflow_snapshot: Some(json!({
                "workflowId": "wf-abc",
                "stage": "collect_vendor",
                "stageKind": "collect"
            })),
            session_user_id: Some("user-42".to_string()),
            pending_workflow_patch_errors: Some(vec![WorkflowFieldError {
                field: "vendorId".into(),
                message: "required".into(),
            }]),
            active_job_results_start: None,
            overlay_steps_completed: 0,
            overlay_init_action: None,
        };
        let serialized = serde_json::to_string(&state).unwrap();
        let deserialized: WorkflowState = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.session_id, state.session_id);
        assert_eq!(deserialized.version, WORKFLOW_STATE_VERSION);
        assert_eq!(
            deserialized.plan.as_ref().unwrap().tasks[0].skill_id,
            Some("skill-1".to_string())
        );
        assert_eq!(deserialized.active_workflow_id.as_deref(), Some("wf-abc"));
        assert_eq!(deserialized.active_workflow_version, Some(3));
        assert!(deserialized.active_workflow_snapshot.is_some());
        assert_eq!(deserialized.session_user_id.as_deref(), Some("user-42"));
        assert_eq!(
            deserialized
                .pending_workflow_patch_errors
                .as_ref()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn workflow_state_v3_deserializes_without_workflow_binding_fields() {
        let v3 = json!({
            "version": 3,
            "sessionId": "s1",
            "correlationId": "c1",
            "phase": "executing",
            "userMessage": "hi",
            "agents": [],
            "history": [],
            "results": [],
            "usage": {},
            "orchestrationContract": {
                "outcomeProfile": "lookup_facts"
            }
        });
        let state: WorkflowState = serde_json::from_value(v3).unwrap();
        assert_eq!(state.version, 3);
        assert!(state.active_workflow_id.is_none());
        assert!(state.active_workflow_version.is_none());
        assert!(state.active_workflow_snapshot.is_none());
    }

    #[test]
    fn supervisor_always_enabled() {
        assert!(supervisor_chat_path_enabled());
        assert_eq!(workflow_path_label(), "supervisor");
    }

    #[test]
    fn workflow_flags_supervisor_always_on() {
        let flags = WorkflowFlags::from_env();
        assert!(flags.supervisor_chat);
    }

    #[test]
    fn loop_guards_deserialize_on_old_state() {
        let v1 = json!({
            "version": 1,
            "sessionId": "s1",
            "correlationId": "c1",
            "phase": "executing",
            "userMessage": "hi",
            "agents": [],
            "history": [],
            "results": [],
            "usage": {}
        });
        let state: WorkflowState = serde_json::from_value(v1).unwrap();
        assert_eq!(state.guards.max_steps, 8);
        assert_eq!(state.guards.max_replans, 3);
        assert_eq!(state.guards.replan_count, 0);
    }

    #[test]
    fn guard_max_steps_forces_compose() {
        let mut state = WorkflowState::new("s".into(), "c".into(), "q".into());
        state.guards.max_steps = 8;
        for i in 0..9 {
            state.results.push(TaskResult {
                skill_id: format!("skill_{i}"),
                result: json!(i),
                status: Some("completed".into()),
                tool_id: None,
                agent_slug: None,
                mongo_skill_id: None,
                skill_version: None,
            });
        }
        assert_eq!(guard_breach(&state), Some(GuardBreachReason::MaxSteps));
    }

    #[test]
    fn guard_max_replans_forces_compose() {
        let mut state = WorkflowState::new("s".into(), "c".into(), "q".into());
        state.guards.max_replans = 3;
        state.guards.replan_count = 4;
        assert_eq!(guard_breach(&state), Some(GuardBreachReason::MaxReplans));
    }

    #[test]
    fn guard_cycle_detection() {
        let mut state = WorkflowState::new("s".into(), "c".into(), "q".into());
        state.plan = Some(PlanOutput {
            tasks: vec![
                PlannedTask {
                    step: "1".into(),
                    description: "d".into(),
                    assigned_agent: "A".into(),
                    reason: "r".into(),
                    tool_id: Some("agent__fetch".into()),
                    skill_id: Some("fetch".into()),
                    input_arguments: Some(json!({"q": "x"})),
                },
                PlannedTask {
                    step: "2".into(),
                    description: "d2".into(),
                    assigned_agent: "A".into(),
                    reason: "r".into(),
                    tool_id: Some("agent__fetch".into()),
                    skill_id: Some("fetch".into()),
                    input_arguments: Some(json!({"q": "x"})),
                },
            ],
            notes: "".into(),
            analysis: "".into(),
            direct_answer: None,
            planner_outcome: PlannerOutcome::PlannedTasks,
        });
        append_task_result(&mut state, "fetch".into(), json!("done"), None);
        assert_eq!(guard_breach(&state), Some(GuardBreachReason::Cycle));
    }

    #[test]
    fn v1_state_deserializes_with_defaults() {
        let v1 = json!({
            "version": 1,
            "sessionId": "s1",
            "correlationId": "c1",
            "phase": "executing",
            "userMessage": "hi",
            "agents": [],
            "history": [],
            "results": [],
            "usage": {}
        });
        let state: WorkflowState = serde_json::from_value(v1).unwrap();
        assert_eq!(state.task_index, 0);
        assert!(state.supervisor_history.is_empty());
        assert!(state.active_agent_slugs.is_empty());
        assert!(state.active_assistant_id.is_none());
    }

    #[test]
    fn authz_fingerprint_stable_and_changes_on_switch() {
        let mut state = WorkflowState::new("s".into(), "c".into(), "q".into());
        state.active_assistant_id = Some("asst-a".into());
        state.active_agent_slugs = vec!["agent-a".into()];
        state.skill_catalog = SkillCatalog::from_agents(&[json!({
            "name": "A", "slug": "agent-a",
            "skills": [{ "name": "tool_a", "_id": { "$oid": "1" } }]
        })]);
        let fp_a = state.authz_fingerprint();
        assert!(fp_a.contains("asst-a"));
        assert!(fp_a.contains("agent-a"));
        assert_eq!(fp_a, state.authz_fingerprint());

        state.active_assistant_id = Some("asst-b".into());
        state.active_agent_slugs = vec!["agent-b".into()];
        state.skill_catalog = SkillCatalog::from_agents(&[json!({
            "name": "B", "slug": "agent-b",
            "skills": [{ "name": "tool_b", "_id": { "$oid": "2" } }]
        })]);
        assert_ne!(fp_a, state.authz_fingerprint());
    }

    #[test]
    fn active_agent_set_helper() {
        let mut state = WorkflowState::new("s".into(), "c".into(), "q".into());
        state.active_agent_slugs = vec!["a".into(), "b".into()];
        let set = state.active_agent_set();
        assert_eq!(set.len(), 2);
        assert!(set.contains("a"));
    }

    #[test]
    fn append_task_result_advances_index() {
        let mut state = WorkflowState::new("s".into(), "c".into(), "q".into());
        append_task_result(&mut state, "skill_a".into(), json!("result"), None);
        assert_eq!(state.task_index, 1);
        assert_eq!(state.results.len(), 1);
    }

    #[test]
    fn apply_replan_splices_remaining_tasks() {
        let mut state = WorkflowState::new("s".into(), "c".into(), "q".into());
        state.plan = Some(PlanOutput {
            tasks: vec![
                PlannedTask {
                    step: "1".into(),
                    description: "d".into(),
                    assigned_agent: "A".into(),
                    reason: "r".into(),
                    tool_id: None,
                    skill_id: None,
                    input_arguments: None,
                },
                PlannedTask {
                    step: "2".into(),
                    description: "d".into(),
                    assigned_agent: "A".into(),
                    reason: "r".into(),
                    tool_id: None,
                    skill_id: None,
                    input_arguments: None,
                },
            ],
            notes: "".into(),
            analysis: "".into(),
            direct_answer: None,
            planner_outcome: PlannerOutcome::PlannedTasks,
        });
        append_task_result(&mut state, "s1".into(), json!(1), None);
        apply_replan(
            &mut state,
            vec![PlannedTask {
                step: "3".into(),
                description: "new".into(),
                assigned_agent: "A".into(),
                reason: "r".into(),
                tool_id: None,
                skill_id: Some("new_skill".into()),
                input_arguments: None,
            }],
        );
        assert_eq!(state.plan.as_ref().unwrap().tasks.len(), 2);
        assert_eq!(state.plan.as_ref().unwrap().tasks[1].step, "3");
    }

    #[test]
    fn transition_phase_planning_to_executing() {
        let mut state = WorkflowState::new("s".into(), "c".into(), "q".into());
        assert!(transition_phase(&mut state, WorkflowPhase::Executing).is_ok());
        assert_eq!(state.phase, WorkflowPhase::Executing);
    }

    #[test]
    fn transition_phase_awaiting_macro_replan_to_composing() {
        let mut state = WorkflowState::new("s".into(), "c".into(), "q".into());
        state.phase = WorkflowPhase::AwaitingMacroReplan;
        assert!(transition_phase(&mut state, WorkflowPhase::Composing).is_ok());
        assert_eq!(state.phase, WorkflowPhase::Composing);
    }

    #[test]
    fn transition_phase_rejects_invalid() {
        let mut state = WorkflowState::new("s".into(), "c".into(), "q".into());
        assert!(transition_phase(&mut state, WorkflowPhase::Composing).is_err());
    }

    #[test]
    fn orchestrator_v4_enabled_explicit_false() {
        unsafe { std::env::set_var("WORKFLOW_ORCHESTRATOR_V4", "false") };
        assert!(!orchestrator_v4_enabled());
        unsafe { std::env::remove_var("WORKFLOW_ORCHESTRATOR_V4") };
    }

    #[test]
    fn agent_planner_max_turns_matches_default_constant() {
        assert_eq!(DEFAULT_AGENT_PLANNER_MAX_TURNS, 5);
    }

    #[test]
    fn agent_planner_breaker_trips_at_limit() {
        let job = AgentJob {
            job_index: 0,
            step: "s".into(),
            description: "d".into(),
            assigned_agent: "A".into(),
            assigned_agent_slug: Some("a".into()),
            expected_outcome: "done".into(),
            reason: "r".into(),
            delegation_brief: "brief".into(),
            status: AgentJobStatus::Executing,
            tool_attempts: vec![],
            agent_planner_turn_count: agent_planner_max_turns(),
            pending_arg_failure_retry: false,
            last_failure_reason: None,
            summary: None,
        };
        assert!(agent_planner_breaker_tripped(&job));
    }

    #[test]
    fn transition_phase_planning_to_awaiting_delegation() {
        let mut state = WorkflowState::new("s".into(), "c".into(), "q".into());
        assert!(transition_phase(&mut state, WorkflowPhase::AwaitingAgentDelegation).is_ok());
        assert_eq!(state.phase, WorkflowPhase::AwaitingAgentDelegation);
    }

    #[test]
    fn agent_job_serde_round_trip() {
        let job = AgentJob {
            job_index: 0,
            step: "step".into(),
            description: "desc".into(),
            assigned_agent: "Agent".into(),
            assigned_agent_slug: Some("agent".into()),
            expected_outcome: "outcome met".into(),
            reason: "reason".into(),
            delegation_brief: "brief".into(),
            status: AgentJobStatus::OnHold,
            tool_attempts: vec![],
            agent_planner_turn_count: 0,
            pending_arg_failure_retry: false,
            last_failure_reason: None,
            summary: None,
        };
        let json = serde_json::to_string(&job).unwrap();
        let back: AgentJob = serde_json::from_str(&json).unwrap();
        assert_eq!(back.expected_outcome, "outcome met");
        assert_eq!(back.status, AgentJobStatus::OnHold);
    }

    #[test]
    fn append_orchestration_ledger_entry_appends_snapshot() {
        let mut state = WorkflowState::new("s".into(), "c".into(), "q".into());
        state.agent_jobs.push(AgentJob {
            job_index: 0,
            step: "s".into(),
            description: "d".into(),
            assigned_agent: "A".into(),
            assigned_agent_slug: Some("a".into()),
            expected_outcome: "done".into(),
            reason: "r".into(),
            delegation_brief: String::new(),
            status: AgentJobStatus::OnHold,
            tool_attempts: vec![],
            agent_planner_turn_count: 0,
            pending_arg_failure_retry: false,
            last_failure_reason: None,
            summary: None,
        });
        append_orchestration_ledger_entry(&mut state, "notes".into(), Some("gist".into()));
        assert_eq!(state.orchestration_ledger.len(), 1);
        assert_eq!(
            state.orchestration_ledger[0].jobs[0].expected_outcome,
            "done"
        );
    }

    #[test]
    fn workflow_state_v2_deserializes_with_v3_defaults() {
        let v2 = json!({
            "version": 2,
            "sessionId": "s1",
            "correlationId": "c1",
            "phase": "planning",
            "userMessage": "hi",
            "agents": [],
            "history": [],
            "results": [],
            "usage": {}
        });
        let state: WorkflowState = serde_json::from_value(v2).unwrap();
        assert!(state.agent_jobs.is_empty());
        assert_eq!(state.plan_kind, PlanKind::ToolPlanV3);
    }

    #[test]
    fn build_snapshot_record_has_collection() {
        let state = WorkflowState::new("sess".into(), "corr".into(), "hi".into());
        let rec = build_snapshot_record(&state, SnapshotEventType::Planned, 1);
        assert_eq!(
            rec.get("collectionName").and_then(|v| v.as_str()),
            Some("workflowSnapshots")
        );
        assert!(
            rec.pointer("/payload/skillContext/catalogToolIds")
                .is_some()
        );
    }

    fn three_task_plan_state() -> WorkflowState {
        let mut state = WorkflowState::new("s".into(), "c".into(), "q".into());
        state.plan = Some(PlanOutput {
            tasks: vec![
                PlannedTask {
                    step: "1".into(),
                    description: "d".into(),
                    assigned_agent: "A".into(),
                    reason: "r".into(),
                    tool_id: Some("a__s1".into()),
                    skill_id: Some("s1".into()),
                    input_arguments: None,
                },
                PlannedTask {
                    step: "2".into(),
                    description: "d".into(),
                    assigned_agent: "A".into(),
                    reason: "r".into(),
                    tool_id: Some("a__s2".into()),
                    skill_id: Some("s2".into()),
                    input_arguments: None,
                },
                PlannedTask {
                    step: "3".into(),
                    description: "d".into(),
                    assigned_agent: "A".into(),
                    reason: "r".into(),
                    tool_id: Some("a__s3".into()),
                    skill_id: Some("s3".into()),
                    input_arguments: None,
                },
            ],
            notes: "".into(),
            analysis: "".into(),
            direct_answer: None,
            planner_outcome: PlannerOutcome::PlannedTasks,
        });
        state
    }

    #[test]
    fn multi_step_current_task_advances_with_results() {
        let mut state = three_task_plan_state();
        assert_eq!(
            state.current_task().unwrap().skill_id.as_deref(),
            Some("s1")
        );
        append_task_result(&mut state, "s1".into(), json!(1), None);
        assert_eq!(
            state.current_task().unwrap().skill_id.as_deref(),
            Some("s2")
        );
        append_task_result(&mut state, "s2".into(), json!(2), None);
        assert_eq!(
            state.current_task().unwrap().skill_id.as_deref(),
            Some("s3")
        );
        append_task_result(&mut state, "s3".into(), json!(3), None);
        assert!(state.current_task().is_none());
        assert_eq!(state.task_index, 3);
    }

    #[test]
    fn append_task_result_with_record_enriches_metadata() {
        let catalog = SkillCatalog::from_agents(&[json!({
            "name": "A", "slug": "a",
            "skills": [{ "name": "s1", "_id": { "$oid": "mongo-1" }, "version": "2" }]
        })]);
        let record = catalog.resolve("a", "s1").unwrap();
        let mut state = WorkflowState::new("s".into(), "c".into(), "q".into());
        append_task_result_with_record(&mut state, "s1".into(), json!("ok"), None, Some(&record));
        let r = &state.results[0];
        assert_eq!(r.tool_id.as_deref(), Some("a__s1"));
        assert_eq!(r.agent_slug.as_deref(), Some("a"));
        assert_eq!(r.mongo_skill_id.as_deref(), Some("mongo-1"));
    }

    #[test]
    fn supervisor_outcome_parses_decision() {
        let v = json!({
            "decision": "compose",
            "reasoning": "enough data",
            "notes": "done"
        });
        let o = supervisor_outcome_from_json(&v).unwrap();
        assert_eq!(o.decision, SupervisorDecision::Compose);
    }

    #[test]
    fn to_kriya_execution_state_mirrors_plan_and_results() {
        let mut state = WorkflowState::new("sess-1".into(), "corr".into(), "question".into());
        state.kriya_agents = Some(vec![json!({"name": "Agent A"})]);
        state.plan = Some(PlanOutput {
            tasks: vec![PlannedTask {
                step: "1".into(),
                description: "d".into(),
                assigned_agent: "Agent A".into(),
                reason: "r".into(),
                tool_id: None,
                skill_id: Some("skill-1".into()),
                input_arguments: None,
            }],
            notes: "".into(),
            analysis: "".into(),
            direct_answer: None,
            planner_outcome: PlannerOutcome::PlannedTasks,
        });
        append_task_result(&mut state, "skill-1".into(), json!("done"), None);
        let exec = state.to_kriya_execution_state();
        assert_eq!(exec["tasks"][0]["assigned_agent"].as_str(), Some("Agent A"));
        assert_eq!(exec.get("session").and_then(|v| v.as_str()), Some("sess-1"));
        assert_eq!(
            exec.get("results")
                .and_then(|v| v.as_array())
                .map(|a| a.len()),
            Some(1)
        );
    }

    #[test]
    fn supervisor_remaining_tasks_parses_snake_case() {
        let v = json!({
            "decision": "replan",
            "reasoning": "adjust plan",
            "remaining_tasks": [{
                "step": "2",
                "description": "fetch data",
                "assigned_agent": "Agent A",
                "reason": "needed",
                "skill_id": "fetch_skill",
                "input_arguments": {}
            }]
        });
        let o = supervisor_outcome_from_json(&v).unwrap();
        let tasks = o.remaining_tasks.expect("tasks");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].assigned_agent, "Agent A");
        assert_eq!(tasks[0].skill_id.as_deref(), Some("fetch_skill"));
        let (raw, parsed) = supervisor_remaining_tasks_counts(&v);
        assert_eq!(raw, 1);
        assert_eq!(parsed, 1);
    }

    #[test]
    fn supervisor_heuristic_compose_when_no_remaining_tasks() {
        let mut state = WorkflowState::new("s".into(), "c".into(), "q".into());
        state.kriya_agents = Some(vec![json!({"name": "Agent A"})]);
        state.plan = Some(PlanOutput {
            tasks: vec![PlannedTask {
                step: "1".into(),
                description: "d".into(),
                assigned_agent: "Agent A".into(),
                reason: "r".into(),
                tool_id: None,
                skill_id: Some("skill_a".into()),
                input_arguments: None,
            }],
            notes: "".into(),
            analysis: "".into(),
            direct_answer: None,
            planner_outcome: PlannerOutcome::PlannedTasks,
        });
        append_task_result(&mut state, "skill_a".into(), json!("done"), None);
        let outcome = supervisor_heuristic_fallback(&state);
        assert_eq!(outcome.decision, SupervisorDecision::Compose);
    }

    #[test]
    fn supervisor_heuristic_continue_when_tasks_remain() {
        let mut state = WorkflowState::new("s".into(), "c".into(), "q".into());
        state.kriya_agents = Some(vec![json!({"name": "Agent A"})]);
        state.plan = Some(PlanOutput {
            tasks: vec![
                PlannedTask {
                    step: "1".into(),
                    description: "d".into(),
                    assigned_agent: "Agent A".into(),
                    reason: "r".into(),
                    tool_id: None,
                    skill_id: Some("skill_a".into()),
                    input_arguments: None,
                },
                PlannedTask {
                    step: "2".into(),
                    description: "d2".into(),
                    assigned_agent: "Agent A".into(),
                    reason: "r2".into(),
                    tool_id: None,
                    skill_id: Some("skill_b".into()),
                    input_arguments: None,
                },
            ],
            notes: "".into(),
            analysis: "".into(),
            direct_answer: None,
            planner_outcome: PlannerOutcome::PlannedTasks,
        });
        append_task_result(&mut state, "skill_a".into(), json!("done"), None);
        let outcome = supervisor_heuristic_fallback(&state);
        assert_eq!(outcome.decision, SupervisorDecision::Continue);
    }

    #[test]
    fn prefill_attachment_arguments_fills_file_ids_array() {
        use crate::skill_catalog::{SkillRecord, SkillRef};
        let attachments = vec![SessionAttachment {
            file_name: "doc.docx".into(),
            file_id: "file-abc".into(),
            mime_type: "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                .into(),
        }];
        let record = SkillRecord {
            skill_ref: SkillRef {
                agent_slug: "v".into(),
                skill_key: "analyze".into(),
            },
            tool_id: "v__analyze".into(),
            mongo_skill_id: "m".into(),
            skill_version: None,
            name: "analyze".into(),
            skill_type: None,
            description: None,
            parameters: Some(json!({
                "file_ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "required": true
                },
                "question": { "type": "string", "required": true }
            })),
        };
        let filled =
            prefill_tool_arguments(&record, &attachments, "generate SO interface", &json!({}));
        assert_eq!(filled["file_ids"], json!(["file-abc"]));
        assert_eq!(filled["question"], json!("generate SO interface"));
    }

    #[test]
    fn prefill_tool_arguments_fills_question_from_user_message() {
        use crate::skill_catalog::{SkillRecord, SkillRef};
        let record = SkillRecord {
            skill_ref: SkillRef {
                agent_slug: "interface-agent".into(),
                skill_key: "InterfaceCodeGenerator".into(),
            },
            tool_id: "interface-agent__InterfaceCodeGenerator".into(),
            mongo_skill_id: "m".into(),
            skill_version: None,
            name: "InterfaceCodeGenerator".into(),
            skill_type: None,
            description: None,
            parameters: Some(json!({
                "question": { "type": "string", "required": true }
            })),
        };
        let filled = prefill_tool_arguments(
            &record,
            &[],
            "Generate interface code to import sales orders",
            &json!({}),
        );
        assert_eq!(
            filled["question"],
            json!("Generate interface code to import sales orders")
        );
    }

    #[test]
    fn test_resolve_parameter_references() {
        let results = vec![
            TaskResult {
                skill_id: "ToolA".into(),
                result: json!("{\"success\":true,\"id\":\"resolved-id-123\"}"),
                status: None,
                tool_id: Some("agent__ToolA".into()),
                agent_slug: Some("agent".into()),
                mongo_skill_id: None,
                skill_version: None,
            },
            TaskResult {
                skill_id: "ToolB".into(),
                result: json!({"url": "http://api.com/user"}),
                status: None,
                tool_id: Some("agent__ToolB".into()),
                agent_slug: Some("agent".into()),
                mongo_skill_id: None,
                skill_version: None,
            },
        ];

        let mut args = json!({
            "user_id": "{{$.steps[0].result.id}}",
            "api_url": "{{$.steps[1].result.url}}",
            "nested": {
                "direct": "{{$.results[ToolA].result.id}}"
            },
            "mixed": "ID is {{$.results[agent__ToolB].result.url}}"
        });

        resolve_parameter_references(&mut args, &results);

        assert_eq!(args["user_id"], json!("resolved-id-123"));
        assert_eq!(args["api_url"], json!("http://api.com/user"));
        assert_eq!(args["nested"]["direct"], json!("resolved-id-123"));
        assert_eq!(args["mixed"], json!("ID is http://api.com/user"));
    }

    #[test]
    fn evaluate_json_reference_overlay_relative_steps() {
        let results = vec![
            TaskResult {
                skill_id: "PreTool".into(),
                result: json!({"x": "wrong"}),
                status: None,
                tool_id: Some("agent__PreTool".into()),
                agent_slug: Some("agent".into()),
                mongo_skill_id: None,
                skill_version: None,
            },
            TaskResult {
                skill_id: "PreTool2".into(),
                result: json!({"x": "also-wrong"}),
                status: None,
                tool_id: Some("agent__PreTool2".into()),
                agent_slug: Some("agent".into()),
                mongo_skill_id: None,
                skill_version: None,
            },
            TaskResult {
                skill_id: "OverlayStep0".into(),
                result: json!({"id": "overlay-correct"}),
                status: None,
                tool_id: Some("agent__OverlayStep0".into()),
                agent_slug: Some("agent".into()),
                mongo_skill_id: None,
                skill_version: None,
            },
        ];

        let mut args = json!({"dep": "{{$.steps[0].result.id}}"});
        resolve_parameter_references_for_overlay(&mut args, &results, Some(2));
        assert_eq!(args["dep"], json!("overlay-correct"));

        let mut global_args = json!({"dep": "{{$.steps[0].result.x}}"});
        resolve_parameter_references(&mut global_args, &results);
        assert_eq!(global_args["dep"], json!("wrong"));
    }

    #[test]
    fn evaluate_json_reference_global_fallback() {
        let results = vec![TaskResult {
            skill_id: "Only".into(),
            result: json!({"v": 1}),
            status: None,
            tool_id: None,
            agent_slug: None,
            mongo_skill_id: None,
            skill_version: None,
        }];
        let mut args = json!({"v": "{{$.steps[0].result.v}}"});
        resolve_parameter_references(&mut args, &results);
        assert_eq!(args["v"], json!(1));
    }

    #[test]
    fn resolve_parameter_references_for_overlay_nested() {
        let results = vec![TaskResult {
            skill_id: "A".into(),
            result: json!({"nested": {"key": "val"}}),
            status: None,
            tool_id: None,
            agent_slug: None,
            mongo_skill_id: None,
            skill_version: None,
        }];
        let mut args = json!({
            "outer": {
                "inner": "{{$.steps[0].result.nested.key}}"
            },
            "arr": ["{{$.steps[0].result.nested.key}}"]
        });
        resolve_parameter_references_for_overlay(&mut args, &results, Some(0));
        assert_eq!(args["outer"]["inner"], json!("val"));
        assert_eq!(args["arr"][0], json!("val"));
    }

    #[test]
    fn agent_planner_overlay_max_steps_default() {
        assert_eq!(DEFAULT_AGENT_PLANNER_OVERLAY_MAX_STEPS, 8);
        unsafe {
            std::env::remove_var("AGENT_PLANNER_OVERLAY_MAX_STEPS");
        }
        assert_eq!(
            agent_planner_overlay_max_steps(),
            DEFAULT_AGENT_PLANNER_OVERLAY_MAX_STEPS
        );
        unsafe {
            std::env::set_var("AGENT_PLANNER_OVERLAY_MAX_STEPS", "3");
        }
        assert_eq!(agent_planner_overlay_max_steps(), 3);
        unsafe {
            std::env::remove_var("AGENT_PLANNER_OVERLAY_MAX_STEPS");
        }
    }

    #[test]
    fn agent_planner_arg_failure_free_turns_default_true() {
        unsafe {
            std::env::remove_var("AGENT_PLANNER_ARG_FAILURE_FREE_TURNS");
        }
        assert!(agent_planner_arg_failure_free_turns());
        unsafe {
            std::env::set_var("AGENT_PLANNER_ARG_FAILURE_FREE_TURNS", "false");
        }
        assert!(!agent_planner_arg_failure_free_turns());
        unsafe {
            std::env::remove_var("AGENT_PLANNER_ARG_FAILURE_FREE_TURNS");
        }
    }

    #[test]
    fn has_unresolved_future_step_placeholder_detects_future_ref() {
        let future_ref = json!({"x": "{{$.steps[1].result.id}}"});
        assert!(has_unresolved_future_step_placeholder(&future_ref, 0));
        let prior_ref = json!({"x": "{{$.steps[0].result.id}}"});
        assert!(!has_unresolved_future_step_placeholder(&prior_ref, 1));
    }

    #[test]
    fn has_unresolved_future_step_placeholder_with_prior_allows_completed_refs() {
        let prior_ref = json!({"x": "{{$.steps[0].result.id}}"});
        assert!(!has_unresolved_future_step_placeholder_with_prior(
            &prior_ref, 0, 2
        ));
        assert!(has_unresolved_future_step_placeholder_with_prior(
            &prior_ref, 0, 0
        ));
    }
}
