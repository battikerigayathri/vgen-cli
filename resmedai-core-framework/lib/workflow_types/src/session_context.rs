use serde_json::{Value, json};

use crate::HistoryEntry;

const TRUNCATION_MARKER: &str = "[earlier turns truncated]";

/// Token/turn budget for orchestrator session history formatting.
#[derive(Debug, Clone, Copy)]
pub struct SessionContextBudget {
    pub max_turns: usize,
    pub max_chars: usize,
}

impl SessionContextBudget {
    pub fn from_env() -> Self {
        Self {
            max_turns: std::env::var("ORCHESTRATOR_HISTORY_MAX_TURNS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            max_chars: std::env::var("ORCHESTRATOR_HISTORY_MAX_CHARS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(24_000),
        }
    }
}

/// Build normalized history turns from raw compose history entries.
pub fn compose_history_turns(raw_entries: &[Value]) -> Vec<Value> {
    raw_entries.iter().map(normalize_history_turn).collect()
}

fn normalize_history_turn(item: &Value) -> Value {
    let user = item
        .get("userMessage")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let assistant = item.get("agentResponse").and_then(|v| v.as_str());
    let attachment_summary = item
        .get("attachments")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| {
                    let name = a.get("fileName").and_then(|v| v.as_str())?;
                    let id = a.get("fileId").and_then(|v| v.as_str())?;
                    let mime = a.get("mimeType").and_then(|v| v.as_str())?;
                    Some(format!("{name} (FILE_ID: {id}, MimeType: {mime})"))
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let user_message = if attachment_summary.is_empty() {
        user.to_string()
    } else {
        format!("{user}\nAttached Files: {attachment_summary}")
    };
    json!({
        "userMessage": user_message,
        "agentResponse": assistant
            .map(|s| Value::String(s.to_string()))
            .unwrap_or(Value::Null),
        "functions": item
            .get("functionsResults")
            .or_else(|| item.get("functions"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default(),
    })
}

/// Structured history entries for `WorkflowState.history`.
pub fn history_entries_from_turns(turns: &[Value]) -> Vec<HistoryEntry> {
    turns
        .iter()
        .filter_map(|h| {
            Some(HistoryEntry {
                user_message: h.get("userMessage").and_then(|v| v.as_str())?.to_string(),
                agent_response: h
                    .get("agentResponse")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                functions: h
                    .get("functions")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default(),
            })
        })
        .collect()
}

fn apply_turn_budget(turns: &[Value], max_turns: usize) -> Vec<Value> {
    if max_turns == 0 || turns.len() <= max_turns {
        return turns.to_vec();
    }
    turns[turns.len() - max_turns..].to_vec()
}

fn apply_char_budget(turns: &[Value], max_chars: usize) -> (Vec<Value>, bool) {
    if max_chars == 0 {
        return (turns.to_vec(), false);
    }
    let mut selected = Vec::new();
    let mut total = 0usize;
    let mut truncated = false;
    for turn in turns.iter().rev() {
        let turn_len = serde_json::to_string(turn).map(|s| s.len()).unwrap_or(0);
        if !selected.is_empty() && total + turn_len > max_chars {
            truncated = true;
            break;
        }
        total += turn_len;
        selected.push(turn.clone());
    }
    selected.reverse();
    if selected.len() < turns.len() {
        truncated = true;
    }
    (selected, truncated)
}

/// Format session history for orchestrator prompt slot `{1}`.
pub fn format_history_for_orchestrator(turns: &[Value], budget: SessionContextBudget) -> String {
    let after_turns = apply_turn_budget(turns, budget.max_turns);
    let (selected, truncated) = apply_char_budget(&after_turns, budget.max_chars);
    let body = serde_json::to_string_pretty(&selected).unwrap_or_else(|_| "[]".to_string());
    if truncated {
        format!("{TRUNCATION_MARKER}\n{body}")
    } else {
        body
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn compose_history_turns_maps_functions_results() {
        let raw = vec![json!({
            "userMessage": "hi",
            "agentResponse": "hello",
            "functionsResults": [{"result": "x", "skill_id": "s1"}]
        })];
        let turns = compose_history_turns(&raw);
        assert_eq!(turns[0]["userMessage"], "hi");
        assert!(turns[0]["functions"].is_array());
    }

    #[test]
    fn format_history_truncates_by_chars() {
        let turns: Vec<Value> = (0..20)
            .map(|i| json!({"userMessage": format!("message-{i}"), "agentResponse": "x".repeat(500)}))
            .collect();
        let budget = SessionContextBudget {
            max_turns: 0,
            max_chars: 800,
        };
        let formatted = format_history_for_orchestrator(&turns, budget);
        assert!(formatted.contains(TRUNCATION_MARKER));
    }
}
