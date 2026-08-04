use crate::specs::{
    list_agent_files, list_assistant_files, list_hitl_dirs, list_tool_dirs, load_tool_yaml,
};
use crate::workspace::Workspace;

use super::{
    missing_required_field, push_finding, read_yaml, relative_path, string_field, Finding,
    ResourceKind, ResourceRef, Severity,
};

const HANDLER_NAMES: &[&str] = &["handler.js", "index.js", "handler.ts", "index.ts"];

pub fn check_artifact_rules(ws: &Workspace) -> Vec<Finding> {
    let mut findings = Vec::new();
    check_tools(ws, &mut findings);
    check_agents(ws, &mut findings);
    check_assistants(ws, &mut findings);
    check_hitl(ws, &mut findings);
    findings
}

fn check_tools(ws: &Workspace, findings: &mut Vec<Finding>) {
    for tool_dir in list_tool_dirs(&ws.tools_dir) {
        let rel_dir = relative_path(&tool_dir, &ws.root);
        let Ok((value, yaml_path)) = load_tool_yaml(&tool_dir) else {
            continue;
        };
        let slug = tool_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        let resource = ResourceRef {
            kind: ResourceKind::Tool,
            slug: Some(slug),
            id: string_field(&value, "id"),
        };

        if string_field(&value, "name").is_none() {
            missing_required_field(findings, &yaml_path, ws, resource.clone(), "name");
        }
        if string_field(&value, "type").is_none() {
            missing_required_field(findings, &yaml_path, ws, resource.clone(), "type");
        }

        if !has_handler(&tool_dir) {
            push_finding(
                findings,
                Finding {
                    code: "MISSING_HANDLER".to_string(),
                    severity: Severity::Error,
                    message:
                        "tool directory is missing a handler file (handler.js, index.js, etc.)"
                            .to_string(),
                    path: Some(rel_dir.clone()),
                    resource: Some(resource.clone()),
                },
            );
        }

        let tool_type = value
            .get("type")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .unwrap_or_default();
        let is_js = tool_type.eq_ignore_ascii_case("js");
        let is_faas = tool_type.eq_ignore_ascii_case("faas");
        let has_package = tool_dir.join("package.json").is_file();

        if is_faas && !has_package {
            push_finding(
                findings,
                Finding {
                    code: "FAAS_MISSING_PACKAGE".to_string(),
                    severity: Severity::Error,
                    message: "FAAS tool is missing package.json".to_string(),
                    path: Some(rel_dir.clone()),
                    resource: Some(resource.clone()),
                },
            );
        }
        if is_js && has_package {
            push_finding(
                findings,
                Finding {
                    code: "JS_UNEXPECTED_PACKAGE".to_string(),
                    severity: Severity::Warning,
                    message: "JS tool has package.json; JS tools typically omit package.json"
                        .to_string(),
                    path: Some(rel_dir.clone()),
                    resource: Some(resource.clone()),
                },
            );
        }

        // Lint feeds using workflow_types
        let issues = workflow_types::lint_tool_feeds(&value);
        for issue in issues {
            let severity = match issue.severity {
                workflow_types::FeedValidationSeverity::Error => Severity::Error,
                workflow_types::FeedValidationSeverity::Warning => Severity::Warning,
            };
            push_finding(
                findings,
                Finding {
                    code: "TOOL_FEEDS_INVALID".to_string(),
                    severity,
                    message: format!("{}: {}", issue.path, issue.message),
                    path: Some(relative_path(&yaml_path, &ws.root)),
                    resource: Some(resource.clone()),
                },
            );
        }

        // Warn when a tool has output schema but neither feeds nor x-feeds
        let output = value.get("output");
        let has_feeds = value.get("feeds").is_some();
        let has_x_feeds = output
            .and_then(|o| o.get("x-feeds").or_else(|| o.get("x_feeds")))
            .is_some();
        if output.is_some() && !has_feeds && !has_x_feeds {
            push_finding(
                findings,
                Finding {
                    code: "TOOL_FEEDS_MISSING".to_string(),
                    severity: Severity::Warning,
                    message: "tool has output schema but neither feeds nor x-feeds; runtime will use full-payload fallback".to_string(),
                    path: Some(relative_path(&yaml_path, &ws.root)),
                    resource: Some(resource.clone()),
                },
            );
        }
    }
}

fn has_handler(tool_dir: &std::path::Path) -> bool {
    HANDLER_NAMES
        .iter()
        .any(|name| tool_dir.join(name).is_file())
}

fn check_agents(ws: &Workspace, findings: &mut Vec<Finding>) {
    for (stem, path) in list_agent_files(&ws.agents_dir) {
        let Ok(value) = read_yaml(&path) else {
            continue;
        };
        let resource = ResourceRef {
            kind: ResourceKind::Agent,
            slug: string_field(&value, "slug").or(Some(stem)),
            id: string_field(&value, "id"),
        };
        if string_field(&value, "name").is_none() {
            missing_required_field(findings, &path, ws, resource.clone(), "name");
        }
        if !value
            .get("skills")
            .and_then(|v| v.as_array())
            .is_some_and(|a| !a.is_empty())
        {
            missing_required_field(findings, &path, ws, resource, "skills");
        }
    }
}

fn check_assistants(ws: &Workspace, findings: &mut Vec<Finding>) {
    for (stem, path) in list_assistant_files(&ws.assistants_dir) {
        let Ok(value) = read_yaml(&path) else {
            continue;
        };
        let resource = ResourceRef {
            kind: ResourceKind::Assistant,
            slug: string_field(&value, "slug").or(Some(stem)),
            id: string_field(&value, "id"),
        };
        if string_field(&value, "name").is_none() {
            missing_required_field(findings, &path, ws, resource.clone(), "name");
        }
        if !value
            .get("agents")
            .and_then(|v| v.as_array())
            .is_some_and(|a| !a.is_empty())
        {
            missing_required_field(findings, &path, ws, resource.clone(), "agents");
        }

        // Collect catalog tool IDs for this assistant to pass to validation
        let mut catalog_tool_ids = Vec::new();
        if let Some(agents_array) = value.get("agents").and_then(|v| v.as_array()) {
            for agent_ref in agents_array {
                if let Some(agent_id) = agent_ref.as_str().map(str::trim).filter(|s| !s.is_empty())
                {
                    if let Some(agent_path) = find_agent_path_by_id(ws, agent_id) {
                        if let Ok(agent_doc) = read_yaml(&agent_path) {
                            let agent_slug =
                                string_field(&agent_doc, "slug").unwrap_or_else(|| {
                                    agent_path
                                        .file_stem()
                                        .unwrap()
                                        .to_str()
                                        .unwrap()
                                        .to_string()
                                });
                            if let Some(skills_array) =
                                agent_doc.get("skills").and_then(|v| v.as_array())
                            {
                                for skill_ref in skills_array {
                                    if let Some(tool_id) =
                                        skill_ref.as_str().map(str::trim).filter(|s| !s.is_empty())
                                    {
                                        if let Some(tool_dir) = find_tool_dir_by_id(ws, tool_id) {
                                            if let Ok((tool_doc, _)) = load_tool_yaml(&tool_dir) {
                                                if let Some(tool_name) =
                                                    string_field(&tool_doc, "name")
                                                {
                                                    catalog_tool_ids.push(format!(
                                                        "{}__{}",
                                                        agent_slug, tool_name
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Validate assistant feeds overrides
        let issues = workflow_types::lint_assistant_tool_feeds(&value, &catalog_tool_ids);
        for issue in issues {
            let severity = match issue.severity {
                workflow_types::FeedValidationSeverity::Error => Severity::Error,
                workflow_types::FeedValidationSeverity::Warning => Severity::Warning,
            };
            push_finding(
                findings,
                Finding {
                    code: "ASSISTANT_TOOL_FEEDS_INVALID".to_string(),
                    severity,
                    message: format!("{}: {}", issue.path, issue.message),
                    path: Some(relative_path(&path, &ws.root)),
                    resource: Some(resource.clone()),
                },
            );
        }
    }
}

fn find_agent_path_by_id(ws: &Workspace, agent_id: &str) -> Option<std::path::PathBuf> {
    for (_, path) in list_agent_files(&ws.agents_dir) {
        if let Ok(doc) = read_yaml(&path) {
            if let Some(id) = string_field(&doc, "id") {
                if id == agent_id {
                    return Some(path);
                }
            }
        }
    }
    None
}

fn find_tool_dir_by_id(ws: &Workspace, tool_id: &str) -> Option<std::path::PathBuf> {
    for tool_dir in list_tool_dirs(&ws.tools_dir) {
        if let Ok((doc, _)) = load_tool_yaml(&tool_dir) {
            if let Some(id) = string_field(&doc, "id") {
                if id == tool_id {
                    return Some(tool_dir);
                }
            }
        }
    }
    None
}

fn check_hitl(ws: &Workspace, findings: &mut Vec<Finding>) {
    for hitl_dir in list_hitl_dirs(&ws.hitl_dir) {
        let rel_dir = relative_path(&hitl_dir, &ws.root);
        let meta_path = if hitl_dir.join("meta.yaml").is_file() {
            hitl_dir.join("meta.yaml")
        } else {
            hitl_dir.join("meta.yml")
        };
        let Ok(value) = read_yaml(&meta_path) else {
            continue;
        };
        let slug = string_field(&value, "slug").or_else(|| {
            hitl_dir
                .file_name()
                .and_then(|s| s.to_str())
                .map(str::to_string)
        });
        let resource = ResourceRef {
            kind: ResourceKind::Hitl,
            slug: slug.clone(),
            id: string_field(&value, "id"),
        };
        if string_field(&value, "name").is_none() {
            missing_required_field(findings, &meta_path, ws, resource.clone(), "name");
        }
        if slug.is_none() {
            missing_required_field(findings, &meta_path, ws, resource.clone(), "slug");
        }
        if !hitl_dir.join("config.json").is_file() {
            push_finding(
                findings,
                Finding {
                    code: "MISSING_HITL_CONFIG".to_string(),
                    severity: Severity::Error,
                    message: "HITL directory is missing config.json".to_string(),
                    path: Some(rel_dir),
                    resource: Some(resource),
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::Severity;
    use crate::workspace::Workspace;

    #[test]
    fn test_tool_feeds_invalid() {
        let uuid_str = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string();
        let dir = std::env::temp_dir().join(format!("resmate-test-{}", uuid_str));
        let tools_dir = dir.join("tools");
        let tool_dir = tools_dir.join("test-tool");
        std::fs::create_dir_all(&tool_dir).unwrap();

        // Create tool.yaml with invalid feeds
        let tool_yaml_content = r#"
name: test-tool
type: JS
feeds:
  invalid_consumer: [field1]
"#;
        std::fs::write(tool_dir.join("tool.yaml"), tool_yaml_content).unwrap();
        std::fs::write(tool_dir.join("handler.js"), "return 'hello';").unwrap();

        let ws = Workspace {
            root: dir.clone(),
            agents_dir: dir.join("agents"),
            assistants_dir: dir.join("assistants"),
            tools_dir,
            hitl_dir: dir.join("hitl"),
            workflows_dir: dir.join("workflows"),
        };

        let mut findings = Vec::new();
        check_tools(&ws, &mut findings);

        assert!(!findings.is_empty());
        let feed_finding = findings
            .iter()
            .find(|f| f.code == "TOOL_FEEDS_INVALID")
            .unwrap();
        assert_eq!(feed_finding.severity, Severity::Error);
        assert!(feed_finding.message.contains("unknown feeds consumer"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_assistant_tool_feeds_invalid() {
        let uuid_str = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string();
        let dir = std::env::temp_dir().join(format!("resmate-test-{}", uuid_str));
        let assistants_dir = dir.join("assistants");
        std::fs::create_dir_all(&assistants_dir).unwrap();

        // Create assistant.yaml with invalid tool_feeds overrides
        let assistant_yaml_content = r#"
name: test-assistant
slug: test-assistant
agents: []
tool_feeds:
  agent__tool:
    planner: [""]
"#;
        std::fs::write(
            assistants_dir.join("test-assistant.yaml"),
            assistant_yaml_content,
        )
        .unwrap();

        let ws = Workspace {
            root: dir.clone(),
            agents_dir: dir.join("agents"),
            assistants_dir,
            tools_dir: dir.join("tools"),
            hitl_dir: dir.join("hitl"),
            workflows_dir: dir.join("workflows"),
        };

        let mut findings = Vec::new();
        check_assistants(&ws, &mut findings);

        assert!(!findings.is_empty());
        let feed_finding = findings
            .iter()
            .find(|f| f.code == "ASSISTANT_TOOL_FEEDS_INVALID")
            .unwrap();
        assert_eq!(feed_finding.severity, Severity::Error);
        assert!(feed_finding
            .message
            .contains("feed field name must not be empty"));

        std::fs::remove_dir_all(&dir).ok();
    }
}
