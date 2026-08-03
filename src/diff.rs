use crate::api::get_workflow_definition;
use crate::remote_cache::{RemoteCache, RemoteLookupStatus};
use crate::specs::normalize::normalize_mongo_oids;
use crate::specs::{
    load_agent, load_assistant, load_hitl_from_dir, load_tool_from_dir, load_workflow_from_dir,
};
use crate::workspace::Workspace;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffResourceType {
    Tool,
    Agent,
    Assistant,
    Hitl,
    Workflow,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DiffHunk {
    pub field: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote: Option<Value>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DiffReport {
    pub resource_type: DiffResourceType,
    pub name: String,
    pub has_changes: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hunks: Vec<DiffHunk>,
    pub remote_missing: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug)]
pub enum DiffError {
    NotFound(String),
    NoId(String),
    Remote(String),
    Io(std::io::Error),
}

impl std::fmt::Display for DiffError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiffError::NotFound(msg) => write!(f, "{msg}"),
            DiffError::NoId(msg) => write!(f, "{msg}"),
            DiffError::Remote(msg) => write!(f, "{msg}"),
            DiffError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for DiffError {}

pub fn parse_resource_type(raw: &str) -> Option<DiffResourceType> {
    match raw.to_ascii_lowercase().as_str() {
        "tool" | "tools" => Some(DiffResourceType::Tool),
        "agent" | "agents" => Some(DiffResourceType::Agent),
        "assistant" | "assistants" => Some(DiffResourceType::Assistant),
        "hitl" => Some(DiffResourceType::Hitl),
        "workflow" | "workflows" => Some(DiffResourceType::Workflow),
        _ => None,
    }
}

pub async fn diff_resource(
    ws: &Workspace,
    resource_type: DiffResourceType,
    name: &str,
) -> Result<DiffReport, DiffError> {
    match resource_type {
        DiffResourceType::Tool => diff_tool(ws, name).await,
        DiffResourceType::Agent => diff_agent(ws, name).await,
        DiffResourceType::Assistant => diff_assistant(ws, name).await,
        DiffResourceType::Hitl => diff_hitl(ws, name).await,
        DiffResourceType::Workflow => diff_workflow(ws, name).await,
    }
}

pub async fn remote_drift_fields(
    ws: &Workspace,
    resource_type: DiffResourceType,
    name: &str,
    cache: &mut RemoteCache,
) -> Result<Vec<String>, DiffError> {
    let report = diff_resource_with_cache(ws, resource_type, name, cache).await?;
    Ok(report.hunks.iter().map(|h| h.field.clone()).collect())
}

async fn diff_resource_with_cache(
    ws: &Workspace,
    resource_type: DiffResourceType,
    name: &str,
    cache: &mut RemoteCache,
) -> Result<DiffReport, DiffError> {
    match resource_type {
        DiffResourceType::Tool => diff_tool_cached(ws, name, cache).await,
        DiffResourceType::Agent => diff_agent_cached(ws, name, cache).await,
        DiffResourceType::Assistant => diff_assistant_cached(ws, name, cache).await,
        DiffResourceType::Hitl => diff_hitl_cached(ws, name, cache).await,
        DiffResourceType::Workflow => diff_workflow(ws, name).await,
    }
}

async fn diff_tool(ws: &Workspace, name: &str) -> Result<DiffReport, DiffError> {
    let mut cache = RemoteCache::new();
    diff_tool_cached(ws, name, &mut cache).await
}

async fn diff_tool_cached(
    ws: &Workspace,
    name: &str,
    cache: &mut RemoteCache,
) -> Result<DiffReport, DiffError> {
    let tool_dir = ws.tools_dir.join(name);
    if !tool_dir.is_dir() {
        return Err(DiffError::NotFound(format!("tool not found: {name}")));
    }
    let (local_body, _) =
        load_tool_from_dir(&tool_dir).map_err(|e| DiffError::NotFound(e.to_string()))?;
    let id = local_body
        .get("id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| DiffError::NoId(format!("tool `{name}` has no platform id")))?;

    let lookup = cache.get_record("skillConfig", id).await;
    if lookup.status == RemoteLookupStatus::NotFound {
        return Ok(DiffReport {
            resource_type: DiffResourceType::Tool,
            name: name.to_string(),
            has_changes: true,
            hunks: vec![],
            remote_missing: true,
            message: Some(format!("remote tool `{id}` not found")),
        });
    }
    if lookup.status == RemoteLookupStatus::Error {
        return Err(DiffError::Remote(
            lookup
                .message
                .unwrap_or_else(|| "remote lookup failed".into()),
        ));
    }

    let remote = lookup.data.unwrap_or(Value::Null);
    let local_cmp = tool_compare_view(&local_body);
    let remote_cmp = tool_compare_view(&remote);
    build_diff_report(DiffResourceType::Tool, name, &local_cmp, &remote_cmp)
}

async fn diff_agent(ws: &Workspace, name: &str) -> Result<DiffReport, DiffError> {
    let mut cache = RemoteCache::new();
    diff_agent_cached(ws, name, &mut cache).await
}

async fn diff_agent_cached(
    ws: &Workspace,
    name: &str,
    cache: &mut RemoteCache,
) -> Result<DiffReport, DiffError> {
    let (local_body, _) =
        load_agent(&ws.agents_dir, name).map_err(|e| DiffError::NotFound(e.to_string()))?;
    let id = local_body
        .get("id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| DiffError::NoId(format!("agent `{name}` has no platform id")))?;

    let lookup = cache.get_record("agentConfig", id).await;
    if lookup.status == RemoteLookupStatus::NotFound {
        return Ok(remote_missing_report(DiffResourceType::Agent, name, id));
    }
    if lookup.status == RemoteLookupStatus::Error {
        return Err(DiffError::Remote(
            lookup
                .message
                .unwrap_or_else(|| "remote lookup failed".into()),
        ));
    }
    let remote = lookup.data.unwrap_or(Value::Null);
    build_diff_report(
        DiffResourceType::Agent,
        name,
        &strip_volatile(&local_body),
        &strip_volatile(&remote),
    )
}

async fn diff_assistant(ws: &Workspace, name: &str) -> Result<DiffReport, DiffError> {
    let mut cache = RemoteCache::new();
    diff_assistant_cached(ws, name, &mut cache).await
}

async fn diff_assistant_cached(
    ws: &Workspace,
    name: &str,
    cache: &mut RemoteCache,
) -> Result<DiffReport, DiffError> {
    let (local_body, _) =
        load_assistant(&ws.assistants_dir, name).map_err(|e| DiffError::NotFound(e.to_string()))?;
    let id = local_body
        .get("id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| DiffError::NoId(format!("assistant `{name}` has no platform id")))?;

    let lookup = cache.get_record("chat", id).await;
    if lookup.status == RemoteLookupStatus::NotFound {
        return Ok(remote_missing_report(DiffResourceType::Assistant, name, id));
    }
    if lookup.status == RemoteLookupStatus::Error {
        return Err(DiffError::Remote(
            lookup
                .message
                .unwrap_or_else(|| "remote lookup failed".into()),
        ));
    }
    let remote = lookup.data.unwrap_or(Value::Null);
    build_diff_report(
        DiffResourceType::Assistant,
        name,
        &strip_volatile(&local_body),
        &strip_volatile(&remote),
    )
}

async fn diff_hitl(ws: &Workspace, name: &str) -> Result<DiffReport, DiffError> {
    let mut cache = RemoteCache::new();
    diff_hitl_cached(ws, name, &mut cache).await
}

async fn diff_hitl_cached(
    ws: &Workspace,
    name: &str,
    cache: &mut RemoteCache,
) -> Result<DiffReport, DiffError> {
    let hitl_dir = ws.hitl_dir.join(name);
    if !hitl_dir.is_dir() {
        return Err(DiffError::NotFound(format!("hitl not found: {name}")));
    }
    let (local_body, _) =
        load_hitl_from_dir(&hitl_dir).map_err(|e| DiffError::NotFound(e.to_string()))?;
    let id = local_body
        .get("id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| DiffError::NoId(format!("hitl `{name}` has no platform id")))?;

    let lookup = cache.get_record("hitlConfig", id).await;
    if lookup.status == RemoteLookupStatus::NotFound {
        return Ok(remote_missing_report(DiffResourceType::Hitl, name, id));
    }
    if lookup.status == RemoteLookupStatus::Error {
        return Err(DiffError::Remote(
            lookup
                .message
                .unwrap_or_else(|| "remote lookup failed".into()),
        ));
    }
    let remote = lookup.data.unwrap_or(Value::Null);
    build_diff_report(
        DiffResourceType::Hitl,
        name,
        &strip_volatile(&local_body),
        &strip_volatile(&remote),
    )
}

async fn diff_workflow(ws: &Workspace, name: &str) -> Result<DiffReport, DiffError> {
    let workflow_dir = ws.workflows_dir.join(name);
    if !workflow_dir.is_dir() {
        return Err(DiffError::NotFound(format!("workflow not found: {name}")));
    }
    let (local_bundle, _, _) =
        load_workflow_from_dir(&workflow_dir).map_err(|e| DiffError::NotFound(e.to_string()))?;
    let slug = local_bundle
        .get("meta")
        .and_then(|m| m.get("slug"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| DiffError::NotFound(format!("workflow `{name}` missing meta.slug")))?;
    let version = local_bundle
        .get("meta")
        .and_then(|m| m.get("version"))
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    let remote_body = get_workflow_definition(slug, version)
        .await
        .map_err(|e| DiffError::Remote(e.to_string()))?;
    let remote_data = remote_body
        .get("data")
        .cloned()
        .unwrap_or(remote_body.clone());
    let remote_normalized = normalize_mongo_oids(remote_data);

    let local_cmp = workflow_compare_view(&local_bundle);
    let remote_cmp = workflow_compare_view(&remote_normalized);
    build_diff_report(DiffResourceType::Workflow, name, &local_cmp, &remote_cmp)
}

fn remote_missing_report(resource_type: DiffResourceType, name: &str, id: &str) -> DiffReport {
    DiffReport {
        resource_type,
        name: name.to_string(),
        has_changes: true,
        hunks: vec![],
        remote_missing: true,
        message: Some(format!("remote record `{id}` not found")),
    }
}

fn tool_compare_view(body: &Value) -> Value {
    let mut view = strip_volatile(body);
    if let Some(obj) = view.as_object_mut() {
        obj.remove("code");
        obj.remove("handler");
        obj.remove("packageJson");
    }
    view
}

fn workflow_compare_view(bundle: &Value) -> Value {
    let mut view = bundle.clone();
    if let Some(meta) = view.get_mut("meta").and_then(|m| m.as_object_mut()) {
        meta.remove("id");
        meta.remove("_id");
        meta.remove("createdAt");
        meta.remove("updatedAt");
    }
    view
}

fn strip_volatile(value: &Value) -> Value {
    let mut out = normalize_mongo_oids(value.clone());
    if let Some(obj) = out.as_object_mut() {
        for key in [
            "createdAt",
            "updatedAt",
            "createdBy",
            "managedBy",
            "__v",
            "_id",
        ] {
            obj.remove(key);
        }
    }
    out
}

fn build_diff_report(
    resource_type: DiffResourceType,
    name: &str,
    local: &Value,
    remote: &Value,
) -> Result<DiffReport, DiffError> {
    let local_obj = local
        .as_object()
        .ok_or_else(|| DiffError::NotFound(format!("local `{name}` is not a JSON object")))?;
    let remote_obj = remote
        .as_object()
        .ok_or_else(|| DiffError::Remote(format!("remote `{name}` is not a JSON object")))?;

    let mut fields: Vec<String> = local_obj.keys().chain(remote_obj.keys()).cloned().collect();
    fields.sort();
    fields.dedup();

    let mut hunks = Vec::new();
    for field in fields {
        let local_val = local_obj.get(&field).cloned();
        let remote_val = remote_obj.get(&field).cloned();
        if local_val != remote_val {
            hunks.push(DiffHunk {
                field,
                local: local_val,
                remote: remote_val,
            });
        }
    }

    Ok(DiffReport {
        resource_type,
        name: name.to_string(),
        has_changes: !hunks.is_empty(),
        hunks,
        remote_missing: false,
        message: None,
    })
}

pub fn step_resource_type(kind: &crate::push_plan::ResourceType) -> Option<DiffResourceType> {
    use crate::push_plan::ResourceType;
    match kind {
        ResourceType::Tool => Some(DiffResourceType::Tool),
        ResourceType::Agent => Some(DiffResourceType::Agent),
        ResourceType::Assistant => Some(DiffResourceType::Assistant),
        ResourceType::Hitl => Some(DiffResourceType::Hitl),
        ResourceType::Workflow => Some(DiffResourceType::Workflow),
    }
}

pub async fn check_step_remote_drift(
    ws: &Workspace,
    resource_type: crate::push_plan::ResourceType,
    name: &str,
    has_id: bool,
    cache: &mut RemoteCache,
) -> (bool, Vec<String>) {
    if !has_id {
        return (false, Vec::new());
    }
    let Some(diff_type) = step_resource_type(&resource_type) else {
        return (false, Vec::new());
    };
    match remote_drift_fields(ws, diff_type, name, cache).await {
        Ok(fields) if !fields.is_empty() => (true, fields),
        Ok(_) => (false, Vec::new()),
        Err(_) => (false, Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_diff_detects_changed_field() {
        let local = serde_json::json!({ "name": "A", "version": "1" });
        let remote = serde_json::json!({ "name": "B", "version": "1" });
        let report = build_diff_report(DiffResourceType::Tool, "t", &local, &remote).unwrap();
        assert!(report.has_changes);
        assert_eq!(report.hunks.len(), 1);
        assert_eq!(report.hunks[0].field, "name");
    }
}
