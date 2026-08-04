//! Integration tests for `resmate workflow validate`.

use resmate::workflow_validate::validate_workflow_dir;
use std::path::PathBuf;

fn pr_agent_v2_root() -> PathBuf {
    PathBuf::from("/Users/roshankgujarathi/Workspace/ResMed/pr-agent-v2")
}

fn testdata_workflows_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/workflows")
}

#[test]
fn pr_agent_v2_oracle_purchase_requisition_passes() {
    let root = pr_agent_v2_root();
    if !root.is_dir() {
        eprintln!("Skipping: pr-agent-v2 not found at {}", root.display());
        return;
    }

    let dir = root.join("workflows/oracle-purchase-requisition-v1");
    let data = validate_workflow_dir(&dir).expect("oracle-purchase-requisition should validate");
    assert_eq!(data.slug, "oracle-purchase-requisition-v1");
    assert!(data.version >= 3);
    assert_eq!(data.source_format, "split_yaml");
}

#[test]
fn missing_schema_yaml_fails_with_layout_error() {
    let base = std::env::temp_dir().join(format!("resmate-wf-no-schema-{}", uuid::Uuid::new_v4()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(
        base.join("meta.yaml"),
        "slug: x-v1\nname: X\nworkflowType: x\nversion: 1\n",
    )
    .unwrap();
    std::fs::write(base.join("flow.yaml"), "initialStage: a\nstages: []\n").unwrap();

    let err = validate_workflow_dir(&base).expect_err("missing schema.yaml should fail");
    assert_eq!(err.code, "WORKFLOW_LAYOUT_INVALID");
    assert!(
        err.message.contains("schema.yaml"),
        "expected schema.yaml in message, got: {}",
        err.message
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn invalid_next_stage_ref_fails_with_semantic_error() {
    let dir = testdata_workflows_dir().join("invalid-bad-next");
    let err = validate_workflow_dir(&dir).expect_err("bad next ref should fail");
    assert_eq!(err.code, "WORKFLOW_SEMANTIC_INVALID");
    assert!(
        err.message.contains("flow.stages[s1].next"),
        "expected stage path in message, got: {}",
        err.message
    );
    let details = err.details.expect("semantic error should include details");
    assert_eq!(details["path"], "flow.stages[s1].next");
}

#[test]
fn cli_validate_oracle_exits_zero() {
    let root = pr_agent_v2_root();
    if !root.is_dir() {
        eprintln!("Skipping: pr-agent-v2 not found at {}", root.display());
        return;
    }

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_resmate"))
        .args([
            "--json",
            "workflow",
            "validate",
            "oracle-purchase-requisition-v1",
            "--workflows-dir",
        ])
        .arg(root.join("workflows"))
        .output()
        .expect("run resmate workflow validate");

    assert!(
        output.status.success(),
        "expected exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let envelope: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse JSON envelope");
    assert_eq!(envelope["ok"], true);
    assert_eq!(envelope["command"], "workflow validate");
    assert_eq!(envelope["data"]["slug"], "oracle-purchase-requisition-v1");
}
