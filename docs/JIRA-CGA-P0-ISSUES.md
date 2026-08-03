# Jira issue tracker — ResMate CLI P0 (CGA)

Use this document to create issues on the **CGA** Jira project. Each section is copy-paste ready for Jira description fields (Jira markdown: `h2.`, bullets, `{code}` blocks).

**Source plan:** [PHASE1-P0-PLAN.md](./PHASE1-P0-PLAN.md)

---

## Epic

| Field | Value |
|-------|-------|
| **Title** | ResMate CLI P0 — Agent-authoring commands |
| **Issue type** | Epic |
| **Project** | CGA |
| **Labels** | `resmate-cli`, `p0`, `agent-authoring` |

### Description

h2. Summary

Deliver machine-readable CLI commands so IDE agents (Cursor, Codex, etc.) can inspect, validate, and plan pushes for a ResMate use-case workspace without parsing human-oriented CLI text. All new commands support a global `--json` flag with stable exit codes and a structured JSON envelope.

h2. Goals

* Global `--json` output layer with stable exit codes (0 success, 1 runtime, 2 usage, 3 validation)
* Workspace detection and health: `resmate workspace info`, `resmate doctor`
* Dependency link graph: `resmate graph`
* Workspace-level validation: `resmate validate`
* Local workflow schema validation (no API): `resmate workflow validate <folder>`
* Push plan without side effects: `resmate push-all --dry-run`

h2. Reference

* Implementation plan: [PHASE1-P0-PLAN.md](./PHASE1-P0-PLAN.md) (in `resmed_resmate-cli` repo)
* Target repo: `resmed_resmate-cli`
* Reference workspace for smoke tests: `pr-agent-v2` (Oracle PR use case)
* Platform validator: `resmedai-core-framework/lib/smriti_client`

h2. Success criteria (smoke test)

Run from `pr-agent-v2` workspace root:

{code:bash}
# PR1 — envelope + exit codes
resmate --json config show | jq '.ok == true'
resmate --json config validate

# PR2 — workspace
resmate --json workspace info | jq '.data.artifact_dirs.tools.count >= 3'
resmate --json doctor | jq '.data.checks | length >= 3'

# PR3 — graph
resmate --json graph | jq '.data.nodes | map(.kind) | unique'
resmate --json graph | jq '[.data.edges[] | select(.kind=="broken_ref")] | length'  # expect 0

# PR4 — workflow validate
resmate --json workflow validate oracle-purchase-requisition | jq '.ok == true'

# PR5 — full validate
resmate --json validate | jq '.data.summary.error_count == 0'

# PR6 — push plan
resmate --json push-all --dry-run | jq '.data.steps | map(.resource_type)'
# Expect order: hitl → workflow → tool → agent → assistant
{code}

**Agent ergonomics:** An agent can run `validate` + `graph` + `push-all --dry-run` in a loop, fix findings in YAML/handlers, and only call real `push` when `validate` reports zero errors.

h2. Non-goals (P1/P2)

* MCP server wrapping the CLI
* `resmate scaffold` / codegen
* `push-all` execute (batch push with rollback)
* Fix `sync` to pull HITL
* Remote ID existence checks
* Handler static analysis beyond secrets grep

h2. Timeline

* Sequential: ~16–18 person-days (~3–4 weeks, 1 developer)
* With PR4 parallel to PR2–PR3: ~14–16 person-days (~2 weeks, 2 developers)

---

## Story 1 — PR1: JSON output layer + exit codes

| Field | Value |
|-------|-------|
| **Title** | PR1: JSON output layer and exit codes |
| **Issue type** | Story |
| **Project** | CGA |
| **Epic link** | ResMate CLI P0 — Agent-authoring commands |
| **Priority** | Highest |
| **Story points** | 5 |
| **Effort** | 2–3 person-days |
| **Labels** | `resmate-cli`, `p0`, `pr1`, `foundation` |
| **Dependencies** | None (foundation PR — unblocks all others) |

### Description

h2. Scope

Foundation for all agent-facing commands. Introduce global `--json`, structured JSON envelope, and consistent exit codes. No new subcommands beyond wiring `config show` and `config validate` as first JSON consumers.

* Exit `0` — success (including validate with warnings only, dry-run plan)
* Exit `1` — runtime / user error (API failure, I/O, connectivity)
* Exit `2` — usage / invalid input (bad flags, missing required arg)
* Exit `3` — validation failed (reserved for PR4/PR5; document contract now)

JSON mode always prints one envelope object on stdout. Human mode keeps existing `println!` / `eprintln!` behavior for legacy commands.

h2. Key deliverables

* `src/output.rs` — `OutputMode`, `CliContext`, `Envelope`, `emit_success` / `emit_error`
* `src/cli/mod.rs` — global `--json` on `Cli`
* `docs/json-output.md` — envelope schema and exit codes
* `docs/errors/README.md` + taxonomy stubs (`config.md`, etc.)
* `main()` returns `ExitCode`; thread `CliContext` into handlers

### Acceptance criteria

- [ ] `resmate --json config show` prints valid envelope with redacted secrets
- [ ] `resmate --json config validate` returns exit `0` or `1` with `error.code` set
- [ ] Missing subcommand returns exit `2`
- [ ] `docs/json-output.md` documents envelope + exit codes
- [ ] Unit test: serialize/deserialize envelope round-trip

### Technical notes

| Action | Path |
|--------|------|
| Add | `src/output.rs`, `src/cli/mod.rs` |
| Add | `docs/json-output.md`, `docs/errors/README.md`, `docs/errors/*.md` |
| Add | `tests/output_envelope_test.rs` |
| Modify | `src/main.rs`, `src/config.rs`, `src/http_client.rs` |

Proposed envelope shape: `{ ok, command, data, error, warnings }` with `error.code` as stable machine identifier (e.g. `CONFIG_MISSING_API_KEY`).

### Sub-tasks

_None required — single cohesive foundation PR._

---

## Story 2 — PR2: Workspace info and doctor

| Field | Value |
|-------|-------|
| **Title** | PR2: workspace info and doctor commands |
| **Issue type** | Story |
| **Project** | CGA |
| **Epic link** | ResMate CLI P0 — Agent-authoring commands |
| **Priority** | High |
| **Story points** | 3 |
| **Effort** | 2 person-days |
| **Labels** | `resmate-cli`, `p0`, `pr2`, `workspace` |
| **Dependencies** | PR1 (JSON output layer) |

### Description

h2. Scope

Workspace detection and environment diagnostics for ResMate use-case directories.

* `resmate workspace info [--json]` — artifact dirs, counts, env summary, redacted config
* `resmate doctor [--json] [--offline]` — composes workspace + config + connectivity checks

Doctor checks: `workspace_root`, `config_load`, `api_key_set`, `jwt_secret` (warn), `connectivity` (skip with `--offline`), `artifact_dirs_exist` (warn if missing).

Honors `RESMATE_*_DIR` overrides. `Workspace::detect` walks from cwd; `WorkspaceInfo` reports per-dir existence and artifact counts.

### Acceptance criteria

- [ ] Against `pr-agent-v2`: reports 3+ tools, 1 agent, 1 assistant, 1+ hitl, 1 workflow
- [ ] Honors `RESMATE_*_DIR` overrides
- [ ] `doctor --offline` skips connectivity but still returns workspace + config checks
- [ ] JSON envelope matches `docs/json-output.md`

### Technical notes

| Action | Path |
|--------|------|
| Add | `src/workspace.rs`, `src/doctor.rs` |
| Add | `src/commands/workspace.rs`, `src/commands/doctor.rs` |
| Add | `tests/workspace_detect_test.rs` |
| Modify | `src/cli/mod.rs`, `src/main.rs`, `src/config.rs` (`ResolvedPaths`) |

---

## Story 3 — PR3: Graph command

| Field | Value |
|-------|-------|
| **Title** | PR3: dependency link graph command |
| **Issue type** | Story |
| **Project** | CGA |
| **Epic link** | ResMate CLI P0 — Agent-authoring commands |
| **Priority** | High |
| **Story points** | 5 |
| **Effort** | 3 person-days |
| **Labels** | `resmate-cli`, `p0`, `pr3`, `graph` |
| **Dependencies** | PR2 (workspace detection) |

### Description

h2. Scope

Build and emit the assistant → agent → tool → hitl → workflow link graph with broken-ref detection.

* `resmate graph [--json] [--assistant <name>]` — optional filter to one assistant subtree
* Nodes: assistant, agent, tool, hitl, workflow (with `missing_id` flag when platform id empty)
* Edges: `AssistantAgent`, `AgentTool`, `AssistantWorkflow`, `WorkflowHitl`, `WorkflowAgent`, `BrokenRef`
* Orphans: nodes not reachable from any assistant

Move `extract_workflow_slug` from `main.rs` → `specs/assistant.rs`. Add `specs` discovery helpers and `ResourceIndex`.

Resolution: match assistant `agents[]` → agent `id`; agent `skills[]` → tool `id`; `systemContext` `workflow_definition_slug` → workflow `meta.slug`; workflow stage `hitlSlug` / `agentSlug` → hitl/agent slugs.

### Acceptance criteria

- [ ] `pr-agent-v2`: single connected component from `oracle-pr-assistant`
- [ ] Empty `id` in `workflows/oracle-purchase-requisition/meta.yaml` → node `missing_id: true`, not broken ref
- [ ] Inject broken agent id in test fixture → graph shows `broken_ref` edge
- [ ] Orphan tool (not in any agent `skills`) listed in `orphans`

### Technical notes

| Action | Path |
|--------|------|
| Add | `src/graph.rs`, `src/commands/graph.rs` |
| Add | `tests/graph_pr_agent_test.rs` |
| Modify | `src/specs/mod.rs` (discovery helpers, `ResourceIndex`) |
| Move | `extract_workflow_slug` → `specs/assistant.rs` |

---

## Story 4 — PR4: Workflow validate

| Field | Value |
|-------|-------|
| **Title** | PR4: local workflow validate (no API) |
| **Issue type** | Story |
| **Project** | CGA |
| **Epic link** | ResMate CLI P0 — Agent-authoring commands |
| **Priority** | High |
| **Story points** | 5 |
| **Effort** | 2–3 person-days (+1 day monorepo if needed) |
| **Labels** | `resmate-cli`, `p0`, `pr4`, `workflow`, `smriti_client` |
| **Dependencies** | PR1 (JSON output layer). Can parallelize with PR2–PR3 after PR1 merges. |

### Description

h2. Scope

Local workflow bundle validation without API push — schema + semantics identical to platform via `smriti_client`.

* `resmate workflow validate <folder-name> [--json] [--workflows-dir PATH]`
* Uses `WorkflowDefinitionLoader::load_from_dir` (requires `schema.yaml` in split layout)
* Map errors: `WORKFLOW_SCHEMA_INVALID`, `WORKFLOW_SEMANTIC_INVALID`, `WORKFLOW_LAYOUT_INVALID`
* Exit `3` on validation failure; no network calls

**Dependency decision (D1):** Prefer git dep on `smriti_client` with `validate` feature (`default-features = false`). Alternative: vendored schema + copied validate logic (document sync in `docs/workflow-validator-sync.md`).

Note: CLI `specs/workflow.rs` push loader may differ from smriti loader; P0 validate uses smriti only. Align push path in P1.

### Acceptance criteria

- [ ] `pr-agent-v2` `oracle-purchase-requisition` passes
- [ ] Missing `schema.yaml` → exit `3` with clear error path
- [ ] Invalid `next` stage ref → semantic error with `flow.stages[...].next` path
- [ ] No network calls

### Technical notes

| Action | Path |
|--------|------|
| Add | `src/commands/workflow.rs`, `tests/workflow_validate_test.rs` |
| Add | `testdata/workflows/` (minimal invalid fixtures) |
| Modify | `src/cli/mod.rs`, `src/main.rs`, `Cargo.toml` |

Monorepo (if Option A): `lib/smriti_client/Cargo.toml` — add `validate` feature gate.

### Sub-tasks (optional)

| Sub-task | Repo | Description |
|----------|------|-------------|
| **CGA-?-a** | `resmedai-core-framework` | Add `validate` feature to `smriti_client` (`workflow_schema`, `jsonschema`, `serde_yaml`; `default-features = false` for CLI) |
| **CGA-?-b** | `resmed_resmate-cli` | Wire git dep: `smriti_client` with `features = ["validate"]`, pin rev |
| **CGA-?-c** | `resmed_resmate-cli` | Implement `workflow validate` handler + error code mapping |
| **CGA-?-d** | `resmed_resmate-cli` | Integration tests against `oracle-purchase-requisition` + invalid fixtures |

---

## Story 5 — PR5: Workspace validate command

| Field | Value |
|-------|-------|
| **Title** | PR5: aggregate workspace validate command |
| **Issue type** | Story |
| **Project** | CGA |
| **Epic link** | ResMate CLI P0 — Agent-authoring commands |
| **Priority** | High |
| **Story points** | 8 |
| **Effort** | 3–4 person-days |
| **Labels** | `resmate-cli`, `p0`, `pr5`, `validation` |
| **Dependencies** | PR3 (graph), PR4 (workflow validate) |

### Description

h2. Scope

Aggregate workspace validation: graph integrity + per-artifact lint + workflow validate + secrets scan + push readiness.

* `resmate validate [--json] [--strict] [--offline]`
* `--strict`: warnings → exit `3`; default: exit `3` only on errors
* Reuses `graph::build_graph` and workflow validate internally (no duplicate logic)

Rule modules: `graph_rules`, `artifact_rules`, `workflow_rules`, `secrets`, `push_readiness`.

P0 validation codes include: `BROKEN_AGENT_REF`, `BROKEN_TOOL_REF`, `WORKFLOW_SLUG_MISMATCH`, `HITL_SLUG_UNRESOLVED`, `MISSING_HANDLER`, `SECRET_SUSPECTED`, `MISSING_ID`, etc. (full table in PHASE1-P0-PLAN §6).

### Acceptance criteria

- [ ] Runs all P0 rules against `pr-agent-v2` with zero errors
- [ ] Findings include stable `code` strings documented in `docs/errors/`
- [ ] Reuses `graph::build_graph` and workflow validate internally (no duplicate logic)

### Technical notes

| Action | Path |
|--------|------|
| Add | `src/validate/mod.rs`, `graph_rules.rs`, `artifact_rules.rs`, `workflow_rules.rs`, `secrets.rs`, `push_readiness.rs` |
| Add | `src/commands/validate.rs`, `tests/validate_pr_agent_test.rs` |
| Modify | `src/cli/mod.rs`, `src/main.rs` |

---

## Story 6 — PR6: Push-all dry-run

| Field | Value |
|-------|-------|
| **Title** | PR6: push-all dry-run plan |
| **Issue type** | Story |
| **Project** | CGA |
| **Epic link** | ResMate CLI P0 — Agent-authoring commands |
| **Priority** | Medium |
| **Story points** | 3 |
| **Effort** | 2 person-days |
| **Labels** | `resmate-cli`, `p0`, `pr6`, `push-plan` |
| **Dependencies** | PR5 (workspace validate) |

### Description

h2. Scope

Emit ordered push plan as JSON; no API calls, no file writes.

* `resmate push-all --dry-run [--json] [--assistant <name>]`
* Order: **hitl → workflow → tool → agent → assistant** (alphabetical within phase)
* `Create` if `id` missing/empty; `Update` if present
* Attach `blockers` from validation errors touching that resource
* `--assistant` limits plan to subgraph for one assistant (uses graph)

Document push plan schema in `docs/json-output.md#push-plan`. Sync `cli-context/cli/commands-reference.md` in `resmedai-core-framework` on completion.

### Acceptance criteria

- [ ] `pr-agent-v2` plan lists `roc-select-requester-form` hitl before `oracle-purchase-requisition` workflow before tools/agents/assistant
- [ ] No HTTP traffic (mock test — no calls in dry-run)
- [ ] JSON plan matches schema in `docs/json-output.md#push-plan`

### Technical notes

| Action | Path |
|--------|------|
| Add | `src/push_plan.rs`, `src/commands/push_all.rs`, `tests/push_plan_test.rs` |
| Modify | `src/cli/mod.rs`, `src/main.rs`, `README.md` |
| Docs (PR6) | `resmedai-core-framework/cli-context/cli/commands-reference.md` |

---

## PR sequencing diagram

```
PR1 (json) ──┬──> PR2 (workspace) ──> PR3 (graph) ──> PR5 (validate) ──> PR6 (push-all dry-run)
             │
             └──> PR4 (workflow validate) ───────────────────────────────^
```

**Suggested implementation order:** PR1 → (PR2 + PR4 parallel) → PR3 → PR5 → PR6 → docs sync

---

## Bulk create instructions

### 1. GitKraken Jira (MCP / Cursor)

1. Open Jira integration: [Connect Jira in GitKraken](cursor://eamodio.gitlens/link/integrations/connect?id=jira&source=mcp)
2. Complete Atlassian OAuth in the browser
3. Confirm CGA project is visible in the integration settings
4. Create issues manually from sections above, or use GitKraken MCP Jira tools if available in your Cursor session

### 2. Atlassian extension (VS Code / Cursor)

1. Install the **Atlassian** extension if not already present
2. Command Palette (`Cmd+Shift+P`) → **Create Jira Issue**
3. Select project **CGA**
4. Create the Epic first; note the Epic key (e.g. `CGA-123`)
5. Create each Story; set **Epic Link** / parent to the Epic
6. Add optional sub-tasks under PR4 Story for monorepo `smriti_client` work

### 3. Suggested create order

1. Epic — ResMate CLI P0 — Agent-authoring commands
2. Story PR1 (blocks everything)
3. Stories PR2 and PR4 (parallel after PR1)
4. Story PR3
5. Story PR5
6. Story PR6
7. Sub-tasks under PR4 (monorepo feature gate) if splitting work across repos

### 4. Issue title → PR mapping

| Jira issue title | PR | Story points | Depends on |
|------------------|-----|--------------|------------|
| ResMate CLI P0 — Agent-authoring commands | Epic | — | — |
| PR1: JSON output layer and exit codes | PR1 | 5 | — |
| PR2: workspace info and doctor commands | PR2 | 3 | PR1 |
| PR3: dependency link graph command | PR3 | 5 | PR2 |
| PR4: local workflow validate (no API) | PR4 | 5 | PR1 (parallel w/ PR2–3) |
| PR5: aggregate workspace validate command | PR5 | 8 | PR3, PR4 |
| PR6: push-all dry-run plan | PR6 | 3 | PR5 |
| *(sub-task)* smriti_client `validate` feature gate | — | — | PR4 parent |
| **Total (stories)** | | **29 SP** | **~14–18 pd** |

### 5. Labels (apply consistently)

| Label | Use |
|-------|-----|
| `resmate-cli` | All issues |
| `p0` | All issues |
| `agent-authoring` | Epic + all stories |
| `pr1` … `pr6` | Per-story |
| `smriti_client` | PR4 + monorepo sub-task |
| `foundation` | PR1 only |

---

## Open decisions (track in PR4 or Epic)

- [ ] **D1:** Git dependency on `smriti_client` with `validate` feature vs vendored schema
- [ ] **D2:** `doctor` as top-level shortcut (plan: yes, separate from `workspace doctor`)
- [ ] **D3:** Exit code `3` for validation failures (plan: yes, for agent branching)
