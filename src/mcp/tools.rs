use serde_json::{json, Value};

pub fn list_tool_definitions() -> Vec<Value> {
    vec![
        tool(
            "workspace_info",
            "Workspace detection and artifact summary",
            false,
            json!({
                "type": "object",
                "properties": {
                    "workspace_root": { "type": "string", "description": "Path to use-case workspace; default cwd" }
                }
            }),
        ),
        tool(
            "doctor",
            "Environment and connectivity diagnostics",
            false,
            json!({
                "type": "object",
                "properties": {
                    "workspace_root": { "type": "string" },
                    "offline": { "type": "boolean", "description": "Skip API connectivity check" }
                }
            }),
        ),
        tool(
            "graph",
            "Dependency link graph",
            false,
            json!({
                "type": "object",
                "properties": {
                    "workspace_root": { "type": "string" },
                    "assistant": { "type": "string", "description": "Filter to one assistant subtree" }
                }
            }),
        ),
        tool(
            "validate",
            "Aggregate workspace validation",
            false,
            json!({
                "type": "object",
                "properties": {
                    "workspace_root": { "type": "string" },
                    "strict": { "type": "boolean" }
                }
            }),
        ),
        tool(
            "workflow_validate",
            "Validate one workflow locally (schema + semantics, including transitions, dead-end checks, path-aware completeness via requiredFromStage, reset on rework-loop transitions — see spec-workflow.md §2.4.3; named gates via gates.yaml with composed predicates and cycle detection — see §2.4.5)",
            false,
            json!({
                "type": "object",
                "properties": {
                    "workspace_root": { "type": "string" },
                    "name": { "type": "string", "description": "Workflow folder name" }
                },
                "required": ["name"]
            }),
        ),
        tool(
            "push_all_dry_run",
            "Plan batch push without HTTP",
            false,
            json!({
                "type": "object",
                "properties": {
                    "workspace_root": { "type": "string" },
                    "assistant": { "type": "string" }
                }
            }),
        ),
        tool(
            "push_all_execute",
            "Execute batch push in dependency order",
            true,
            json!({
                "type": "object",
                "properties": {
                    "workspace_root": { "type": "string" },
                    "assistant": { "type": "string" },
                    "force": { "type": "boolean" },
                    "confirm": { "type": "boolean", "description": "Must be true to execute" }
                },
                "required": ["confirm"]
            }),
        ),
        tool(
            "explain",
            "Explain an error/finding code",
            false,
            json!({
                "type": "object",
                "properties": {
                    "code": { "type": "string" }
                },
                "required": ["code"]
            }),
        ),
        tool(
            "explain_list",
            "List known error codes",
            false,
            json!({
                "type": "object",
                "properties": {
                    "domain": { "type": "string", "description": "Filter by domain" }
                }
            }),
        ),
        tool(
            "init_workspace",
            "Bootstrap a ResMate workspace into an empty or allowlisted directory (allowlist: .git, .gitignore, README.md, .DS_Store). Refuses with INIT_REFUSED and lists offending top-level entries otherwise; force bootstrap-overwrites kit/seed files without deleting live artifacts (prefer kit_update for refreshing existing repos)",
            true,
            json!({
                "type": "object",
                "properties": {
                    "workspace_root": { "type": "string" },
                    "name": { "type": "string" },
                    "description": { "type": "string", "description": "Description for resmate.yaml (default: \"ResMate use case workspace\")" },
                    "force": { "type": "boolean", "description": "Bootstrap-overwrite kit/seed in a non-init-safe directory; never deletes live artifacts" },
                    "confirm": { "type": "boolean", "description": "Must be true to init" }
                },
                "required": ["confirm"]
            }),
        ),
        tool(
            "scaffold_recipe",
            "Scaffold workspace from recipe",
            true,
            json!({
                "type": "object",
                "properties": {
                    "workspace_root": { "type": "string" },
                    "recipe": { "type": "string" },
                    "name": { "type": "string" },
                    "force": { "type": "boolean" },
                    "confirm": { "type": "boolean", "description": "Must be true to scaffold" }
                },
                "required": ["recipe", "confirm"]
            }),
        ),
        tool(
            "kit_update",
            "Refresh the authoring kit files (skills, rules, docs, AGENTS.md) in an already-scaffolded workspace without touching live artifacts (tools/agents/assistants/hitl/workflows) or user config; skips examples/ by default; updates kit_version in resmate.yaml",
            true,
            json!({
                "type": "object",
                "properties": {
                    "workspace_root": { "type": "string" },
                    "examples": { "type": "boolean", "description": "Also refresh the examples/ reference tree" },
                    "dry_run": { "type": "boolean", "description": "List paths that would be written without modifying the filesystem" },
                    "confirm": { "type": "boolean", "description": "Must be true to update" }
                },
                "required": ["confirm"]
            }),
        ),
    ]
}

fn tool(name: &str, description: &str, mutating: bool, input_schema: Value) -> Value {
    let desc = if mutating {
        format!("{description} (mutating — requires confirm: true)")
    } else {
        description.to_string()
    };
    json!({
        "name": name,
        "description": desc,
        "inputSchema": input_schema
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_least_eight_tools() {
        assert!(list_tool_definitions().len() >= 8);
    }

    #[test]
    fn mutating_tools_require_confirm_in_schema() {
        for tool in list_tool_definitions() {
            let name = tool["name"].as_str().unwrap_or("");
            if name == "push_all_execute"
                || name == "init_workspace"
                || name == "scaffold_recipe"
                || name == "kit_update"
            {
                let required = tool["inputSchema"]["required"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                assert!(
                    required.iter().any(|v| v == "confirm"),
                    "{} should require confirm",
                    name
                );
            }
        }
    }
}
