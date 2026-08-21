//! Integration tests for aggregate workspace validation against pr-agent-v2.

use vgen::validate::run_workspace_validation;
use vgen::workspace::Workspace;
use std::path::PathBuf;

fn pr_agent_v2_root() -> PathBuf {
    PathBuf::from("/Users/roshankgujarathi/Workspace/ResMed/pr-agent-v2")
}

#[test]
fn pr_agent_v2_validate_has_zero_errors() {
    let root = pr_agent_v2_root();
    if !root.is_dir() {
        eprintln!("Skipping: pr-agent-v2 not found at {}", root.display());
        return;
    }

    let ws = Workspace::detect(&root).expect("detect pr-agent-v2");
    let report = run_workspace_validation(&ws, Default::default()).expect("validate workspace");

    let errors: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.severity == vgen::validate::Severity::Error)
        .collect();

    assert_eq!(
        report.summary.error_count,
        0,
        "expected zero validation errors; got {}: {:?}",
        errors.len(),
        errors
            .iter()
            .map(|f| format!("{}: {}", f.code, f.message))
            .collect::<Vec<_>>()
    );
}

#[test]
fn pr_agent_v2_validate_finding_codes_are_stable() {
    let root = pr_agent_v2_root();
    if !root.is_dir() {
        eprintln!("Skipping: pr-agent-v2 not found at {}", root.display());
        return;
    }

    let ws = Workspace::detect(&root).expect("detect pr-agent-v2");
    let report = run_workspace_validation(&ws, Default::default()).expect("validate workspace");

    for finding in &report.findings {
        assert!(
            !finding.code.is_empty(),
            "finding code must be non-empty: {:?}",
            finding
        );
        assert!(
            finding
                .code
                .chars()
                .all(|c| c.is_ascii_uppercase() || c == '_'),
            "finding code should be SCREAMING_SNAKE_CASE: {}",
            finding.code
        );
    }
}
