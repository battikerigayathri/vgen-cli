---
name: Init Phase 1 — ResMate Kit Update
overview: Implement the `resmate kit update` CLI command and MCP `kit_update` tool to safely refresh authoring kit files (skills, rules, docs, AGENTS.md) in existing use-case workspaces without touching live artifacts, supporting dry-run, examples opt-in, and manifest kit_version update.
todos:
  - id: cli-subcommand
    content: "resmed_resmate-cli — Add `kit update` subcommand to CLI with `--examples` and `--dry-run` flags, and `refresh` alias"
    status: pending
  - id: kit-update-logic
    content: "resmed_resmate-cli — Implement run_kit_update command logic in src/commands/kit.rs, reusing/extending copy_workspace_kit with dry_run and force=true"
    status: pending
  - id: manifest-version-update
    content: "resmed_resmate-cli — Implement update_kit_version_in_manifest helper to safely update kit_version in resmate.yaml preserving comments/formatting"
    status: pending
  - id: mcp-kit-update
    content: "resmed_resmate-cli — Implement kit_update tool in MCP (dispatch + handler + tools schema + tests)"
    status: pending
  - id: tests-phase1
    content: "resmed_resmate-cli — Add integration tests in tests/init_authoring_kit_test.rs for kit update (overwrite kit, skip live, examples opt-in, dry-run, version update)"
    status: pending
  - id: docs-phase1
    content: "resmed_resmate-cli — Update agent-authoring-guide.md, QUICKSTART.md, and templates/workspace/.../docs/cli-commands.md to document `resmate kit update`"
    status: pending
isProject: false
---

# Phase 1 — ResMate Kit Update (`resmate kit update`)

**Status:** planned  
**Repo (only):** `resmed_resmate-cli`  
**Parent plan:** `docs/INIT-AUTHORING-KIT-GAPS-PLAN.md` — Phase 1 / Gap #8  
**Do not implement:** Phase 2+ (skill cookbooks, ID lifecycle, SDK/HITL docs, other repos)

---

## Goal

Provide a safe, non-destructive command for authors (and Cursor agents) to refresh template kit files (skills, rules, AGENTS.md, docs) from the current CLI into an already-scaffolded use-case workspace without touching live artifacts or user configurations, and without being blocked by the init emptiness gate.

---

## Scope

| In scope | Out of scope |
|----------|--------------|
| `src/cli/mod.rs`, `src/main.rs`, `src/commands/mod.rs` | Changes in `teemo`, `pr-agent-v2`, `resmedai-core-framework` |
| `src/commands/kit.rs` (new) | Skill content thickening (ID lifecycle, push/wire, SDK, HITL resume) |
| `src/kit/copy.rs` (extend `InitOptions` and `copy_tree` / `copy_workspace_kit`) | Restoring full `platform/` / `cli/` trees into the kit (Phase 4 option) |
| `src/mcp/dispatch.rs`, `handler.rs`, `tools.rs` | Any CLI-side push validation / warning on invented IDs (Phase 5) |
| `tests/init_authoring_kit_test.rs` (new integration tests) | |
| Operator + shipped skill docs that describe `resmate kit update` | |

---

## Product decisions (locked for this plan)

Do not re-open these unless implementation reveals a hard conflict.

| # | Decision | Choice for Phase 1 |
|---|----------|--------------------|
| 1 | Command Name | **`resmate kit update`** with subcommand alias `refresh` (e.g. `resmate kit refresh` works identically). |
| 2 | Default Overwrite | **Overwrite kit-owned files by default** (skills, rules, AGENTS.md, kit README) because the user explicitly asked to update them. This means `force = true` is passed to the copy logic for kit files. |
| 3 | Live Artifacts | **Never delete or overwrite** live artifact trees (`tools/`, `agents/`, `assistants/`, `hitl/`, `workflows/` authored files). This is already guaranteed by `is_live_artifact_path` in `copy.rs`. |
| 4 | Examples Opt-In | **Skip `examples/` by default**; only refresh them if `--examples` flag is passed. This prevents overriding custom example modifications. |
| 5 | Dry-Run | **Support `--dry-run`** to list what files would be written/updated without modifying the filesystem. |
| 6 | Manifest Update | **Update `kit_version` in `resmate.yaml`** to match the current CLI version, preserving comments, formatting, and indentation. |
| 7 | MCP Parity | **Add `kit_update` tool** to MCP with identical semantics, requiring `confirm: true` as it is a mutating tool. |

---

## Current baseline (verified)

- `InitOptions` in `src/kit/copy.rs` only has `name`, `force`, and `no_examples`.
- `copy_tree` directly executes `fs::copy` and `fs::create_dir_all` without dry-run capability.
- `workspace_has_live_artifacts` exists but is only used for `init` refusal.
- No `kit` command or subcommand exists in `src/cli/mod.rs` or `src/main.rs`.
- `init_refused_message` in `src/commands/init.rs` says `resmate kit update` is "coming soon".

---

## Implementation steps

### 1. Extend Copy Options & Dry-Run — `src/kit/copy.rs`

1. Update `InitOptions` to include `dry_run`:
```rust
#[derive(Debug, Clone)]
pub struct InitOptions {
    pub name: String,
    pub force: bool,
    pub no_examples: bool,
    pub dry_run: bool,
}
```
2. Update `copy_tree` signature and implementation to accept and respect `dry_run`:
```rust
fn copy_tree(
    src: &Path,
    dst_root: &Path,
    rel_prefix: &str,
    force: bool,
    no_examples: bool,
    dry_run: bool,
    written: &mut Vec<String>,
) -> Result<(), KitError> {
    // ...
    // Inside the file copy block:
    if !dry_run {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| KitError::Io(e.to_string()))?;
        }
        fs::copy(entry.path(), &target).map_err(|e| KitError::Io(e.to_string()))?;
    }
    written.push(rel);
    // ...
}
```
3. Update `copy_workspace_kit` to forward `options.dry_run` to `copy_tree`.
4. Update `src/commands/init.rs` and `src/commands/scaffold.rs` to supply `dry_run: false` when building `InitOptions`.

### 2. Manifest kit_version Updater — `src/kit/copy.rs` (or `src/manifest.rs`)

Implement a helper to safely update `kit_version` in `resmate.yaml` without parsing/re-serializing (which strips comments and resets formatting):
```rust
pub fn update_kit_version_in_manifest(root: &Path, new_version: &str) -> Result<bool, String> {
    let path = root.join("resmate.yaml");
    if !path.is_file() {
        return Ok(false);
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut updated = false;
    let mut new_lines = Vec::new();
    for line in content.lines() {
        if line.trim_start().starts_with("kit_version:") {
            let indent = line.len() - line.trim_start().len();
            let indent_str = &line[..indent];
            new_lines.push(format!("{}kit_version: \"{}\"", indent_str, new_version));
            updated = true;
        } else {
            new_lines.push(line.to_string());
        }
    }
    if updated {
        let new_content = new_lines.join("\n") + "\n";
        fs::write(&path, new_content).map_err(|e| e.to_string())?;
    }
    Ok(updated)
}
```

### 3. Add CLI Subcommand — `src/cli/mod.rs` & `src/main.rs`

1. Add `Kit(KitCmd)` to the `Commands` enum in `src/cli/mod.rs`.
2. Define `KitCmd` and `KitSubcommand` with `refresh` alias:
```rust
#[derive(Parser)]
pub struct KitCmd {
    #[command(subcommand)]
    pub subcommand: KitSubcommand,
}

#[derive(Subcommand)]
pub enum KitSubcommand {
    /// Refresh the authoring kit files (skills, rules, docs, AGENTS.md) in an existing workspace
    #[command(alias = "refresh")]
    Update {
        /// Also refresh the examples/ reference tree
        #[arg(long)]
        examples: bool,
        /// List paths that would be written without modifying the filesystem
        #[arg(long)]
        dry_run: bool,
    },
}
```
3. Wire dispatch in `src/main.rs`:
```rust
        Commands::Kit(cmd) => match cmd.subcommand {
            KitSubcommand::Update { examples, dry_run } => {
                let ctx = CliContext {
                    mode: OutputMode::from_json_flag(json),
                    command: "kit update",
                };
                return Ok(kit_cmd::run_kit_update(&ctx, examples, dry_run));
            }
        },
```

### 4. Implement Kit Update Command — `src/commands/kit.rs` (new)

1. Declare `pub mod kit;` in `src/commands/mod.rs`.
2. Create `src/commands/kit.rs` implementing `run_kit_update`:
   - Find current working directory (workspace root).
   - Verify it is a ResMate workspace (must contain `resmate.yaml`). If not, return error `NOT_A_WORKSPACE`.
   - Build `InitOptions` with:
     - `name`: dummy or parsed from existing `resmate.yaml`
     - `force`: `true` (overwrite kit files)
     - `no_examples`: `!examples`
     - `dry_run`: `dry_run`
   - Call `copy_workspace_kit(&root, &options)`.
   - If not `dry_run`, call `update_kit_version_in_manifest(&root, kit_version())`.
   - Output success / JSON data matching:
```rust
#[derive(Debug, Serialize)]
pub struct KitUpdateData {
    pub root: String,
    pub files_written: Vec<String>,
    pub kit_version: String,
    pub dry_run: bool,
    pub manifest_updated: bool,
}
```
3. Update `init_refused_message` in `src/commands/init.rs` to remove "(coming soon)".

### 5. Add MCP Parity — `src/mcp/`

1. **`src/mcp/tools.rs`**: Add `kit_update` to `list_tool_definitions()` requiring `confirm: true`. Update `mutating_tools_require_confirm_in_schema` test.
2. **`src/mcp/handler.rs`**: Add match arm for `kit_update` under `handle_tool_call`, parsing `examples`, `dry_run`, and requiring `confirm`.
3. **`src/mcp/dispatch.rs`**: Implement `pub fn tool_kit_update(ctx: &CliContext, examples: bool, dry_run: bool) -> (String, bool)` calling the core kit update logic and returning JSON.

### 6. Integration Tests — `tests/init_authoring_kit_test.rs`

Add integration tests covering:
- `kit_update_requires_manifest`: running `kit update` in an empty directory fails with `NOT_A_WORKSPACE`.
- `kit_update_refreshes_kit_files`: running `init` first, then editing a kit file (e.g. `AGENTS.md`), then running `kit update` restores the kit file.
- `kit_update_does_not_touch_live_artifacts`: running `init`, creating a file `tools/my-tool/tool.yaml`, running `kit update` does not delete or modify that file.
- `kit_update_examples_opt_in`: running `init --no-examples`, then `kit update` does not write examples; running `kit update --examples` writes examples.
- `kit_update_dry_run`: running `kit update --dry-run` lists files but does not actually write them.
- `kit_update_updates_manifest_version`: running `kit update` updates the `kit_version` field in `resmate.yaml`.

---

## Acceptance / exit criteria

- [ ] `resmate kit update` (and `refresh` alias) safely overwrites kit files while preserving live artifacts.
- [ ] `--examples` flag correctly controls example tree refresh.
- [ ] `--dry-run` lists actions without modifying files.
- [ ] `resmate.yaml` `kit_version` is updated, preserving comments and formatting.
- [ ] MCP `kit_update` tool works identically and requires `confirm: true`.
- [ ] Integration tests cover all of the above and pass.
- [ ] Operator and template docs are updated to remove "coming soon" and fully document the command.

---

## Test plan

1. Run `cargo test --test init_authoring_kit_test` to verify all integration tests pass.
2. Run full `cargo test` suite to ensure no regressions.
3. Manual smoke test:
   - Create a temp dir, run `resmate init`.
   - Modify `AGENTS.md` (add some custom text).
   - Create `tools/my-custom-tool/tool.yaml`.
   - Run `resmate kit update --dry-run` -> verify it lists `AGENTS.md` but not `tools/my-custom-tool/tool.yaml`. Verify `AGENTS.md` is unchanged.
   - Run `resmate kit update` -> verify `AGENTS.md` is restored to template state, `tools/my-custom-tool/tool.yaml` is untouched, and `kit_version` in `resmate.yaml` is updated.
   - Run `resmate kit refresh` -> verify alias works.
