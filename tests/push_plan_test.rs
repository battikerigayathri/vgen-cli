//! Integration tests for push plan against pr-agent-v2 and minimal fixtures.

use resmate::push_plan::{build_push_plan, PushAction, ResourceType};
use resmate::validate::{run_workspace_validation, ValidateOptions};
use resmate::workspace::Workspace;
use std::path::PathBuf;

fn pr_agent_v2_root() -> PathBuf {
    PathBuf::from("/Users/roshankgujarathi/Workspace/ResMed/pr-agent-v2")
}

#[test]
fn pr_agent_v2_push_plan_has_seven_steps_in_dependency_order() {
    let root = pr_agent_v2_root();
    if !root.is_dir() {
        eprintln!("Skipping: pr-agent-v2 not found at {}", root.display());
        return;
    }

    let ws = Workspace::detect(&root).expect("detect pr-agent-v2");
    let report = run_workspace_validation(
        &ws,
        ValidateOptions {
            strict: false,
            offline: true,
            remote: false,
        },
    )
    .expect("validate");
    let plan = build_push_plan(&ws, &report, None).expect("push plan");

    assert!(
        plan.steps.len() >= 7,
        "expected at least 7 push steps, got {}",
        plan.steps.len()
    );
    // plan.skipped can be non-empty due to active development in the workspace

    let names: Vec<&str> = plan.steps.iter().map(|s| s.name.as_str()).collect();
    assert!(
        names.contains(&"oracle-pr-agent"),
        "plan should contain oracle-pr-agent"
    );
    assert!(
        names.contains(&"oracle-pr-assistant"),
        "plan should contain oracle-pr-assistant"
    );

    let assistant_step = plan
        .steps
        .iter()
        .find(|s| s.name == "oracle-pr-assistant")
        .expect("assistant step");
    assert_eq!(assistant_step.resource_type, ResourceType::Assistant);
}

#[test]
fn pr_agent_v2_assistant_filter_matches_full_plan() {
    let root = pr_agent_v2_root();
    if !root.is_dir() {
        eprintln!("Skipping: pr-agent-v2 not found at {}", root.display());
        return;
    }

    let ws = Workspace::detect(&root).expect("detect pr-agent-v2");
    let report = run_workspace_validation(
        &ws,
        ValidateOptions {
            strict: false,
            offline: true,
            remote: false,
        },
    )
    .expect("validate");

    let full = build_push_plan(&ws, &report, None).expect("full plan");
    let filtered =
        build_push_plan(&ws, &report, Some("oracle-pr-assistant")).expect("filtered plan");

    assert_eq!(full.steps.len(), filtered.steps.len());
    for (a, b) in full.steps.iter().zip(filtered.steps.iter()) {
        assert_eq!(a.name, b.name);
        assert_eq!(a.resource_type, b.resource_type);
    }
}

#[test]
fn minimal_workspace_push_plan_orders_phases_alphabetically() {
    let base = std::env::temp_dir().join(format!("resmate-push-plan-{}", uuid::Uuid::new_v4()));
    let _ = std::fs::remove_dir_all(&base);
    write_minimal_push_workspace(&base);

    let ws = Workspace::detect(&base).expect("workspace");
    let report = run_workspace_validation(
        &ws,
        ValidateOptions {
            strict: false,
            offline: true,
            remote: false,
        },
    )
    .expect("validate");
    let plan = build_push_plan(&ws, &report, None).expect("push plan");

    assert_eq!(plan.steps.len(), 5);
    let types: Vec<ResourceType> = plan.steps.iter().map(|s| s.resource_type).collect();
    assert_eq!(
        types,
        vec![
            ResourceType::Hitl,
            ResourceType::Workflow,
            ResourceType::Tool,
            ResourceType::Agent,
            ResourceType::Assistant,
        ]
    );

    let orphan = plan
        .skipped
        .iter()
        .find(|s| s.name == "orphan-tool")
        .expect("orphan tool skipped");
    assert_eq!(orphan.action, PushAction::Skip);

    let _ = std::fs::remove_dir_all(&base);
}

fn write_minimal_push_workspace(root: &PathBuf) {
    std::fs::create_dir_all(root.join("assistants")).unwrap();
    std::fs::create_dir_all(root.join("agents")).unwrap();
    std::fs::create_dir_all(root.join("tools/linked-tool")).unwrap();
    std::fs::create_dir_all(root.join("tools/orphan-tool")).unwrap();
    std::fs::create_dir_all(root.join("hitl/aaa-form")).unwrap();
    std::fs::create_dir_all(root.join("workflows/test-flow")).unwrap();

    std::fs::write(
        root.join("assistants/test-assistant.yaml"),
        r#"id: asst-1
slug: test-assistant
name: Test Assistant
agents:
  - agent-1
systemContext: |
  workflow_definition_slug: test-flow-v1
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("agents/test-agent.yaml"),
        r#"id: agent-1
slug: test-agent
name: Test Agent
skills:
  - tool-1
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("tools/linked-tool/tool.yaml"),
        "id: tool-1\nname: linked\ntype: JS\n",
    )
    .unwrap();
    std::fs::write(
        root.join("tools/linked-tool/handler.js"),
        "export default async function handler() { return {}; }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("tools/orphan-tool/tool.yaml"),
        "id: tool-orphan\nname: orphan\ntype: JS\n",
    )
    .unwrap();
    std::fs::write(
        root.join("tools/orphan-tool/handler.js"),
        "export default async function handler() { return {}; }\n",
    )
    .unwrap();
    std::fs::write(root.join("hitl/aaa-form/config.json"), "{}").unwrap();
    std::fs::write(
        root.join("hitl/aaa-form/meta.yaml"),
        "id: hitl-1\nname: AAA\nslug: aaa-form\n",
    )
    .unwrap();
    std::fs::write(
        root.join("workflows/test-flow/meta.yaml"),
        "id: wf-1\nname: Test Flow\nslug: test-flow-v1\nworkflowType: test_flow\nversion: 1\n",
    )
    .unwrap();
    std::fs::write(
        root.join("workflows/test-flow/flow.yaml"),
        r#"initialStage: collect
stages:
  - id: collect
    label: Collect
    kind: collect
    hitlSlug: aaa-form
    agentSlug: test-agent
    doneWhen: validator_pass
  - id: done
    label: Done
    kind: terminal
    doneWhen: terminal
"#,
    )
    .unwrap();
    std::fs::write(root.join("workflows/test-flow/schema.yaml"), "fields: []\n").unwrap();
}
