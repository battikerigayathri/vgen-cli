//! Integration tests for workspace detection against a real use-case tree.

use vgen::workspace::Workspace;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

fn env_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn pr_agent_v2_root() -> PathBuf {
    PathBuf::from("/Users/roshankgujarathi/Workspace/ResMed/pr-agent-v2")
}

#[test]
fn detect_pr_agent_v2_workspace() {
    let root = pr_agent_v2_root();
    if !root.is_dir() {
        eprintln!("Skipping: pr-agent-v2 not found at {}", root.display());
        return;
    }

    let ws = Workspace::detect(&root).expect("detect pr-agent-v2 workspace");
    assert_eq!(ws.root, root);

    let info = ws.info().expect("workspace info");
    assert!(info.detected, "workspace should be detected");
    assert!(
        info.counts.tools >= 3,
        "expected 3+ tools, got {}",
        info.counts.tools
    );
    assert!(info.counts.agents >= 1, "expected 1+ agents");
    assert!(info.counts.assistants >= 1, "expected 1+ assistants");
    assert!(info.counts.hitl >= 1, "expected 1+ hitl");
    assert!(info.counts.workflows >= 1, "expected 1+ workflows");
}

#[test]
fn detect_pr_agent_v2_from_subdirectory() {
    let root = pr_agent_v2_root();
    if !root.is_dir() {
        eprintln!("Skipping: pr-agent-v2 not found at {}", root.display());
        return;
    }

    let start = root.join("tools");
    if !start.is_dir() {
        eprintln!("Skipping: pr-agent-v2 tools dir missing");
        return;
    }

    let ws = Workspace::detect(&start).expect("detect from tools subdirectory");
    assert_eq!(ws.root, root);
}

#[test]
fn honors_vgen_tools_dir_override_for_pr_agent_v2() {
    let _guard = env_test_lock();
    let root = pr_agent_v2_root();
    if !root.is_dir() {
        eprintln!("Skipping: pr-agent-v2 not found at {}", root.display());
        return;
    }

    let custom_tools = root.join("tools");
    std::env::set_var("VGEN_TOOLS_DIR", &custom_tools);

    let ws = Workspace::detect(&root).expect("detect with tools override");
    assert_eq!(ws.tools_dir, custom_tools);

    let info = ws.info().expect("workspace info with override");
    assert!(info.env.tools_dir.set);
    assert!(info.counts.tools >= 3);

    std::env::remove_var("VGEN_TOOLS_DIR");
}
