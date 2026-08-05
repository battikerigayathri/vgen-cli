use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedValidationSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone)]
pub struct FeedValidationIssue {
    pub path: String,
    pub message: String,
    pub severity: FeedValidationSeverity,
}

const VALID_CONSUMERS: &[&str] = &[
    "planner",
    "compose",
    "compose_verbatim",
    "macro_validator",
];

fn is_valid_field_name(field: &str) -> bool {
    !field.is_empty()
        && field
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
}

fn lint_feeds_block(feeds: &Value, path_prefix: &str) -> Vec<FeedValidationIssue> {
    let mut issues = Vec::new();

    let Some(obj) = feeds.as_object() else {
        issues.push(FeedValidationIssue {
            path: path_prefix.to_string(),
            message: "feeds block must be an object".to_string(),
            severity: FeedValidationSeverity::Error,
        });
        return issues;
    };

    for (consumer, fields) in obj {
        let consumer_path = format!("{path_prefix}.{consumer}");
        if !VALID_CONSUMERS.contains(&consumer.as_str()) {
            issues.push(FeedValidationIssue {
                path: consumer_path.clone(),
                message: format!("unknown feed consumer '{consumer}'"),
                severity: FeedValidationSeverity::Error,
            });
            continue;
        }

        let Some(arr) = fields.as_array() else {
            issues.push(FeedValidationIssue {
                path: consumer_path,
                message: "consumer entry must be an array of field names".to_string(),
                severity: FeedValidationSeverity::Error,
            });
            continue;
        };

        if arr.is_empty() {
            issues.push(FeedValidationIssue {
                path: consumer_path.clone(),
                message: "consumer field list must not be empty".to_string(),
                severity: FeedValidationSeverity::Error,
            });
        }

        let mut seen = HashSet::new();
        for (idx, field) in arr.iter().enumerate() {
            let field_path = format!("{consumer_path}[{idx}]");
            let Some(field_name) = field.as_str() else {
                issues.push(FeedValidationIssue {
                    path: field_path,
                    message: "field entry must be a string".to_string(),
                    severity: FeedValidationSeverity::Error,
                });
                continue;
            };
            if !is_valid_field_name(field_name) {
                issues.push(FeedValidationIssue {
                    path: field_path.clone(),
                    message: format!("invalid field name '{field_name}'"),
                    severity: FeedValidationSeverity::Error,
                });
            }
            if !seen.insert(field_name.to_string()) {
                issues.push(FeedValidationIssue {
                    path: field_path,
                    message: format!("duplicate field '{field_name}'"),
                    severity: FeedValidationSeverity::Error,
                });
            }
        }
    }

    issues
}

pub fn lint_tool_feeds(tool: &Value) -> Vec<FeedValidationIssue> {
    let mut issues = Vec::new();

    if let Some(feeds) = tool.get("feeds") {
        issues.extend(lint_feeds_block(feeds, "feeds"));
    }

    if let Some(output) = tool.get("output") {
        if let Some(x_feeds) = output.get("x-feeds").or_else(|| output.get("x_feeds")) {
            issues.extend(lint_feeds_block(x_feeds, "output.x-feeds"));
        }
    }

    issues
}

pub fn lint_assistant_tool_feeds(
    assistant: &Value,
    catalog_tool_ids: &[String],
) -> Vec<FeedValidationIssue> {
    let Some(tool_feeds) = assistant.get("tool_feeds") else {
        return Vec::new();
    };

    let Some(obj) = tool_feeds.as_object() else {
        return vec![FeedValidationIssue {
            path: "tool_feeds".to_string(),
            message: "tool_feeds must be an object".to_string(),
            severity: FeedValidationSeverity::Error,
        }];
    };

    let catalog: HashSet<&str> = catalog_tool_ids.iter().map(String::as_str).collect();
    let mut issues = Vec::new();

    for (tool_id, feeds) in obj {
        let path_prefix = format!("tool_feeds.{tool_id}");
        if !catalog.is_empty() && !catalog.contains(tool_id.as_str()) {
            issues.push(FeedValidationIssue {
                path: path_prefix.clone(),
                message: format!("unknown catalog tool id '{tool_id}'"),
                severity: FeedValidationSeverity::Warning,
            });
        }
        issues.extend(lint_feeds_block(feeds, &path_prefix));
    }

    issues
}
