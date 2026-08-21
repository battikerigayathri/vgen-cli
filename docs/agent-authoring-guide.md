# ResMate agent authoring guide

**Canonical short guide** for bootstrapping and publishing ResMate use cases with the CLI. Platform detail lives in your workspace kit after `vgen init` — this document links there instead of duplicating it.

**Jira:** [CGA-1110](https://resmedglobal.atlassian.net/browse/CGA-1110) · **Phase:** [PHASE3A-P2-PLAN.md](./PHASE3A-P2-PLAN.md) · **Architecture:** [AUTHORING-KIT-ARCHITECTURE.md](./AUTHORING-KIT-ARCHITECTURE.md)

---

## Prerequisites

- **ResMate CLI** — `cargo install --path .` from [`resmed_vgen-cli`](../.) (or a release binary on your PATH).
- **API credentials** — `.env` at workspace root with at least `VGEN_API_KEY` (and `VGEN_SECRET` when using JWT). See [cli/setup.md](../templates/workspace/cli/setup.md) in the kit.

---

## Quick start: init → scaffold → validate → push

```bash
mkdir my-assistant && cd my-assistant
vgen init --name my-assistant          # copies authoring kit + vgen.yaml
vgen scaffold oracle-pr --name my-assistant   # optional recipe starter
# edit tools/, agents/, assistants/, hitl/, workflows/ at workspace root
vgen doctor
vgen graph
vgen workflow validate <name>          # when workflow-bound
vgen validate
vgen push-all --dry-run
vgen push-all --yes
```

| Step | Command | Purpose |
|------|---------|---------|
| Bootstrap | `vgen init [--name <name>] [--description <text>] [--no-examples] [--force]` | Copy kit from `templates/workspace/`; write `vgen.yaml` with `kit_version` and RFC3339 `initialized_at` |
| Refresh kit | `vgen kit update [--examples] [--dry-run]` (alias: `vgen kit refresh`) | Refresh skills/rules/docs/`AGENTS.md` in an existing workspace; never touches live artifacts or `examples/` unless `--examples` is passed; updates `kit_version` in `vgen.yaml` |
| Recipe | `vgen scaffold <recipe> [--name <name>]` | Add starter artifacts (`minimal`, `form-wizard`, `oracle-pr`) |
| Inspect | `vgen workspace info`, `vgen graph` | Layout, counts, dependency graph |
| Validate | `vgen doctor` → `graph` → `workflow validate` → `validate` | Fix errors before push |
| Plan | `vgen push-all --dry-run` | Preview push order |
| Publish | `vgen push-all --yes` | Push in dependency order |

**Push order:** **hitl → workflow → tool → agent → assistant** (skip steps that do not apply).

Add `--json` to any command for machine-readable output (global flag).

### `vgen init` requirements

- **Init-safe directory:** the target must be **empty or contain only allowlisted top-level entries** — `.git`, `.gitignore`, `README.md`, `.DS_Store`. Any other file or directory (including empty live-artifact dirs like `tools/`) makes init refuse with `INIT_REFUSED`, listing the offending entries.
- **`--force`:** bootstrap-overwrites kit and seed files in a non-init-safe directory. It **never deletes live artifacts** (`tools/<name>/`, `agents/*.yaml`, etc.). To refresh the kit in an existing use-case repo, prefer `vgen kit update` instead.
- **`--description <text>`:** written to `vgen.yaml`. Defaults to `ResMate use case workspace` when omitted.
- **`initialized_at`:** recorded as an RFC3339 timestamp (e.g. `2026-07-17T09:38:00+00:00`).

### `vgen kit update` (refresh an existing workspace)

Unlike `init`, `kit update` is **not** gated by the init-safety check — it works on an already-scaffolded workspace (any directory containing `vgen.yaml`).

- Overwrites kit-owned files (skills, rules, `AGENTS.md`, kit README, docs) with the current CLI's templates. Equivalent to `init --force` scoped to kit files only.
- **Never touches live artifacts** (`tools/`, `agents/`, `assistants/`, `hitl/`, `workflows/` authored files) — same `is_live_artifact_path` guard used by `init`.
- **Skips `examples/` by default**; pass `--examples` to also refresh the reference tree.
- **`--dry-run`:** lists the files that would be written without touching the filesystem or `vgen.yaml`.
- Updates `kit_version` in `vgen.yaml` in place (line-based rewrite that preserves comments, formatting, and key order) unless `--dry-run` is set.
- Alias: `vgen kit refresh` behaves identically.
- Fails with `NOT_A_WORKSPACE` if `vgen.yaml` is not found in the current directory.

```bash
vgen kit update --dry-run   # preview what would change
vgen kit update             # refresh kit files + kit_version
vgen kit update --examples  # also refresh examples/
vgen kit refresh            # alias
```

---

## Workspace layout (after init)

| Path | Role |
|------|------|
| **Workspace root** | Run all `vgen` commands here |
| `tools/`, `agents/`, `assistants/`, `hitl/`, `workflows/` | **Live artifacts** — develop and push from here |
| `platform/`, `cli/`, `workflows/*.md`, `tools/*.md`, … | **Kit docs** — guides and YAML references |
| `examples/` | **Reference only** — copy patterns; do not develop here |
| `AGENTS.md`, `.cursor/skills/` | Cursor agent entry point and skill |

"Project root" means **workspace root**, not the platform monorepo.

---

## Validate loop (before every push)

From workspace root:

1. `vgen doctor` — environment and connectivity (`--offline` skips API check)
2. `vgen graph` — no `broken_ref` edges; optional `--assistant <name>`
3. `vgen workflow validate <folder>` — for each workflow-bound definition
4. `vgen validate` — aggregate check; use `--strict` to fail on warnings
5. `vgen push-all --dry-run` — confirm step order and resources

Full checklist: [cli/authoring-checklist.md](../templates/workspace/cli/authoring-checklist.md) (in your workspace after init: `cli/authoring-checklist.md`).

---

## Kit docs (read after init)

Open your workspace in the IDE and follow this order:

1. [README.md](../templates/workspace/README.md) — layout and index
2. [AGENTS.md](../templates/workspace/AGENTS.md) — agent read order
3. [workflows/README.md](../templates/workspace/workflows/README.md) — pick a recipe
4. [platform/execution-model.md](../templates/workspace/platform/execution-model.md) + [platform/state-and-workflows.md](../templates/workspace/platform/state-and-workflows.md)
5. Layer refs: [tools/](../templates/workspace/tools/), [agents/](../templates/workspace/agents/), [assistants/](../templates/workspace/assistants/), [hitl/](../templates/workspace/hitl/)
6. CLI surface: [cli/commands-reference.md](../templates/workspace/cli/commands-reference.md)

Reference samples: [examples/README.md](../templates/workspace/examples/README.md).

---

## MCP (IDE agents)

`vgen-mcp` exposes inspect, validate, plan, and push tools over stdio. Configure in Cursor and pass `workspace_root` to target your init'd folder.

See [mcp-setup.md](./mcp-setup.md).

---

## Maintainer sync

Kit content is vendored at `templates/workspace/` from `resmedai-core-framework/cli-context/`. Run `scripts/sync-authoring-kit.sh` after platform doc changes.

---

## Legacy docs

- [CLAD-vgen-use-cases.md](./CLAD-vgen-use-cases.md) — deprecated; use this guide + workspace kit
- Manual copy of `cli-context/` — replaced by `vgen init`; see [MIGRATION.md](../../resmedai-core-framework/cli-context/MIGRATION.md) in the platform repo
