use crate::specs::{list_tool_dirs, load_tool_yaml};
use crate::workspace::Workspace;
use std::path::Path;

use super::{push_finding, relative_path, Finding, ResourceKind, ResourceRef, Severity};

const HANDLER_NAMES: &[&str] = &["handler.js", "index.js", "handler.ts", "index.ts"];

pub fn check_handler_lint(ws: &Workspace) -> Vec<Finding> {
    let mut findings = Vec::new();
    for tool_dir in list_tool_dirs(&ws.tools_dir) {
        lint_tool(ws, &tool_dir, &mut findings);
    }
    findings
}

fn lint_tool(ws: &Workspace, tool_dir: &Path, findings: &mut Vec<Finding>) {
    let Ok((value, yaml_path)) = load_tool_yaml(tool_dir) else {
        return;
    };
    let rel_dir = relative_path(tool_dir, &ws.root);
    let slug = tool_dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();
    let resource = ResourceRef {
        kind: ResourceKind::Tool,
        slug: Some(slug.clone()),
        id: value.get("id").and_then(|v| v.as_str()).map(str::to_string),
    };

    let tool_type = value
        .get("type")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or_default();
    let is_js = tool_type.eq_ignore_ascii_case("js");
    let is_faas = tool_type.eq_ignore_ascii_case("faas");

    let handler_path = find_handler(tool_dir);
    let handler_source = handler_path
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok());

    if is_likely_save_tool(&slug, &value) {
        if let Some(source) = &handler_source {
            if !source.contains("workflowPatch") {
                push_finding(
                    findings,
                    Finding {
                        code: "MISSING_WORKFLOW_PATCH".to_string(),
                        severity: Severity::Warning,
                        message: "save tool handler does not reference workflowPatch in source"
                            .to_string(),
                        path: Some(rel_dir.clone()),
                        resource: Some(resource.clone()),
                    },
                );
            }
        }
    }

    if is_faas {
        if let Some(source) = &handler_source {
            let has_faas_shape = source.contains("export async function handler")
                || source.contains("export function handler")
                || source.contains("module.exports")
                || source.contains("exports.handler");
            if !has_faas_shape {
                push_finding(
                    findings,
                    Finding {
                        code: "FAAS_HANDLER_SHAPE".to_string(),
                        severity: Severity::Warning,
                        message: "FAAS handler should export an async handler function".to_string(),
                        path: Some(
                            handler_path
                                .as_ref()
                                .map(|p| relative_path(p, &ws.root))
                                .unwrap_or(rel_dir.clone()),
                        ),
                        resource: Some(resource.clone()),
                    },
                );
            }
        }
    }

    if is_js {
        if let Some(source) = &handler_source {
            let uses_json_stringify = source.contains("JSON.stringify");
            let uses_module_exports = source.contains("module.exports");
            if uses_module_exports && !uses_json_stringify {
                push_finding(
                    findings,
                    Finding {
                        code: "JS_HANDLER_SHAPE".to_string(),
                        severity: Severity::Warning,
                        message:
                            "JS tool handler should return JSON.stringify(...) rather than module.exports"
                                .to_string(),
                        path: Some(handler_path
                            .as_ref()
                            .map(|p| relative_path(p, &ws.root))
                            .unwrap_or(rel_dir)),
                        resource: Some(resource),
                    },
                );
            }
        } else if !yaml_path.exists() {
            let _ = yaml_path;
        }
    }
}

fn is_likely_save_tool(slug: &str, yaml: &serde_json::Value) -> bool {
    if slug.ends_with("-save") || slug.contains("save") {
        return true;
    }
    let description = yaml
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    description.contains("workflowpatch") || description.contains("workflow patch")
}

fn find_handler(tool_dir: &Path) -> Option<std::path::PathBuf> {
    HANDLER_NAMES
        .iter()
        .map(|name| tool_dir.join(name))
        .find(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_save_tool_by_slug() {
        assert!(is_likely_save_tool(
            "roc-select-requester-save",
            &serde_json::json!({})
        ));
        assert!(!is_likely_save_tool(
            "roc-search-users",
            &serde_json::json!({})
        ));
    }
}
