//! Integration smoke tests against pr-agent-v2 (skipped when fixture absent).

use std::path::PathBuf;

fn pr_agent_v2_root() -> PathBuf {
    PathBuf::from("/Users/roshankgujarathi/Workspace/ResMed/pr-agent-v2")
}

fn resmate_bin() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_resmate"))
}

#[test]
fn smoke_validate_local_zero_errors() {
    let root = pr_agent_v2_root();
    if !root.is_dir() {
        eprintln!("Skipping: pr-agent-v2 not found at {}", root.display());
        return;
    }

    let output = std::process::Command::new(resmate_bin())
        .current_dir(&root)
        .args(["--json", "validate"])
        .output()
        .expect("run resmate validate");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse JSON envelope");
    assert_eq!(envelope["ok"], true);
    assert_eq!(envelope["data"]["summary"]["error_count"], 0);
}

#[test]
fn smoke_workflow_validate_oracle() {
    let root = pr_agent_v2_root();
    if !root.is_dir() {
        eprintln!("Skipping: pr-agent-v2 not found at {}", root.display());
        return;
    }

    let output = std::process::Command::new(resmate_bin())
        .current_dir(&root)
        .args([
            "--json",
            "workflow",
            "validate",
            "oracle-purchase-requisition-v1",
        ])
        .output()
        .expect("run workflow validate");

    assert!(output.status.success());
    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse JSON");
    assert_eq!(envelope["ok"], true);
}

#[test]
fn smoke_push_all_dry_run() {
    let root = pr_agent_v2_root();
    if !root.is_dir() {
        eprintln!("Skipping: pr-agent-v2 not found at {}", root.display());
        return;
    }

    let output = std::process::Command::new(resmate_bin())
        .current_dir(&root)
        .args(["--json", "push-all", "--dry-run"])
        .output()
        .expect("run push-all dry-run");

    assert!(output.status.success());
    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse JSON");
    assert_eq!(envelope["ok"], true);
    assert!(envelope["data"]["steps"].is_array());
}

#[test]
fn smoke_workflow_loader_parity() {
    let root = pr_agent_v2_root();
    if !root.is_dir() {
        eprintln!("Skipping: pr-agent-v2 not found at {}", root.display());
        return;
    }

    let dir = root.join("workflows/oracle-purchase-requisition-v1");
    let validate = resmate::workflow_validate::validate_workflow_dir(&dir).expect("validate");
    let (bundle, _, _) =
        resmate::workflow_loader::load_workflow_from_dir(&dir).expect("load for push");
    assert_eq!(
        bundle
            .get("meta")
            .and_then(|m| m.get("slug"))
            .and_then(|v| v.as_str()),
        Some(validate.slug.as_str())
    );
}
