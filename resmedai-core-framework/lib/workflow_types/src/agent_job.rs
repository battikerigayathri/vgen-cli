use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{PlannerOutcome, WorkflowState};

/// Macro orchestration plan kind (Phase 7.1).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum PlanKind {
    #[default]
    ToolPlanV3,
    AgentJobV4,
}

/// Per-agent job lifecycle (macro layer).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum AgentJobStatus {
    #[default]
    Planned,
    Validated,
    OnHold,
    Delegated,
    Executing,
    Summarized,
    Completed,
    Failed,
    Skipped,
}

/// One agent planner turn (tool execution or terminal decision).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentPlannerAttempt {
    pub turn_index: u32,
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_arguments: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    /// Completed overlay step index when action is execute_plan or revise_plan (Phase 7.8).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay_step_index: Option<u32>,
}

/// Legacy alias — serde reads old persisted state.
pub type AgentMicroAttempt = AgentPlannerAttempt;

/// Stub — populated in Phase 7.3+.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentJobSummary {
    pub outcome_status: String,
    pub summary_text: String,
    pub tools_triggered: Vec<String>,
}

/// Assistant macro validator outcome (Phase 7.4).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MacroValidationEntry {
    pub decision: String,
    pub reasoning: String,
    pub gaps: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replan_rationale: Option<String>,
    pub correlation_id: String,
    /// Structured missing field keys from completeness pre-check (Phase 9.6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missing_required: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completeness_pct: Option<u8>,
}

/// Macro escalation handoff to assistant planner (Phase 7.2+).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MacroEscalationEntry {
    pub job_index: u32,
    pub reason: String,
    pub expected_outcome: String,
    pub delegation_brief: String,
    pub tool_attempts_snapshot: Vec<AgentPlannerAttempt>,
    pub correlation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary_text: Option<String>,
}

/// Macro task assigned to an agent (orchestrator v4).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentJob {
    pub job_index: u32,
    pub step: String,
    pub description: String,
    pub assigned_agent: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigned_agent_slug: Option<String>,
    pub expected_outcome: String,
    pub reason: String,
    #[serde(default)]
    pub delegation_brief: String,
    #[serde(default)]
    pub status: AgentJobStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[serde(alias = "microAttempts", alias = "micro_attempts")]
    pub tool_attempts: Vec<AgentPlannerAttempt>,
    #[serde(default)]
    pub agent_planner_turn_count: u32,
    /// When true, next `run_agent_planner_turn` skips turn-count increment (Phase 7.7 arg-failure free turns).
    #[serde(default)]
    pub pending_arg_failure_retry: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_failure_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<AgentJobSummary>,
}

/// LLM row shape before validation enriches indices and slugs.
/// Accepts camelCase (serde default) and snake_case (orchestrator_schema_v4 property names).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentJobLlmRow {
    pub step: String,
    pub description: String,
    #[serde(alias = "assigned_agent")]
    pub assigned_agent: String,
    #[serde(alias = "expected_outcome")]
    pub expected_outcome: String,
    pub reason: String,
    #[serde(default, alias = "delegation_brief")]
    pub delegation_brief: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentJobSnapshot {
    pub job_index: u32,
    pub assigned_agent_slug: String,
    pub expected_outcome: String,
    pub status: AgentJobStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OrchestrationLedgerEntry {
    pub correlation_id: String,
    pub turn_index: u32,
    pub user_message: String,
    pub plan_notes: String,
    pub jobs: Vec<AgentJobSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_gist: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentPlanOutput {
    pub jobs: Vec<AgentJob>,
    pub notes: String,
    pub analysis: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_gist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direct_answer: Option<String>,
    pub planner_outcome: PlannerOutcome,
}

impl AgentJob {
    pub fn snapshot(&self) -> AgentJobSnapshot {
        let summary_preview = self.summary.as_ref().map(|s| {
            let text = s.summary_text.trim();
            if text.len() <= 200 {
                text.to_string()
            } else {
                format!("{}…", &text[..200])
            }
        });
        AgentJobSnapshot {
            job_index: self.job_index,
            assigned_agent_slug: self.assigned_agent_slug.clone().unwrap_or_default(),
            expected_outcome: self.expected_outcome.clone(),
            status: self.status.clone(),
            summary_preview,
        }
    }
}

/// Distinct tool_ids from agent planner attempts for summary metadata.
pub fn tools_triggered_from_attempts(attempts: &[AgentPlannerAttempt]) -> Vec<String> {
    let mut ids: Vec<String> = attempts
        .iter()
        .filter_map(|a| a.tool_id.as_deref())
        .filter(|id| !id.is_empty())
        .map(String::from)
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

/// Max chars of tool result JSON injected into agent planner prompts.
const TOOL_RESULT_PROMPT_MAX_CHARS: usize = 12_000;

/// Extract the substantive payload from a Kriya skill result (unwrap common envelopes).
fn extract_tool_result_payload(result: &Value) -> Value {
    if !result.is_object() {
        return result.clone();
    }
    let obj = result.as_object().expect("object");
    for key in [
        "data", "output", "result", "results", "people", "body", "response", "payload",
    ] {
        if let Some(v) = obj.get(key)
            && !v.is_null()
        {
            return v.clone();
        }
    }
    const META: &[&str] = &[
        "status",
        "error",
        "skill_id",
        "session",
        "task_index",
        "usage",
    ];
    let filtered: serde_json::Map<String, Value> = obj
        .iter()
        .filter(|(k, _)| !META.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if filtered.is_empty() {
        result.clone()
    } else {
        Value::Object(filtered)
    }
}

fn truncate_json_for_prompt(value: &Value, max_chars: usize) -> String {
    let Ok(mut s) = serde_json::to_string(value) else {
        return "(unserializable result)".to_string();
    };
    if s.len() <= max_chars {
        return s;
    }
    s.truncate(max_chars);
    format!("{s}... [truncated]")
}

/// Format a stored tool result for agent planner / resolver prompts.
pub fn format_tool_result_for_prompt(result: &Value) -> String {
    truncate_json_for_prompt(
        &extract_tool_result_payload(result),
        TOOL_RESULT_PROMPT_MAX_CHARS,
    )
}

/// Human-readable prior tool attempts for agent planner / resolver prompts (Phase 7.5).
pub fn format_tool_attempts_for_prompt(job: &AgentJob) -> String {
    if job.tool_attempts.is_empty() {
        return "(none)".to_string();
    }
    let mut lines = Vec::new();
    let mut last_hitl_halt = false;
    for (i, attempt) in job.tool_attempts.iter().enumerate() {
        lines.push(format!("### Attempt {}", i + 1));
        lines.push(format!("action: {}", attempt.action));
        if let Some(tool_id) = &attempt.tool_id {
            lines.push(format!("tool_id: {tool_id}"));
        }
        if let Some(step_idx) = attempt.overlay_step_index
            && matches!(attempt.action.as_str(), "execute_plan" | "revise_plan")
        {
            lines.push(format!("overlay step: {step_idx} of micro-plan"));
        }
        if let Some(args) = &attempt.input_arguments
            && let Ok(s) = serde_json::to_string(args)
        {
            lines.push(format!("input_arguments: {s}"));
        }
        lines.push(format!("status: {}", attempt.status));
        if let Some(reason) = &attempt.reasoning {
            lines.push(format!("reasoning/error: {reason}"));
        }
        if attempt.status == "arguments_invalid" {
            lines.push(
                "Note: required tool arguments were missing or invalid — extract concrete values from the user message and delegation brief.".to_string(),
            );
            if matches!(attempt.action.as_str(), "execute_plan" | "revise_plan") {
                let step_n = attempt.overlay_step_index.unwrap_or(0);
                lines.push(format!(
                    "Note: overlay dispatch failed — consider revise_plan with remaining tasks from step {step_n}."
                ));
            }
        }
        if let Some(result) = &attempt.result {
            if let Some(err) = result.get("error").and_then(|e| e.as_str()) {
                lines.push(format!("execution error: {err}"));
            } else if let Some(code) = result.get("status").and_then(|s| s.as_str())
                && !matches!(code, "completed" | "success" | "ok")
            {
                lines.push(format!("result status: {code}"));
            }
            let payload = format_tool_result_for_prompt(result);
            if !payload.is_empty() && payload != "null" {
                lines.push(format!("tool result: {payload}"));
            }
            if result.get("config").is_some()
                || result
                    .as_str()
                    .and_then(|s| serde_json::from_str::<Value>(s).ok())
                    .is_some_and(|v| v.get("config").is_some())
            {
                lines.push(
                    "Note: HITL halted micro-plan — use revise_plan or execute_tool for remaining work on next turn.".to_string(),
                );
                last_hitl_halt = true;
            }
        }
        lines.push(String::new());
    }
    if last_hitl_halt && job.status != AgentJobStatus::Completed {
        lines.push(
            "Recovery hint: prior micro-plan halted for HITL; emit revise_plan with remaining non-HITL tasks.".to_string(),
        );
    }
    lines.join("\n")
}

/// True when repeated argument-resolution failures suggest escalation (Phase 7.5).
pub fn argument_resolution_exhausted(job: &AgentJob, tool_id: &str, min_failures: usize) -> bool {
    let failures = job
        .tool_attempts
        .iter()
        .rev()
        .filter(|a| a.tool_id.as_deref() == Some(tool_id))
        .filter(|a| a.status == "arguments_invalid")
        .take(min_failures)
        .count();
    failures >= min_failures
}

/// Server-built workflow context injected into agent planner prompts (Phase 7.3).
pub fn build_agent_handoff_context(workflow: &WorkflowState, job: &AgentJob) -> String {
    let total = workflow.agent_jobs.len();
    let job_num = job.job_index + 1;
    let phase = format!("{:?}", workflow.phase);
    let plan_kind = format!("{:?}", workflow.plan_kind);
    let gist = workflow
        .session_gist()
        .unwrap_or_else(|| "(none)".to_string());

    let mut lines = vec![
        format!("Workflow phase: {phase} (plan: {plan_kind})"),
        format!("This job: {job_num} of {total} — \"{}\"", job.step),
    ];

    let prior: Vec<String> = workflow
        .agent_jobs
        .iter()
        .filter(|j| j.job_index < job.job_index)
        .map(|j| {
            if let Some(s) = j.summary.as_ref() {
                format!(
                    "  Job {} ({}): {} — {}",
                    j.job_index + 1,
                    j.assigned_agent,
                    s.outcome_status,
                    s.summary_text
                )
            } else {
                format!(
                    "  Job {} ({}): {:?} — expected: {}",
                    j.job_index + 1,
                    j.assigned_agent,
                    j.status,
                    j.expected_outcome
                )
            }
        })
        .collect();
    if prior.is_empty() {
        lines.push("Prior jobs: none (first job in this plan)".to_string());
    } else {
        lines.push("Prior jobs:".to_string());
        lines.extend(prior);
    }

    lines.push(format!("Session gist: {gist}"));
    lines.push(format!("User message: {}", workflow.user_message));
    lines.push(format!(
        "Session attachments (use file_id values for file_ids tool arguments):\n{}",
        crate::format_session_attachments_for_prompt(&workflow.attachments)
    ));
    lines.push(format!("Delegation brief: {}", job.delegation_brief));
    lines.push(format!("Expected outcome: {}", job.expected_outcome));

    if let Some(snapshot) = workflow.active_workflow_snapshot.as_ref()
        && let Some(block) = crate::workflow_prompt::format_workflow_block(
            snapshot,
            crate::workflow_prompt::WorkflowBlockStyle::Slim,
        )
    {
        lines.push(String::new());
        lines.push(block);
    }

    if let Some(errors) = &workflow.pending_workflow_patch_errors
        && !errors.is_empty()
    {
        lines.push(String::new());
        lines.push("Workflow patch validation errors (fix and retry tool):".into());
        for e in errors {
            lines.push(format!("- {}: {}", e.field, e.message));
        }
    }

    lines.join("\n")
}

/// Context block for macro compose from completed agent job summaries (Phase 7.3).
pub fn format_agent_summaries_for_compose(workflow: &WorkflowState) -> String {
    let gist = workflow
        .session_gist()
        .unwrap_or_else(|| "(none)".to_string());
    let deliverable_fields = workflow
        .orchestration_contract
        .as_ref()
        .filter(|c| c.is_deliverable_codegen() || !c.deliverable_fields.is_empty())
        .map(|c| {
            if c.deliverable_fields.is_empty() {
                vec!["apiCode".to_string(), "fallback".to_string()]
            } else {
                c.deliverable_fields.clone()
            }
        });
    let mut lines = vec![
        "Agent job summaries (use these facts to answer the user):".to_string(),
        format!("Session gist: {gist}"),
        format!("User question: {}", workflow.user_message),
        String::new(),
    ];
    for job in &workflow.agent_jobs {
        lines.push(format!(
            "## Job {} — {} ({:?})",
            job.job_index + 1,
            job.assigned_agent,
            job.status
        ));
        lines.push(format!("Expected outcome: {}", job.expected_outcome));
        if let Some(s) = &job.summary {
            lines.push(format!("Outcome status: {}", s.outcome_status));
            lines.push(format!("Summary: {}", s.summary_text));
            if !s.tools_triggered.is_empty() {
                lines.push(format!("Tools used: {}", s.tools_triggered.join(", ")));
            }
        } else if job.status == AgentJobStatus::Failed {
            lines.push(format!(
                "Failed: {}",
                job.last_failure_reason.as_deref().unwrap_or("unknown")
            ));
        } else {
            lines.push("(no summary recorded)".to_string());
        }
        if let Some(fields) = deliverable_fields.as_ref()
            && let Some(result) = last_successful_tool_result(job)
            && let Some(block) = format_deliverable_fields(result, fields)
        {
            lines.push(block);
        }
        lines.push(String::new());
    }
    lines.join("\n")
}

fn last_successful_tool_result(job: &AgentJob) -> Option<&Value> {
    job.tool_attempts
        .iter()
        .rev()
        .find(|a| {
            a.action == "execute_tool"
                && a.status == "completed"
                && a.result.as_ref().is_some_and(|r| !r.is_null())
        })
        .and_then(|a| a.result.as_ref())
}

fn format_deliverable_fields(result: &Value, fields: &[String]) -> Option<String> {
    let payload = extract_tool_result_payload(result);
    let mut parts = Vec::new();
    for field in fields {
        let value = payload.get(field).or_else(|| result.get(field));
        let Some(value) = value else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        if let Some(s) = value.as_str() {
            if s.trim().is_empty() {
                continue;
            }
            parts.push(format!("{field}:\n{s}"));
        } else {
            parts.push(format!(
                "{field}:\n{}",
                truncate_json_for_prompt(value, 24_000)
            ));
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(format!("Tool deliverables:\n{}", parts.join("\n\n")))
    }
}

/// Full macro compose system context for v4 (contract + summaries + optional scope).
pub fn format_macro_compose_context(workflow: &WorkflowState) -> String {
    let mut sections = Vec::new();
    if let Some(contract) = &workflow.orchestration_contract {
        if contract.has_compose_guidance() {
            sections.push(format!(
                "Compose guidance (Orchestration Contract):\n{}",
                contract.compose.trim()
            ));
        }
        if !contract.outcome_profile.is_empty() {
            sections.push(format!(
                "Outcome profile: {}",
                contract.outcome_profile.trim()
            ));
        }
    }
    sections.push(format_agent_summaries_for_compose(workflow));
    if !workflow.assistant_system_context.is_empty() {
        let scope = assistant_scope_excerpt(&workflow.assistant_system_context, 2_000);
        if !scope.is_empty() {
            sections.push(format!(
                "Assistant scope and tone (do not override tool deliverables):\n{scope}"
            ));
        }
    }
    sections.join("\n\n")
}

fn assistant_scope_excerpt(system_context: &str, max_chars: usize) -> String {
    let trimmed = system_context.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let contract_marker = "## orchestration contract";
    let lower = trimmed.to_lowercase();
    let end = lower.find(contract_marker).unwrap_or(trimmed.len());
    let excerpt = trimmed[..end].trim();
    if excerpt.len() <= max_chars {
        excerpt.to_string()
    } else {
        format!("{}...", &excerpt[..max_chars])
    }
}

/// Context for macro validator LLM (Phase 7.4).
pub fn build_macro_validation_context(workflow: &WorkflowState) -> String {
    build_macro_validation_context_with_completeness(workflow, None)
}

/// Macro validator context with optional live completeness block (Phase 9.6).
pub fn build_macro_validation_context_with_completeness(
    workflow: &WorkflowState,
    completeness_block: Option<&str>,
) -> String {
    let gist = workflow
        .session_gist()
        .unwrap_or_else(|| "(none)".to_string());
    let mut lines = vec![
        format!("User question: {}", workflow.user_message),
        format!("Session gist: {gist}"),
    ];

    if let Some(block) = completeness_block {
        lines.push(String::new());
        lines.push(block.to_string());
    }

    if let Some(snapshot) = workflow.active_workflow_snapshot.as_ref() {
        if let Some(block) = crate::workflow_prompt::format_workflow_block_full(snapshot) {
            lines.push(String::new());
            lines.push(block);
        } else if let Some(summary) = crate::workflow_prompt::workflow_validator_summary(snapshot) {
            lines.push(String::new());
            lines.push(summary);
        }
    }

    lines.push(format!(
        "Replan count: {}/{}",
        workflow.guards.replan_count, workflow.guards.max_replans
    ));
    lines.push(String::new());
    lines.push("Agent job results:".to_string());
    for job in &workflow.agent_jobs {
        lines.push(format!(
            "- Job {} ({:?}) agent={} expected=\"{}\"",
            job.job_index + 1,
            job.status,
            job.assigned_agent,
            job.expected_outcome
        ));
        if let Some(s) = &job.summary {
            lines.push(format!(
                "  summary [{}]: {}",
                s.outcome_status, s.summary_text
            ));
        } else if let Some(reason) = &job.last_failure_reason {
            lines.push(format!("  failed: {reason}"));
        }
    }
    if !workflow.macro_escalations.is_empty() {
        lines.push(String::new());
        lines.push("Escalations:".to_string());
        for e in &workflow.macro_escalations {
            lines.push(format!(
                "- job {} reason={} outcome=\"{}\"",
                e.job_index, e.reason, e.expected_outcome
            ));
            if let Some(s) = &e.summary_text {
                lines.push(format!("  {s}"));
            }
        }
    }
    if !workflow.orchestration_ledger.is_empty() {
        lines.push(String::new());
        lines.push(format!(
            "Orchestration ledger turns: {}",
            workflow.orchestration_ledger.len()
        ));
    }
    if let Some(contract) = &workflow.orchestration_contract {
        lines.push(String::new());
        lines.push("Orchestration Contract (honor when validating):".to_string());
        if !contract.outcome_profile.is_empty() {
            lines.push(format!("Outcome profile: {}", contract.outcome_profile));
        }
        if contract.has_validator_guidance() {
            lines.push(format!(
                "Validator guidance:\n{}",
                contract.validator.trim()
            ));
        }
        if !contract.orchestrator.is_empty() {
            lines.push(format!(
                "Orchestrator framing (for context):\n{}",
                contract.orchestrator.trim()
            ));
        }
    }
    lines.join("\n")
}

/// Context for replan orchestrator LLM (Phase 7.4).
pub fn build_macro_replan_context(
    workflow: &WorkflowState,
    validation: &MacroValidationEntry,
) -> String {
    let mut lines = vec![
        build_macro_validation_context(workflow),
        String::new(),
        format!("Validator gaps: {}", validation.gaps),
        format!("Validator reasoning: {}", validation.reasoning),
    ];
    if let Some(r) = &validation.replan_rationale {
        lines.push(format!("Replan rationale: {r}"));
    }
    lines.join("\n")
}

/// Compose context when macro validator chooses fail_user (Phase 7.4).
pub fn format_macro_failure_for_compose(workflow: &WorkflowState) -> String {
    let summaries = format_agent_summaries_for_compose(workflow);
    let validation = workflow
        .last_validation
        .as_ref()
        .map(|v| {
            let mut parts = vec![format!(
                "Validator decision: {}\nReasoning: {}\nGaps: {}",
                v.decision, v.reasoning, v.gaps
            )];
            if let Some(missing) = structured_missing_required(workflow, v) {
                parts.push(format!("Missing required fields: {missing}"));
            }
            if let Some(pct) = v.completeness_pct {
                parts.push(format!("Completeness: {pct}%"));
            }
            parts.join("\n")
        })
        .unwrap_or_else(|| "Validation failed.".to_string());

    let workflow_incomplete = workflow
        .last_validation
        .as_ref()
        .and_then(|v| structured_missing_required(workflow, v))
        .is_some();

    let workflow_note = if workflow_incomplete {
        "Workflow incomplete — cannot submit.\n\n"
    } else {
        ""
    };

    format!(
        "The assistant could not fully complete the user's request.\n\
         Explain honestly what was tried and what happened. Do NOT invent success.\n\n\
         {workflow_note}{validation}\n\n{summaries}"
    )
}

fn structured_missing_required(
    workflow: &WorkflowState,
    validation: &MacroValidationEntry,
) -> Option<String> {
    if let Some(list) = validation
        .missing_required
        .as_ref()
        .filter(|l| !l.is_empty())
    {
        return Some(list.join(", "));
    }
    workflow.active_workflow_snapshot.as_ref().and_then(|snap| {
        snap.get("missingRequired")
            .or_else(|| snap.get("missing_required"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .filter(|s| !s.is_empty())
    })
}

/// Whether v4 compose should use failure framing.
pub fn macro_compose_is_failure(workflow: &WorkflowState) -> bool {
    workflow
        .last_validation
        .as_ref()
        .is_some_and(|v| v.decision == "fail_user")
}

/// Build delegation brief when assistant planner omits it.
pub fn assemble_delegation_brief(
    session_gist: Option<&str>,
    user_message: &str,
    description: &str,
    expected_outcome: &str,
    llm_brief: &str,
) -> String {
    if !llm_brief.trim().is_empty() {
        return llm_brief.trim().to_string();
    }
    let gist = session_gist.unwrap_or("").trim();
    format!(
        "Session: {gist}\nUser ask: {user_message}\nYour task: {description}\nSuccess means: {expected_outcome}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentJobStatus, PlanKind, WorkflowFieldError, WorkflowPhase, WorkflowState};
    use serde_json::json;

    #[test]
    fn format_agent_summaries_includes_deliverable_fields_for_codegen_profile() {
        use crate::OrchestrationContract;
        let mut w = WorkflowState::new("s1".into(), "c1".into(), "generate plsql".into());
        w.orchestration_contract = Some(OrchestrationContract {
            outcome_profile: "deliverable_codegen".into(),
            deliverable_fields: vec!["apiCode".into()],
            ..OrchestrationContract::default()
        });
        w.agent_jobs = vec![AgentJob {
            job_index: 0,
            step: "s".into(),
            description: "d".into(),
            assigned_agent: "Interface Agent".into(),
            assigned_agent_slug: Some("interface-agent".into()),
            expected_outcome: "code".into(),
            reason: "r".into(),
            delegation_brief: String::new(),
            status: AgentJobStatus::Completed,
            tool_attempts: vec![AgentPlannerAttempt {
                turn_index: 1,
                action: "execute_tool".into(),
                tool_id: Some("interface-agent__InterfaceCodeGenerator".into()),
                input_arguments: None,
                result: Some(json!({
                    "status": "completed",
                    "apiCode": "CREATE OR REPLACE PACKAGE x AS END x;"
                })),
                status: "completed".into(),
                reasoning: None,
                overlay_step_index: None,
            }],
            agent_planner_turn_count: 1,
            pending_arg_failure_retry: false,
            last_failure_reason: None,
            summary: Some(AgentJobSummary {
                outcome_status: "met".into(),
                summary_text: "User request fulfilled.".into(),
                tools_triggered: vec!["interface-agent__InterfaceCodeGenerator".into()],
            }),
        }];
        let block = format_agent_summaries_for_compose(&w);
        assert!(block.contains("Tool deliverables:"));
        assert!(block.contains("CREATE OR REPLACE PACKAGE"));
    }

    #[test]
    fn agent_job_llm_row_accepts_snake_case_keys() {
        let row: AgentJobLlmRow = serde_json::from_value(json!({
            "step": "s",
            "description": "d",
            "assigned_agent": "Jira Agent",
            "expected_outcome": "issue fetched",
            "reason": "r",
            "delegation_brief": "brief"
        }))
        .unwrap();
        assert_eq!(row.assigned_agent, "Jira Agent");
        assert_eq!(row.expected_outcome, "issue fetched");
    }

    #[test]
    fn format_tool_attempts_for_prompt_includes_successful_tool_result() {
        let job = AgentJob {
            job_index: 0,
            step: "s".into(),
            description: "d".into(),
            assigned_agent: "People".into(),
            assigned_agent_slug: Some("people".into()),
            expected_outcome: "find assignee".into(),
            reason: "r".into(),
            delegation_brief: String::new(),
            status: AgentJobStatus::Executing,
            tool_attempts: vec![AgentPlannerAttempt {
                turn_index: 1,
                action: "execute_tool".into(),
                tool_id: Some("people__lookup".into()),
                input_arguments: Some(json!({ "name": "Alice" })),
                result: Some(json!({
                    "status": "completed",
                    "people": [{ "name": "Alice", "email": "alice@example.com" }]
                })),
                status: "completed".into(),
                reasoning: None,
                overlay_step_index: None,
            }],
            agent_planner_turn_count: 1,
            pending_arg_failure_retry: false,
            last_failure_reason: None,
            summary: None,
        };
        let block = format_tool_attempts_for_prompt(&job);
        assert!(block.contains("tool result:"));
        assert!(block.contains("alice@example.com"));
    }

    #[test]
    fn format_tool_result_for_prompt_unwraps_people_envelope() {
        let result = json!({ "people": [{ "id": 1 }] });
        let formatted = format_tool_result_for_prompt(&result);
        assert!(formatted.contains("\"id\":1"));
        assert!(!formatted.contains("people"));
    }

    #[test]
    fn format_tool_attempts_for_prompt_includes_validation_errors() {
        let job = AgentJob {
            job_index: 0,
            step: "s".into(),
            description: "d".into(),
            assigned_agent: "Jira".into(),
            assigned_agent_slug: Some("jira".into()),
            expected_outcome: "done".into(),
            reason: "r".into(),
            delegation_brief: String::new(),
            status: AgentJobStatus::Executing,
            tool_attempts: vec![AgentPlannerAttempt {
                turn_index: 1,
                action: "execute_tool".into(),
                tool_id: Some("jira__fetch".into()),
                input_arguments: Some(json!({})),
                result: None,
                status: "arguments_invalid".into(),
                reasoning: Some("missing required argument 'issueKey'".into()),
                overlay_step_index: None,
            }],
            agent_planner_turn_count: 1,
            pending_arg_failure_retry: false,
            last_failure_reason: None,
            summary: None,
        };
        let block = format_tool_attempts_for_prompt(&job);
        assert!(block.contains("issueKey"));
        assert!(block.contains("arguments_invalid"));
    }

    #[test]
    fn format_tool_attempts_overlay_step_index() {
        let job = AgentJob {
            job_index: 0,
            step: "s".into(),
            description: "d".into(),
            assigned_agent: "Agent".into(),
            assigned_agent_slug: Some("agent".into()),
            expected_outcome: "done".into(),
            reason: "r".into(),
            delegation_brief: String::new(),
            status: AgentJobStatus::Executing,
            tool_attempts: vec![AgentPlannerAttempt {
                turn_index: 1,
                action: "execute_plan".into(),
                tool_id: Some("agent__Search".into()),
                input_arguments: None,
                result: Some(json!({"hits": []})),
                status: "completed".into(),
                reasoning: None,
                overlay_step_index: Some(0),
            }],
            agent_planner_turn_count: 1,
            pending_arg_failure_retry: false,
            last_failure_reason: None,
            summary: None,
        };
        let block = format_tool_attempts_for_prompt(&job);
        assert!(block.contains("overlay step: 0 of micro-plan"));
    }

    #[test]
    fn format_tool_attempts_hitl_halt_hint() {
        let job = AgentJob {
            job_index: 0,
            step: "s".into(),
            description: "d".into(),
            assigned_agent: "Agent".into(),
            assigned_agent_slug: Some("agent".into()),
            expected_outcome: "done".into(),
            reason: "r".into(),
            delegation_brief: String::new(),
            status: AgentJobStatus::Executing,
            tool_attempts: vec![AgentPlannerAttempt {
                turn_index: 1,
                action: "execute_plan".into(),
                tool_id: Some("agent__HITL".into()),
                input_arguments: None,
                result: Some(json!({"config": {"type": "AdaptiveCard"}, "cardType": "form"})),
                status: "completed".into(),
                reasoning: None,
                overlay_step_index: Some(1),
            }],
            agent_planner_turn_count: 1,
            pending_arg_failure_retry: false,
            last_failure_reason: None,
            summary: None,
        };
        let block = format_tool_attempts_for_prompt(&job);
        assert!(block.contains("HITL halted micro-plan"));
        assert!(block.contains("Recovery hint"));
    }

    #[test]
    fn format_tool_attempts_revise_plan_nudge() {
        let job = AgentJob {
            job_index: 0,
            step: "s".into(),
            description: "d".into(),
            assigned_agent: "Agent".into(),
            assigned_agent_slug: Some("agent".into()),
            expected_outcome: "done".into(),
            reason: "r".into(),
            delegation_brief: String::new(),
            status: AgentJobStatus::Executing,
            tool_attempts: vec![AgentPlannerAttempt {
                turn_index: 2,
                action: "execute_plan".into(),
                tool_id: Some("agent__Save".into()),
                input_arguments: Some(json!({})),
                result: None,
                status: "arguments_invalid".into(),
                reasoning: Some("missing name".into()),
                overlay_step_index: Some(2),
            }],
            agent_planner_turn_count: 2,
            pending_arg_failure_retry: true,
            last_failure_reason: None,
            summary: None,
        };
        let block = format_tool_attempts_for_prompt(&job);
        assert!(block.contains("consider revise_plan with remaining tasks from step 2"));
    }

    #[test]
    fn argument_resolution_exhausted_after_repeated_failures() {
        let job = AgentJob {
            job_index: 0,
            step: "s".into(),
            description: "d".into(),
            assigned_agent: "Jira".into(),
            assigned_agent_slug: Some("jira".into()),
            expected_outcome: "done".into(),
            reason: "r".into(),
            delegation_brief: String::new(),
            status: AgentJobStatus::Executing,
            tool_attempts: vec![
                AgentPlannerAttempt {
                    turn_index: 1,
                    action: "execute_tool".into(),
                    tool_id: Some("jira__fetch".into()),
                    input_arguments: Some(json!({})),
                    result: None,
                    status: "arguments_invalid".into(),
                    reasoning: Some("missing".into()),
                    overlay_step_index: None,
                },
                AgentPlannerAttempt {
                    turn_index: 2,
                    action: "execute_tool".into(),
                    tool_id: Some("jira__fetch".into()),
                    input_arguments: Some(json!({})),
                    result: None,
                    status: "arguments_invalid".into(),
                    reasoning: Some("missing".into()),
                    overlay_step_index: None,
                },
            ],
            agent_planner_turn_count: 2,
            pending_arg_failure_retry: false,
            last_failure_reason: None,
            summary: None,
        };
        assert!(argument_resolution_exhausted(&job, "jira__fetch", 2));
    }

    #[test]
    fn build_agent_handoff_context_includes_job_position_and_prior_summary() {
        let mut workflow = WorkflowState::new("s".into(), "c".into(), "get assignee".into());
        workflow.plan_kind = PlanKind::AgentJobV4;
        workflow.phase = WorkflowPhase::Executing;
        workflow.agent_jobs = vec![
            AgentJob {
                job_index: 0,
                step: "fetch".into(),
                description: "fetch issue".into(),
                assigned_agent: "Jira".into(),
                assigned_agent_slug: Some("jira".into()),
                expected_outcome: "issue fetched".into(),
                reason: "r".into(),
                delegation_brief: "brief1".into(),
                status: AgentJobStatus::Completed,
                tool_attempts: vec![],
                agent_planner_turn_count: 1,
                pending_arg_failure_retry: false,
                last_failure_reason: None,
                summary: Some(AgentJobSummary {
                    outcome_status: "met".into(),
                    summary_text: "Assignee: Jane Doe".into(),
                    tools_triggered: vec!["jira__fetch".into()],
                }),
            },
            AgentJob {
                job_index: 1,
                step: "report".into(),
                description: "report back".into(),
                assigned_agent: "Jira".into(),
                assigned_agent_slug: Some("jira".into()),
                expected_outcome: "user informed".into(),
                reason: "r".into(),
                delegation_brief: "brief2".into(),
                status: AgentJobStatus::Delegated,
                tool_attempts: vec![],
                agent_planner_turn_count: 0,
                pending_arg_failure_retry: false,
                last_failure_reason: None,
                summary: None,
            },
        ];
        let ctx = build_agent_handoff_context(&workflow, &workflow.agent_jobs[1]);
        assert!(ctx.contains("2 of 2"));
        assert!(ctx.contains("Jane Doe"));
        assert!(ctx.contains("brief2"));
    }

    fn workflow_snapshot_fixture() -> serde_json::Value {
        serde_json::from_str(include_str!(
            "../tests/fixtures/workflow_snapshot_collect.json"
        ))
        .unwrap()
    }

    #[test]
    fn build_agent_handoff_context_includes_slim_workflow_block() {
        let mut workflow = WorkflowState::new("s".into(), "c".into(), "start PR".into());
        workflow.active_workflow_snapshot = Some(workflow_snapshot_fixture());
        workflow.agent_jobs.push(AgentJob {
            job_index: 0,
            step: "collect".into(),
            description: "d".into(),
            assigned_agent: "Agent".into(),
            assigned_agent_slug: None,
            expected_outcome: "job outcome".into(),
            reason: "r".into(),
            delegation_brief: "brief".into(),
            status: AgentJobStatus::Delegated,
            tool_attempts: vec![],
            agent_planner_turn_count: 0,
            pending_arg_failure_retry: false,
            last_failure_reason: None,
            summary: None,
        });
        let ctx = build_agent_handoff_context(&workflow, &workflow.agent_jobs[0]);
        assert!(ctx.contains("Business workflow: purchase_requisition"));
        assert!(ctx.contains("collect_line_items"));
        assert!(ctx.contains("Missing required: lineItems, costCenter"));
        assert!(ctx.contains("Stage expected outcome:"));
        assert!(ctx.contains("Expected outcome: job outcome"));
    }

    #[test]
    fn build_agent_handoff_context_includes_workflow_patch_validation_errors() {
        let mut workflow = WorkflowState::new("s".into(), "c".into(), "save vendor".into());
        workflow.pending_workflow_patch_errors = Some(vec![
            WorkflowFieldError {
                field: "vendorId".into(),
                message: "unknown field key for workflow definition".into(),
            },
            WorkflowFieldError {
                field: "costCenter".into(),
                message: "required".into(),
            },
        ]);
        workflow.agent_jobs.push(AgentJob {
            job_index: 0,
            step: "save".into(),
            description: "d".into(),
            assigned_agent: "Agent".into(),
            assigned_agent_slug: None,
            expected_outcome: "saved".into(),
            reason: "r".into(),
            delegation_brief: "brief".into(),
            status: AgentJobStatus::Executing,
            tool_attempts: vec![],
            agent_planner_turn_count: 0,
            pending_arg_failure_retry: false,
            last_failure_reason: None,
            summary: None,
        });
        let ctx = build_agent_handoff_context(&workflow, &workflow.agent_jobs[0]);
        assert!(ctx.contains("Workflow patch validation errors"));
        assert!(ctx.contains("vendorId"));
        assert!(ctx.contains("costCenter"));
    }

    #[test]
    fn build_macro_validation_context_includes_full_workflow_block() {
        let mut workflow = WorkflowState::new("s".into(), "c".into(), "start PR".into());
        workflow.active_workflow_snapshot = Some(workflow_snapshot_fixture());
        let ctx = build_macro_validation_context(&workflow);
        assert!(ctx.contains("=== ACTIVE WORKFLOW ==="));
        assert!(ctx.contains("missingRequired"));
        assert!(ctx.contains("completeness: 40%"));
    }

    #[test]
    fn tools_triggered_from_attempts_dedupes() {
        let attempts = vec![
            AgentPlannerAttempt {
                turn_index: 0,
                action: "execute_tool".into(),
                tool_id: Some("a__x".into()),
                input_arguments: None,
                result: None,
                status: "completed".into(),
                reasoning: None,
                overlay_step_index: None,
            },
            AgentPlannerAttempt {
                turn_index: 1,
                action: "execute_tool".into(),
                tool_id: Some("a__x".into()),
                input_arguments: None,
                result: None,
                status: "completed".into(),
                reasoning: None,
                overlay_step_index: None,
            },
        ];
        assert_eq!(tools_triggered_from_attempts(&attempts), vec!["a__x"]);
    }

    #[test]
    fn format_agent_summaries_for_compose_includes_summary_text() {
        let mut workflow = WorkflowState::new("s".into(), "c".into(), "who is assignee?".into());
        workflow.agent_jobs.push(AgentJob {
            job_index: 0,
            step: "s".into(),
            description: "d".into(),
            assigned_agent: "Jira".into(),
            assigned_agent_slug: Some("jira".into()),
            expected_outcome: "assignee returned".into(),
            reason: "r".into(),
            delegation_brief: String::new(),
            status: AgentJobStatus::Completed,
            tool_attempts: vec![],
            agent_planner_turn_count: 1,
            pending_arg_failure_retry: false,
            last_failure_reason: None,
            summary: Some(AgentJobSummary {
                outcome_status: "met".into(),
                summary_text: "Assignee: Bob".into(),
                tools_triggered: vec![],
            }),
        });
        let block = format_agent_summaries_for_compose(&workflow);
        assert!(block.contains("Assignee: Bob"));
        assert!(block.contains("who is assignee?"));
    }

    #[test]
    fn format_macro_failure_for_compose_includes_gaps() {
        let mut workflow = WorkflowState::new("s".into(), "c".into(), "get report".into());
        workflow.last_validation = Some(MacroValidationEntry {
            decision: "fail_user".into(),
            reasoning: "could not fetch".into(),
            gaps: "missing Q3 data".into(),
            replan_rationale: None,
            correlation_id: "c".into(),
            missing_required: None,
            completeness_pct: None,
        });
        let block = format_macro_failure_for_compose(&workflow);
        assert!(block.contains("missing Q3 data"));
        assert!(block.contains("Do NOT invent success"));
    }
}
