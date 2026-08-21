use std::fs;
use std::process::Command;

fn vgen_bin() -> String {
    env!("CARGO_BIN_EXE_vgen").to_string()
}

/// Repo `templates/` dir so tests use this checkout's kit, not an installed copy.
fn repo_templates_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("templates")
}

/// Spawn `vgen` pinned to the repo templates for deterministic init/scaffold.
fn vgen_cmd() -> Command {
    let mut cmd = Command::new(vgen_bin());
    cmd.env("VGEN_TEMPLATES_DIR", repo_templates_dir());
    cmd
}

/// Unique temp dir per test name so parallel tests don't collide.
fn temp_dir(tag: &str) -> std::path::PathBuf {
    let base = std::env::temp_dir().join(format!(
        "vgen-{}-{}-{}",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();
    base
}

/// Lightweight RFC3339 shape check (no chrono dep in the integration test).
/// Verifies a `YYYY-MM-DDT...` prefix with an offset/`Z` suffix.
fn is_rfc3339_shape(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() < 20 {
        return false;
    }
    let digit = |i: usize| bytes.get(i).map(u8::is_ascii_digit).unwrap_or(false);
    let ch = |i: usize, c: u8| bytes.get(i).copied() == Some(c);
    let date_ok = digit(0)
        && digit(1)
        && digit(2)
        && digit(3)
        && ch(4, b'-')
        && digit(5)
        && digit(6)
        && ch(7, b'-')
        && digit(8)
        && digit(9)
        && ch(10, b'T');
    let has_offset = s.ends_with('Z') || s[11..].contains('+') || s[11..].contains('-');
    date_ok && has_offset
}

fn manifest_field(manifest: &str, key: &str) -> Option<String> {
    manifest.lines().find_map(|line| {
        let line = line.trim();
        let rest = line.strip_prefix(key)?.trim_start();
        let rest = rest.strip_prefix(':')?.trim();
        Some(rest.trim_matches('"').to_string())
    })
}

#[test]
fn init_creates_authoring_kit() {
    let base = std::env::temp_dir().join(format!("vgen-init-kit-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();

    let output = vgen_cmd()
        .args(["init", "--name", "test-project", "--json"])
        .current_dir(&base)
        .output()
        .expect("run init");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\": true"));
    assert!(base.join("AGENTS.md").is_file());
    assert!(base
        .join(".cursor/skills/vgen-use-case/SKILL.md")
        .is_file());
    assert!(base
        .join(".cursor/skills/vgen-use-case/docs/decision-matrix.md")
        .is_file());
    assert!(base
        .join(".cursor/skills/vgen-use-case/docs/cli-commands.md")
        .is_file());
    assert!(base.join("tools").is_dir());
    assert!(base.join("agents").is_dir());
    assert!(base.join("assistants").is_dir());
    assert!(base.join("hitl").is_dir());
    assert!(base.join("workflows").is_dir());
    assert!(base.join("vgen.yaml").is_file());

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn init_no_examples_skips_examples_tree() {
    let base = std::env::temp_dir().join(format!("vgen-init-noex-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();

    let output = vgen_cmd()
        .args(["init", "--name", "test-project", "--no-examples", "--json"])
        .current_dir(&base)
        .output()
        .expect("run init");

    assert!(output.status.success());
    assert!(!base.join("examples").exists());

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn scaffold_oracle_pr_after_init() {
    let base = std::env::temp_dir().join(format!("vgen-scaffold-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();

    let init = vgen_cmd()
        .args(["init", "--name", "oracle-pr", "--json"])
        .current_dir(&base)
        .output()
        .expect("run init");
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );

    let output = vgen_cmd()
        .args(["scaffold", "oracle-pr", "--name", "oracle-pr", "--json"])
        .current_dir(&base)
        .output()
        .expect("run scaffold");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(base
        .join("workflows/oracle-purchase-requisition/flow.yaml")
        .is_file());
    assert!(base
        .join("hitl/roc-select-requester-form/meta.yaml")
        .is_file());
    assert!(base.join("agents/oracle-pr-agent.yaml").is_file());

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn init_allows_dir_with_only_git() {
    let base = temp_dir("init-git-only");
    fs::create_dir_all(base.join(".git")).unwrap();
    fs::write(base.join(".gitignore"), "target/\n").unwrap();
    fs::write(base.join("README.md"), "# repo\n").unwrap();

    let output = vgen_cmd()
        .args(["init", "--name", "test-project", "--json"])
        .current_dir(&base)
        .output()
        .expect("run init");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(base.join("vgen.yaml").is_file());

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn init_refuses_dir_with_stray_file() {
    let base = temp_dir("init-stray");
    fs::write(base.join("notes.txt"), "hello\n").unwrap();

    let output = vgen_cmd()
        .args(["init", "--name", "test-project", "--json"])
        .current_dir(&base)
        .output()
        .expect("run init");

    assert!(!output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("INIT_REFUSED"),
        "expected INIT_REFUSED, got {combined}"
    );
    assert!(
        combined.contains("notes.txt"),
        "expected offending path listed, got {combined}"
    );
    assert!(!base.join("vgen.yaml").exists());

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn init_force_overrides_stray_file() {
    let base = temp_dir("init-force");
    fs::write(base.join("notes.txt"), "hello\n").unwrap();

    let output = vgen_cmd()
        .args(["init", "--name", "test-project", "--force", "--json"])
        .current_dir(&base)
        .output()
        .expect("run init");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(base.join("AGENTS.md").is_file());
    assert!(base.join("vgen.yaml").is_file());
    // --force never deletes pre-existing non-kit files.
    assert!(base.join("notes.txt").is_file());

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn init_writes_rfc3339_timestamp() {
    let base = temp_dir("init-rfc3339");

    let output = vgen_cmd()
        .args(["init", "--name", "test-project", "--json"])
        .current_dir(&base)
        .output()
        .expect("run init");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let manifest = fs::read_to_string(base.join("vgen.yaml")).unwrap();
    let initialized_at = manifest_field(&manifest, "initialized_at")
        .expect("initialized_at present in vgen.yaml");
    assert!(
        !initialized_at.chars().all(|c| c.is_ascii_digit()),
        "initialized_at must not be pure unix seconds, got {initialized_at}"
    );
    // RFC3339 shape: 2026-07-17T09:38:00+00:00 / ...Z
    assert!(
        is_rfc3339_shape(&initialized_at),
        "initialized_at must be RFC3339, got {initialized_at}"
    );

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn init_default_description() {
    let base = temp_dir("init-desc-default");

    let output = vgen_cmd()
        .args(["init", "--name", "test-project", "--json"])
        .current_dir(&base)
        .output()
        .expect("run init");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let manifest = fs::read_to_string(base.join("vgen.yaml")).unwrap();
    assert_eq!(
        manifest_field(&manifest, "description").as_deref(),
        Some("ResMate use case workspace")
    );

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn init_custom_description() {
    let base = temp_dir("init-desc-custom");

    let output = vgen_cmd()
        .args([
            "init",
            "--name",
            "test-project",
            "--description",
            "My app",
            "--json",
        ])
        .current_dir(&base)
        .output()
        .expect("run init");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let manifest = fs::read_to_string(base.join("vgen.yaml")).unwrap();
    assert_eq!(
        manifest_field(&manifest, "description").as_deref(),
        Some("My app")
    );

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn kit_update_requires_manifest() {
    let base = temp_dir("kit-update-nomanifest");

    let output = vgen_cmd()
        .args(["kit", "update", "--json"])
        .current_dir(&base)
        .output()
        .expect("run kit update");

    assert!(!output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("NOT_A_WORKSPACE"),
        "expected NOT_A_WORKSPACE, got {combined}"
    );

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn kit_update_refreshes_kit_files() {
    let base = temp_dir("kit-update-refresh");

    let init = vgen_cmd()
        .args(["init", "--name", "test-project", "--json"])
        .current_dir(&base)
        .output()
        .expect("run init");
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );

    let agents_path = base.join("AGENTS.md");
    fs::write(&agents_path, "custom edits that should be restored\n").unwrap();

    let output = vgen_cmd()
        .args(["kit", "update", "--json"])
        .current_dir(&base)
        .output()
        .expect("run kit update");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let restored = fs::read_to_string(&agents_path).unwrap();
    assert!(
        !restored.contains("custom edits that should be restored"),
        "expected AGENTS.md to be restored from kit template, got: {restored}"
    );

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn kit_update_does_not_touch_live_artifacts() {
    let base = temp_dir("kit-update-live");

    let init = vgen_cmd()
        .args(["init", "--name", "test-project", "--json"])
        .current_dir(&base)
        .output()
        .expect("run init");
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );

    let live_tool = base.join("tools/my-tool/tool.yaml");
    fs::create_dir_all(live_tool.parent().unwrap()).unwrap();
    fs::write(&live_tool, "name: my-tool\ncustom: true\n").unwrap();

    let output = vgen_cmd()
        .args(["kit", "update", "--json"])
        .current_dir(&base)
        .output()
        .expect("run kit update");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(live_tool.is_file(), "live artifact must not be deleted");
    let content = fs::read_to_string(&live_tool).unwrap();
    assert_eq!(content, "name: my-tool\ncustom: true\n");

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn kit_update_examples_opt_in() {
    let base = temp_dir("kit-update-examples");

    let init = vgen_cmd()
        .args(["init", "--name", "test-project", "--no-examples", "--json"])
        .current_dir(&base)
        .output()
        .expect("run init");
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    assert!(!base.join("examples").exists());

    let no_examples_update = vgen_cmd()
        .args(["kit", "update", "--json"])
        .current_dir(&base)
        .output()
        .expect("run kit update");
    assert!(
        no_examples_update.status.success(),
        "{}",
        String::from_utf8_lossy(&no_examples_update.stderr)
    );
    assert!(
        !base.join("examples").exists(),
        "kit update without --examples must not write examples/"
    );

    let with_examples_update = vgen_cmd()
        .args(["kit", "update", "--examples", "--json"])
        .current_dir(&base)
        .output()
        .expect("run kit update --examples");
    assert!(
        with_examples_update.status.success(),
        "{}",
        String::from_utf8_lossy(&with_examples_update.stderr)
    );
    assert!(
        base.join("examples").exists(),
        "kit update --examples must write examples/"
    );

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn kit_update_dry_run() {
    let base = temp_dir("kit-update-dryrun");

    let init = vgen_cmd()
        .args(["init", "--name", "test-project", "--json"])
        .current_dir(&base)
        .output()
        .expect("run init");
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );

    let agents_path = base.join("AGENTS.md");
    fs::write(&agents_path, "custom edits that must survive dry-run\n").unwrap();
    let manifest_before = fs::read_to_string(base.join("vgen.yaml")).unwrap();

    let output = vgen_cmd()
        .args(["kit", "update", "--dry-run", "--json"])
        .current_dir(&base)
        .output()
        .expect("run kit update --dry-run");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("AGENTS.md"),
        "expected AGENTS.md to be listed, got {stdout}"
    );
    assert!(stdout.contains("\"dry_run\": true"));

    let unchanged = fs::read_to_string(&agents_path).unwrap();
    assert_eq!(
        unchanged, "custom edits that must survive dry-run\n",
        "dry-run must not modify the filesystem"
    );
    let manifest_after = fs::read_to_string(base.join("vgen.yaml")).unwrap();
    assert_eq!(
        manifest_before, manifest_after,
        "dry-run must not update vgen.yaml"
    );

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn kit_update_updates_manifest_version() {
    let base = temp_dir("kit-update-version");

    let init = vgen_cmd()
        .args(["init", "--name", "test-project", "--json"])
        .current_dir(&base)
        .output()
        .expect("run init");
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );

    let manifest_path = base.join("vgen.yaml");
    let manifest = fs::read_to_string(&manifest_path).unwrap();
    let original_version = manifest_field(&manifest, "kit_version");
    assert!(
        original_version.is_some(),
        "kit_version must be present after init"
    );

    // Corrupt the recorded kit_version so we can observe kit update repairing it,
    // while keeping every other line (comments/formatting) intact.
    let tampered = manifest.replace(
        &format!("kit_version: \"{}\"", original_version.unwrap()),
        "kit_version: \"0.0.0-stale\"",
    );
    fs::write(&manifest_path, &tampered).unwrap();

    let output = vgen_cmd()
        .args(["kit", "update", "--json"])
        .current_dir(&base)
        .output()
        .expect("run kit update");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"manifest_updated\": true"));

    let updated_manifest = fs::read_to_string(&manifest_path).unwrap();
    let updated_version = manifest_field(&updated_manifest, "kit_version");
    assert_ne!(updated_version.as_deref(), Some("0.0.0-stale"));
    // Other manifest fields (e.g. name) must be preserved verbatim.
    assert_eq!(
        manifest_field(&updated_manifest, "name").as_deref(),
        Some("test-project")
    );

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn kit_refresh_alias_works() {
    let base = temp_dir("kit-refresh-alias");

    let init = vgen_cmd()
        .args(["init", "--name", "test-project", "--json"])
        .current_dir(&base)
        .output()
        .expect("run init");
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );

    let output = vgen_cmd()
        .args(["kit", "refresh", "--json"])
        .current_dir(&base)
        .output()
        .expect("run kit refresh");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\": true"));

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn scaffold_without_kit_requires_with_kit() {
    let base = std::env::temp_dir().join(format!("vgen-scaffold-nokit-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();

    let output = vgen_cmd()
        .args(["scaffold", "oracle-pr", "--name", "oracle-pr", "--json"])
        .current_dir(&base)
        .output()
        .expect("run scaffold");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stderr.contains("KIT_NOT_PRESENT") || stdout.contains("KIT_NOT_PRESENT"),
        "expected KIT_NOT_PRESENT, got stderr={stderr} stdout={stdout}"
    );

    let _ = fs::remove_dir_all(&base);
}
