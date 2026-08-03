use crate::output::ErrorBody;
use crate::push_plan::{PushAction, PushPlan, PushStep, ResourceType};
use crate::workspace::Workspace;
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Success,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct StepResult {
    pub order: u32,
    pub resource_type: ResourceType,
    pub name: String,
    pub action: PushAction,
    pub status: StepStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RollbackEntry {
    pub order: u32,
    pub resource_type: ResourceType,
    pub name: String,
    pub action: PushAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub local_path: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PushRunReport {
    pub dry_run: bool,
    pub steps: Vec<StepResult>,
    pub completed_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_at: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rollback_journal: Vec<RollbackEntry>,
}

#[derive(Debug, Clone)]
pub struct PushExecuteOptions {
    pub force: bool,
    pub stop_on_error: bool,
}

#[derive(Debug)]
pub enum PushExecutorError {
    PreflightBlocked(Vec<String>),
}

impl std::fmt::Display for PushExecutorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PushExecutorError::PreflightBlocked(blockers) => {
                write!(f, "Push blocked by validation: {}", blockers.join(", "))
            }
        }
    }
}

impl std::error::Error for PushExecutorError {}

pub fn preflight_blockers(plan: &PushPlan, force: bool) -> Vec<String> {
    if force {
        return Vec::new();
    }
    let mut blockers = Vec::new();
    for step in &plan.steps {
        for code in &step.blockers {
            blockers.push(format!("{}:{}", step.name, code));
        }
    }
    blockers.sort();
    blockers.dedup();
    blockers
}

pub async fn execute_push_plan(
    ws: &Workspace,
    plan: &PushPlan,
    opts: PushExecuteOptions,
) -> Result<PushRunReport, PushExecutorError> {
    let blockers = preflight_blockers(plan, opts.force);
    if !blockers.is_empty() {
        return Err(PushExecutorError::PreflightBlocked(blockers));
    }

    let mut results = Vec::new();
    let mut journal = Vec::new();
    let mut completed_count = 0u32;
    let mut failed_at = None;

    for step in &plan.steps {
        if step.action == PushAction::Skip {
            results.push(skipped_result(step));
            continue;
        }
        if !step.blockers.is_empty() && !opts.force {
            results.push(skipped_result(step));
            continue;
        }

        match push_one(ws, step).await {
            Ok(id) => {
                journal.push(RollbackEntry {
                    order: step.order,
                    resource_type: step.resource_type,
                    name: step.name.clone(),
                    action: step.action,
                    id: id.clone(),
                    local_path: step.path.clone(),
                });
                results.push(StepResult {
                    order: step.order,
                    resource_type: step.resource_type,
                    name: step.name.clone(),
                    action: step.action,
                    status: StepStatus::Success,
                    id,
                    error: None,
                });
                completed_count += 1;
            }
            Err(message) => {
                results.push(StepResult {
                    order: step.order,
                    resource_type: step.resource_type,
                    name: step.name.clone(),
                    action: step.action,
                    status: StepStatus::Failed,
                    id: None,
                    error: Some(ErrorBody {
                        code: "PUSH_STEP_FAILED".to_string(),
                        message: message.clone(),
                        details: None,
                    }),
                });
                failed_at = Some(step.order);
                if opts.stop_on_error {
                    break;
                }
            }
        }
    }

    Ok(PushRunReport {
        dry_run: false,
        steps: results,
        completed_count,
        failed_at,
        rollback_journal: journal,
    })
}

fn skipped_result(step: &PushStep) -> StepResult {
    StepResult {
        order: step.order,
        resource_type: step.resource_type,
        name: step.name.clone(),
        action: step.action,
        status: StepStatus::Skipped,
        id: step.id.clone(),
        error: None,
    }
}

async fn push_one(ws: &Workspace, step: &PushStep) -> Result<Option<String>, String> {
    match step.resource_type {
        ResourceType::Hitl => push_hitl(ws, step).await,
        ResourceType::Workflow => push_workflow(ws, step).await,
        ResourceType::Tool => push_tool(ws, step).await,
        ResourceType::Agent => push_agent(ws, step).await,
        ResourceType::Assistant => push_assistant(ws, step).await,
    }
}

async fn push_hitl(ws: &Workspace, step: &PushStep) -> Result<Option<String>, String> {
    use crate::api::{create_hitl, update_hitl};
    use crate::specs::{load_hitl_from_dir, write_hitl_id_to_meta};

    let hitl_dir = resolve_hitl_dir(ws, step)?;
    let (mut body, meta_path) = load_hitl_from_dir(&hitl_dir).map_err(|e| e.to_string())?;
    let has_id = body
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    let id = if has_id {
        let record_id = body.get("id").and_then(|v| v.as_str()).unwrap().to_string();
        body.as_object_mut().and_then(|o| o.remove("id"));
        update_hitl(&record_id, &body)
            .await
            .map_err(|e| e.to_string())?
    } else {
        body.as_object_mut().and_then(|o| o.remove("id"));
        let id = create_hitl(&body).await.map_err(|e| e.to_string())?;
        write_hitl_id_to_meta(&meta_path, &id).map_err(|e| e.to_string())?;
        id
    };
    Ok(Some(id))
}

async fn push_workflow(ws: &Workspace, step: &PushStep) -> Result<Option<String>, String> {
    use crate::api::{create_workflow_definition, PushResult};
    use crate::specs::{increment_workflow_version, load_workflow_from_dir, write_workflow_id};

    let workflow_dir = Path::new(&step.path)
        .parent()
        .ok_or_else(|| format!("Invalid workflow path: {}", step.path))?;
    let max_retries = 10;
    let mut retry_count = 0;
    loop {
        let (mut bundle, version_path, format) =
            load_workflow_from_dir(workflow_dir).map_err(|e| e.to_string())?;
        if let Some(meta) = bundle.get_mut("meta").and_then(|m| m.as_object_mut()) {
            meta.remove("id");
            meta.remove("_id");
        }
        let payload = serde_json::json!({
            "authorBundle": bundle,
            "sourceFormat": format.as_str(),
        });
        match create_workflow_definition(&payload)
            .await
            .map_err(|e| e.to_string())?
        {
            PushResult::Success(id) => {
                if !id.is_empty() {
                    write_workflow_id(&version_path, format, &id).map_err(|e| e.to_string())?;
                }
                return Ok(if id.is_empty() { None } else { Some(id) });
            }
            PushResult::Conflict => {
                retry_count += 1;
                if retry_count >= max_retries {
                    return Err(format!(
                        "Workflow conflict persisted after {} retries",
                        max_retries
                    ));
                }
                let next_version =
                    increment_workflow_version(&version_path, format).map_err(|e| e.to_string())?;
                let _ = next_version;
            }
        }
    }
}

async fn push_tool(ws: &Workspace, step: &PushStep) -> Result<Option<String>, String> {
    use crate::api::{create_tool, update_tool};
    use crate::specs::{load_tool_from_dir, write_tool_id_to_yaml};

    let tool_dir = Path::new(&step.path)
        .parent()
        .ok_or_else(|| format!("Invalid tool path: {}", step.path))?;
    let (mut body, yaml_path) = load_tool_from_dir(tool_dir).map_err(|e| e.to_string())?;
    let has_id = body
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    let id = if has_id {
        update_tool(&body).await.map_err(|e| e.to_string())?
    } else {
        body.as_object_mut().and_then(|o| o.remove("id"));
        let id = create_tool(&body).await.map_err(|e| e.to_string())?;
        write_tool_id_to_yaml(&yaml_path, &id).map_err(|e| e.to_string())?;
        id
    };
    Ok(Some(id))
}

async fn push_agent(ws: &Workspace, step: &PushStep) -> Result<Option<String>, String> {
    use crate::api::{create_agent, update_agent};
    use crate::specs::{load_agent, write_agent_id_to_yaml};

    let path = Path::new(&step.path);
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("Invalid agent path: {}", step.path))?;
    let (mut body, yaml_path) = load_agent(&ws.agents_dir, name).map_err(|e| e.to_string())?;
    let has_id = body
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    let id = if has_id {
        let record_id = body.get("id").and_then(|v| v.as_str()).unwrap().to_string();
        body.as_object_mut().and_then(|o| o.remove("id"));
        update_agent(&record_id, &body)
            .await
            .map_err(|e| e.to_string())?
    } else {
        body.as_object_mut().and_then(|o| o.remove("id"));
        let id = create_agent(&body).await.map_err(|e| e.to_string())?;
        write_agent_id_to_yaml(&yaml_path, &id).map_err(|e| e.to_string())?;
        id
    };
    Ok(Some(id))
}

async fn push_assistant(ws: &Workspace, step: &PushStep) -> Result<Option<String>, String> {
    use crate::api::{create_assistant, update_assistant};
    use crate::specs::{load_assistant, write_assistant_id_to_yaml};

    let path = Path::new(&step.path);
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("Invalid assistant path: {}", step.path))?;
    let (mut body, yaml_path) =
        load_assistant(&ws.assistants_dir, name).map_err(|e| e.to_string())?;
    let has_id = body
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    let id = if has_id {
        let record_id = body.get("id").and_then(|v| v.as_str()).unwrap().to_string();
        body.as_object_mut().and_then(|o| o.remove("id"));
        update_assistant(&record_id, &body)
            .await
            .map_err(|e| e.to_string())?
    } else {
        body.as_object_mut().and_then(|o| o.remove("id"));
        let id = create_assistant(&body).await.map_err(|e| e.to_string())?;
        write_assistant_id_to_yaml(&yaml_path, &id).map_err(|e| e.to_string())?;
        id
    };
    Ok(Some(id))
}

fn resolve_hitl_dir(ws: &Workspace, step: &PushStep) -> Result<std::path::PathBuf, String> {
    let path = Path::new(&step.path);
    if path
        .parent()
        .map(|p| p.starts_with(&ws.hitl_dir))
        .unwrap_or(false)
    {
        return Ok(path.parent().unwrap().to_path_buf());
    }
    Ok(ws.hitl_dir.join(&step.name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::push_plan::PushPlan;

    #[test]
    fn preflight_collects_blockers() {
        let plan = PushPlan {
            steps: vec![PushStep {
                order: 1,
                resource_type: ResourceType::Agent,
                name: "a".to_string(),
                path: "agents/a.yaml".to_string(),
                action: PushAction::Create,
                has_id: false,
                id: None,
                blockers: vec!["BROKEN_TOOL_REF".to_string()],
                remote_drift: false,
                drift_fields: vec![],
            }],
            skipped: vec![],
        };
        let blockers = preflight_blockers(&plan, false);
        assert_eq!(blockers, vec!["a:BROKEN_TOOL_REF"]);
    }

    #[test]
    fn force_skips_preflight_blockers() {
        let plan = PushPlan {
            steps: vec![PushStep {
                order: 1,
                resource_type: ResourceType::Agent,
                name: "a".to_string(),
                path: "agents/a.yaml".to_string(),
                action: PushAction::Create,
                has_id: false,
                id: None,
                blockers: vec!["BROKEN_TOOL_REF".to_string()],
                remote_drift: false,
                drift_fields: vec![],
            }],
            skipped: vec![],
        };
        assert!(preflight_blockers(&plan, true).is_empty());
    }
}
