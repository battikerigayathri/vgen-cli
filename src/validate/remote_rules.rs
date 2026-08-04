use crate::remote_cache::{RemoteCache, RemoteLookupStatus};
use crate::specs::{
    list_agent_files, list_assistant_files, list_hitl_dirs, list_tool_dirs, load_tool_yaml,
};
use crate::workspace::Workspace;

use super::{
    push_finding, read_yaml, relative_path, string_field, Finding, ResourceKind, ResourceRef,
    Severity,
};

pub async fn check_remote_rules(ws: &Workspace, cache: &mut RemoteCache) -> Vec<Finding> {
    let mut findings = Vec::new();
    check_tools(ws, cache, &mut findings).await;
    check_agents(ws, cache, &mut findings).await;
    check_assistants(ws, cache, &mut findings).await;
    check_hitl(ws, cache, &mut findings).await;
    findings
}

async fn check_tools(ws: &Workspace, cache: &mut RemoteCache, findings: &mut Vec<Finding>) {
    for tool_dir in list_tool_dirs(&ws.tools_dir) {
        let Ok((value, yaml_path)) = load_tool_yaml(&tool_dir) else {
            continue;
        };
        let Some(id) = string_field(&value, "id") else {
            continue;
        };
        let slug = tool_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        verify_id(
            cache,
            findings,
            "skillConfig",
            &id,
            &relative_path(&yaml_path, &ws.root),
            ResourceRef {
                kind: ResourceKind::Tool,
                slug: Some(slug),
                id: Some(id.clone()),
            },
        )
        .await;
    }
}

async fn check_agents(ws: &Workspace, cache: &mut RemoteCache, findings: &mut Vec<Finding>) {
    for (stem, path) in list_agent_files(&ws.agents_dir) {
        let Ok(value) = read_yaml(&path) else {
            continue;
        };
        let Some(id) = string_field(&value, "id") else {
            continue;
        };
        verify_id(
            cache,
            findings,
            "agentConfig",
            &id,
            &relative_path(&path, &ws.root),
            ResourceRef {
                kind: ResourceKind::Agent,
                slug: string_field(&value, "slug").or(Some(stem)),
                id: Some(id.clone()),
            },
        )
        .await;
    }
}

async fn check_assistants(ws: &Workspace, cache: &mut RemoteCache, findings: &mut Vec<Finding>) {
    for (stem, path) in list_assistant_files(&ws.assistants_dir) {
        let Ok(value) = read_yaml(&path) else {
            continue;
        };
        let Some(id) = string_field(&value, "id") else {
            continue;
        };
        verify_id(
            cache,
            findings,
            "chat",
            &id,
            &relative_path(&path, &ws.root),
            ResourceRef {
                kind: ResourceKind::Assistant,
                slug: string_field(&value, "slug").or(Some(stem)),
                id: Some(id.clone()),
            },
        )
        .await;
    }
}

async fn check_hitl(ws: &Workspace, cache: &mut RemoteCache, findings: &mut Vec<Finding>) {
    for hitl_dir in list_hitl_dirs(&ws.hitl_dir) {
        let meta_path = if hitl_dir.join("meta.yaml").is_file() {
            hitl_dir.join("meta.yaml")
        } else {
            hitl_dir.join("meta.yml")
        };
        let Ok(value) = read_yaml(&meta_path) else {
            continue;
        };
        let Some(id) = string_field(&value, "id") else {
            continue;
        };
        let slug = string_field(&value, "slug").or_else(|| {
            hitl_dir
                .file_name()
                .and_then(|s| s.to_str())
                .map(str::to_string)
        });
        verify_id(
            cache,
            findings,
            "hitlConfig",
            &id,
            &relative_path(&meta_path, &ws.root),
            ResourceRef {
                kind: ResourceKind::Hitl,
                slug,
                id: Some(id.clone()),
            },
        )
        .await;
    }
}

async fn verify_id(
    cache: &mut RemoteCache,
    findings: &mut Vec<Finding>,
    collection: &str,
    id: &str,
    path: &str,
    resource: ResourceRef,
) {
    let lookup = cache.get_record(collection, id).await;
    match lookup.status {
        RemoteLookupStatus::Found => {}
        RemoteLookupStatus::NotFound => {
            push_finding(
                findings,
                Finding {
                    code: "REMOTE_ID_NOT_FOUND".to_string(),
                    severity: Severity::Error,
                    message: format!("platform record `{id}` not found in `{collection}`"),
                    path: Some(path.to_string()),
                    resource: Some(resource),
                },
            );
        }
        RemoteLookupStatus::Error => {
            push_finding(
                findings,
                Finding {
                    code: "REMOTE_LOOKUP_FAILED".to_string(),
                    severity: Severity::Warning,
                    message: lookup
                        .message
                        .unwrap_or_else(|| format!("failed to look up `{id}` in `{collection}`")),
                    path: Some(path.to_string()),
                    resource: Some(resource),
                },
            );
        }
    }
}
