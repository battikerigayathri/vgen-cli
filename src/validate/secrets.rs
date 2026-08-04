use crate::specs::list_tool_dirs;
use crate::workspace::Workspace;

use super::{push_finding, relative_path, Finding, ResourceKind, ResourceRef, Severity};

const HANDLER_NAMES: &[&str] = &["handler.js", "index.js", "handler.ts", "index.ts"];

pub fn check_secrets(ws: &Workspace) -> Vec<Finding> {
    let mut findings = Vec::new();
    for tool_dir in list_tool_dirs(&ws.tools_dir) {
        for handler_name in HANDLER_NAMES {
            let handler_path = tool_dir.join(handler_name);
            if !handler_path.is_file() {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&handler_path) else {
                continue;
            };
            let Some(label) = find_secret_pattern(&content) else {
                continue;
            };
            let rel = relative_path(&handler_path, &ws.root);
            let slug = tool_dir
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            push_finding(
                &mut findings,
                Finding {
                    code: "SECRET_SUSPECTED".to_string(),
                    severity: Severity::Warning,
                    message: format!(
                        "handler may contain a hardcoded secret (matched pattern: {label})"
                    ),
                    path: Some(rel),
                    resource: Some(ResourceRef {
                        kind: ResourceKind::Tool,
                        slug: Some(slug),
                        id: None,
                    }),
                },
            );
            break;
        }
    }
    findings
}

fn find_secret_pattern(content: &str) -> Option<&'static str> {
    if contains_aws_access_key(content) {
        return Some("AKIA");
    }
    if content.contains("BEGIN RSA PRIVATE KEY")
        || content.contains("BEGIN EC PRIVATE KEY")
        || content.contains("BEGIN PRIVATE KEY")
    {
        return Some("PRIVATE_KEY");
    }
    if content.lines().any(|line| {
        let line = line.trim();
        line.contains("api_key")
            && line.contains('=')
            && (line.contains('\'') || line.contains('"'))
    }) {
        return Some("API_KEY_ASSIGN");
    }
    if content.lines().any(|line| {
        let line = line.trim();
        line.contains("password")
            && line.contains('=')
            && (line.contains('\'') || line.contains('"'))
    }) {
        return Some("PASSWORD_ASSIGN");
    }
    if contains_openai_style_key(content) {
        return Some("OPENAI_SK");
    }
    None
}

fn contains_aws_access_key(content: &str) -> bool {
    for (idx, _) in content.match_indices("AKIA") {
        let rest = &content[idx..];
        let key: String = rest.chars().take(20).collect();
        if key.len() == 20
            && key.starts_with("AKIA")
            && key
                .chars()
                .skip(4)
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        {
            return true;
        }
    }
    false
}

fn contains_openai_style_key(content: &str) -> bool {
    for (idx, _) in content.match_indices("sk-") {
        let suffix: String = content[idx + 3..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        if suffix.len() >= 20 {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_aws_key_pattern() {
        assert!(contains_aws_access_key("const x = 'AKIAIOSFODNN7EXAMPLE';"));
    }

    #[test]
    fn detects_openai_key_pattern() {
        assert!(contains_openai_style_key(
            "sk-abcdefghijklmnopqrstuvwxyz123456"
        ));
    }
}
