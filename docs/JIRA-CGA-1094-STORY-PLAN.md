# Jira Story Plan — CGA-1094 ResMate CLI Stabilisation

**Epic:** [CGA-1094](https://resmedglobal.atlassian.net/browse/CGA-1094) — ResMate CLI Stabilisation – Agent & IDE-Friendly Enhancements  
**Project:** CGA (COE Gen AI)  
**Board:** Envisioning (`customfield_11586` = Envisioning)  
**Plan date:** 2026-06-30

## Epic scope alignment

The epic targets machine-parsable CLI output, local validation/graph tooling, batch push planning, workspace scaffolding, and eventual MCP integration for IDE agents. This plan maps **three implementation phases** to the epic:

| Phase | Epic mapping | Focus |
|-------|--------------|-------|
| **Phase 1 (P0)** | Epic P0 items + foundation | `--json`, workspace/doctor, graph, validate, workflow validate, push-all dry-run |
| **Phase 2 (P1)** | Epic P1 + strategic prep | MCP server, scaffold/init, push-all execute, sync fix, explain errors, cli-context sync |
| **Phase 3 (P2)** | Epic P2 + hardening | Remote checks, static analysis, diff, loader alignment, CLAD docs, CI integration tests |

**Source plans:**
- Phase 1 detail: [PHASE1-P0-PLAN.md](./PHASE1-P0-PLAN.md)
- Phase 1 templates: [JIRA-CGA-P0-ISSUES.md](./JIRA-CGA-P0-ISSUES.md)
- Target repo: `resmed_resmate-cli`
- Reference workspace: `pr-agent-v2` (Oracle PR)
- Platform validator: `resmedai-core-framework/lib/smriti_client`

### Epic vs detailed plan note

The epic description groups `workflow validate` and `push-all --dry-run` under P1; the approved implementation plan places them in **Phase 1 (PR4, PR6)** because agents need full inspect→validate→plan loop before batch execute. Stories below follow the detailed plan.

---

## Dependency graph

```
Phase 1:
  PR1 ──┬──> PR2 ──> PR3 ──> PR5 ──> PR6
        └──> PR4 ───────────────^

Phase 2 (after Phase 1):
  PR1/PR6 ──> P2-3 push-all execute
  PR1 ──────> P2-1 MCP, P2-2 scaffold, P2-5 explain
  PR6 ──────> P2-1 MCP (full tool surface)
  (independent) P2-4 sync HITL fix
  P2-* ─────> P2-6 cli-context update (docs)

Phase 3 (after Phase 2):
  PR4 ──> P3-4 workflow push alignment
  PR5/PR6 ──> P3-1 remote ID, P3-2 static analysis, P3-3 diff
  Phase 1-2 ──> P3-5 CLAD doc, P3-6 CI tests
```

---

## Phase 1 stories (6) — CLI P0

### Story P1-1: PR1 — JSON output layer and exit codes

| Field | Value |
|-------|-------|
| **Story points** | 5 |
| **Labels** | `resmate-cli`, `resmate`, `agent-authoring`, `phase-1`, `pr1`, `foundation` |
| **Depends on** | — (foundation) |
| **Repo** | `resmed_resmate-cli` |

**Description**

Foundation for all agent-facing commands. Introduce global `--json`, structured JSON envelope, and consistent exit codes. Wire `config show` and `config validate` as first JSON consumers.

Exit codes: `0` success · `1` runtime · `2` usage · `3` validation (reserved for PR4/PR5).

**Acceptance criteria**

- [ ] `resmate --json config show` prints valid envelope with redacted secrets
- [ ] `resmate --json config validate` returns exit `0` or `1` with `error.code` set
- [ ] Missing subcommand returns exit `2`
- [ ] `docs/json-output.md` documents envelope + exit codes
- [ ] Unit test: envelope serialize/deserialize round-trip

**Technical notes**

Add `src/output.rs`, `src/cli/mod.rs`, `docs/json-output.md`, `docs/errors/*`, `tests/output_envelope_test.rs`. Modify `main.rs`, `config.rs`, `http_client.rs`. See [PHASE1-P0-PLAN.md § PR1](./PHASE1-P0-PLAN.md).

---

### Story P1-2: PR2 — Workspace info and doctor commands

| Field | Value |
|-------|-------|
| **Story points** | 3 |
| **Labels** | `resmate-cli`, `resmate`, `agent-authoring`, `phase-1`, `pr2`, `workspace` |
| **Depends on** | P1-1 |
| **Repo** | `resmed_resmate-cli` |

**Description**

Workspace detection and environment diagnostics: `resmate workspace info` and `resmate doctor [--offline]`. Honors `RESMATE_*_DIR` overrides. Doctor checks: workspace_root, config_load, api_key_set, jwt_secret (warn), connectivity (skip offline), artifact_dirs_exist.

**Acceptance criteria**

- [ ] Against `pr-agent-v2`: 3+ tools, 1 agent, 1 assistant, 1+ hitl, 1 workflow
- [ ] Honors `RESMATE_*_DIR` overrides
- [ ] `doctor --offline` skips connectivity, returns workspace + config checks
- [ ] JSON envelope matches `docs/json-output.md`

**Technical notes**

Add `src/workspace.rs`, `src/doctor.rs`, `src/commands/workspace.rs`, `src/commands/doctor.rs`. See [PHASE1-P0-PLAN.md § PR2](./PHASE1-P0-PLAN.md).

---

### Story P1-3: PR3 — Dependency link graph command

| Field | Value |
|-------|-------|
| **Story points** | 5 |
| **Labels** | `resmate-cli`, `resmate`, `agent-authoring`, `phase-1`, `pr3`, `graph` |
| **Depends on** | P1-2 |
| **Repo** | `resmed_resmate-cli` |

**Description**

`resmate graph [--json] [--assistant <name>]` — assistant → agent → tool → hitl → workflow graph with broken-ref detection and orphan listing. Move `extract_workflow_slug` to `specs/assistant.rs`; add discovery helpers and `ResourceIndex`.

**Acceptance criteria**

- [ ] `pr-agent-v2`: connected component from `oracle-pr-assistant`
- [ ] Empty workflow `id` → `missing_id: true`, not broken ref
- [ ] Broken agent id fixture → `broken_ref` edge
- [ ] Orphan tools listed in `orphans`

**Technical notes**

Add `src/graph.rs`, `src/commands/graph.rs`, `tests/graph_pr_agent_test.rs`. See [PHASE1-P0-PLAN.md § PR3](./PHASE1-P0-PLAN.md).

---

### Story P1-4: PR4 — Local workflow validate (no API)

| Field | Value |
|-------|-------|
| **Story points** | 5 |
| **Labels** | `resmate-cli`, `resmate`, `agent-authoring`, `phase-1`, `pr4`, `workflow`, `smriti_client` |
| **Depends on** | P1-1 (parallel with P1-2, P1-3 after P1-1) |
| **Repo** | `resmed_resmate-cli`, `resmedai-core-framework` (smriti feature gate) |

**Description**

`resmate workflow validate <folder> [--json]` — local schema + semantic validation via `smriti_client` `WorkflowDefinitionLoader`. Exit `3` on failure; no network. Prefer git dep with `validate` feature (`default-features = false`).

**Acceptance criteria**

- [ ] `pr-agent-v2` `oracle-purchase-requisition` passes
- [ ] Missing `schema.yaml` → exit `3` with clear path
- [ ] Invalid `next` stage ref → semantic error with stage path
- [ ] No network calls

**Technical notes**

May require monorepo PR: `smriti_client` `validate` feature gate. See [PHASE1-P0-PLAN.md § PR4](./PHASE1-P0-PLAN.md). Note: CLI push loader may differ from smriti loader — align in Phase 3.

---

### Story P1-5: PR5 — Aggregate workspace validate command

| Field | Value |
|-------|-------|
| **Story points** | 8 |
| **Labels** | `resmate-cli`, `resmate`, `agent-authoring`, `phase-1`, `pr5`, `validation` |
| **Depends on** | P1-3, P1-4 |
| **Repo** | `resmed_resmate-cli` |

**Description**

`resmate validate [--json] [--strict] [--offline]` — graph integrity, artifact lint, workflow validate, secrets grep, push readiness. Reuses `graph::build_graph` and workflow validate internally.

**Acceptance criteria**

- [ ] All P0 rules pass on `pr-agent-v2` with zero errors
- [ ] Findings have stable `code` strings in `docs/errors/`
- [ ] No duplicate validation logic vs graph/workflow validate

**Technical notes**

Add `src/validate/*`, `src/commands/validate.rs`. Full rule table: [PHASE1-P0-PLAN.md §6](./PHASE1-P0-PLAN.md).

---

### Story P1-6: PR6 — Push-all dry-run plan

| Field | Value |
|-------|-------|
| **Story points** | 3 |
| **Labels** | `resmate-cli`, `resmate`, `agent-authoring`, `phase-1`, `pr6`, `push-plan` |
| **Depends on** | P1-5 |
| **Repo** | `resmed_resmate-cli`, `resmedai-core-framework` (cli-context) |

**Description**

`resmate push-all --dry-run [--json] [--assistant <name>]` — ordered push plan (hitl → workflow → tool → agent → assistant), Create/Update detection, validation blockers. No HTTP traffic.

**Acceptance criteria**

- [ ] `pr-agent-v2` plan: hitl before workflow before tools/agents/assistant
- [ ] No HTTP in dry-run (test/mock verified)
- [ ] JSON plan matches `docs/json-output.md#push-plan`
- [ ] `cli-context/cli/commands-reference.md` updated with new commands

**Technical notes**

Add `src/push_plan.rs`, `src/commands/push_all.rs`. See [PHASE1-P0-PLAN.md § PR6](./PHASE1-P0-PLAN.md).

**Phase 1 total:** 29 SP · ~14–18 person-days

---

## Phase 2 stories (6) — CLI P1

### Story P2-1: MCP server wrapping ResMate CLI

| Field | Value |
|-------|-------|
| **Story points** | 8 |
| **Labels** | `resmate-cli`, `resmate`, `agent-authoring`, `phase-2`, `mcp` |
| **Depends on** | P1-1 (full surface: P1-6) |
| **Repo** | `resmed_resmate-cli` |

**Description**

Expose ResMate CLI commands as MCP tools, reusing the `--json` contract from Phase 1. Enables native Cursor/IDE integration without shell text parsing — inline diagnostics, graph data, validate findings, push-plan preview.

**Acceptance criteria**

- [ ] MCP server starts and advertises tools for workspace, doctor, graph, validate, workflow validate, push-all dry-run
- [ ] Each tool returns parsed JSON envelope (not raw stdout strings)
- [ ] Document setup in `resmed_resmate-cli` README and `cli-context`
- [ ] Smoke test: MCP tool call against `pr-agent-v2` validate returns zero errors

**Technical notes**

Depends on stable JSON contracts (Phase 1). Consider stdio MCP transport first; align tool names with cli-context agent skill conventions.

---

### Story P2-2: resmate init / scaffold + resmate.yaml manifest

| Field | Value |
|-------|-------|
| **Story points** | 5 |
| **Labels** | `resmate-cli`, `resmate`, `agent-authoring`, `phase-2`, `scaffold` |
| **Depends on** | P1-1 |
| **Repo** | `resmed_resmate-cli` |

**Description**

`resmate init` and `resmate scaffold <recipe>` generate standard workspace structure. Optional `resmate.yaml` manifest defining artifact paths, recipes, and defaults (replaces implicit env-var-only discovery over time).

**Acceptance criteria**

- [ ] `resmate init` creates tools/, agents/, assistants/, hitl/, workflows/ skeleton
- [ ] `resmate scaffold oracle-pr` (or similar) produces Oracle PR starter layout
- [ ] `resmate.yaml` schema documented; workspace info reads manifest when present
- [ ] JSON output for init/scaffold commands

**Technical notes**

Align with `cli-context/examples/oracle-purchase-requisition/` patterns. Manifest is optional — env overrides still work.

---

### Story P2-3: push-all execute (batch push with rollback)

| Field | Value |
|-------|-------|
| **Story points** | 8 |
| **Labels** | `resmate-cli`, `resmate`, `agent-authoring`, `phase-2`, `push-plan` |
| **Depends on** | P1-6 |
| **Repo** | `resmed_resmate-cli` |

**Description**

Execute the ordered push plan from `push-all` (not just `--dry-run`). Support confirmation gate, per-step reporting, and best-effort rollback or failure stop with structured error output.

**Acceptance criteria**

- [ ] `resmate push-all` (without `--dry-run`) executes hitl → workflow → tool → agent → assistant order
- [ ] Aborts on validation blockers unless `--force` (document risks)
- [ ] Each step emits JSON progress; failed step returns stable error code
- [ ] ID write-back matches existing per-resource push behavior (Appendix A in P0 plan)

**Technical notes**

Reuse `build_push_plan` from P1-6. Requires authenticated API. Consider `--assistant` subgraph limit.

---

### Story P2-4: Fix resmate sync to pull HITL

| Field | Value |
|-------|-------|
| **Story points** | 3 |
| **Labels** | `resmate-cli`, `resmate`, `agent-authoring`, `phase-2`, `sync`, `bugfix` |
| **Depends on** | — (can parallel Phase 1) |
| **Repo** | `resmed_resmate-cli` |

**Description**

Documentation states `resmate sync` pulls HITL artifacts; implementation does not. Align code with docs (or update docs if intentional — prefer fixing code).

**Acceptance criteria**

- [ ] `resmate sync` pulls HITL definitions when IDs exist remotely
- [ ] `doctor` / `workspace info` no longer needs sync mismatch warning
- [ ] cli-context `push-pull-sync.md` matches behavior
- [ ] Test: sync round-trip for HITL fixture

**Technical notes**

Known doc/code mismatch flagged in P0 plan risks table. `extract_workflow_slug` in sync path may need HITL branch similar to workflow.

---

### Story P2-5: resmate explain error codes

| Field | Value |
|-------|-------|
| **Story points** | 3 |
| **Labels** | `resmate-cli`, `resmate`, `agent-authoring`, `phase-2`, `errors` |
| **Depends on** | P1-1 |
| **Repo** | `resmed_resmate-cli` |

**Description**

`resmate explain <error-code>` — human and JSON explanation of stable error codes from `docs/errors/`. Helps agents and developers remediate without reading source.

**Acceptance criteria**

- [ ] `resmate explain BROKEN_AGENT_REF` prints description, severity, remediation hints
- [ ] `resmate --json explain WORKFLOW_SCHEMA_INVALID` returns structured payload
- [ ] All codes in `docs/errors/*.md` are discoverable via `resmate explain --list`

**Technical notes**

Parse markdown taxonomy from PR1 or embed codegen from error registry.

---

### Story P2-6: Update cli-context for Phase 1–2 commands

| Field | Value |
|-------|-------|
| **Story points** | 2 |
| **Labels** | `resmate-cli`, `resmate`, `agent-authoring`, `phase-2`, `documentation` |
| **Depends on** | P1-6, P2-* (as each ships) |
| **Repo** | `resmedai-core-framework` (`cli-context/`) |

**Description**

Keep `cli-context` KB in sync as P1 commands ship: commands-reference, authoring-checklist, agent skill, examples README.

**Acceptance criteria**

- [ ] `cli-context/cli/commands-reference.md` documents all Phase 1 commands + global `--json`
- [ ] Phase 2 commands added as they merge (MCP, init/scaffold, push-all execute, explain)
- [ ] `AGENTS.md` read order unchanged; links valid
- [ ] `pr-agent-v2` workspace copy can sync docs from cli-context

**Technical notes**

Partial Phase 1 docs already landed in cli-context (see Completed doc work below). This story tracks incremental updates through Phase 2.

**Phase 2 total:** 29 SP

---

## Phase 3 stories (6) — CLI P2 + hardening

### Story P3-1: Remote ID existence checks

| Field | Value |
|-------|-------|
| **Story points** | 5 |
| **Labels** | `resmate-cli`, `resmate`, `agent-authoring`, `phase-3`, `validation` |
| **Depends on** | P1-5 |
| **Repo** | `resmed_resmate-cli` |

**Description**

Extend `resmate validate` with optional `--remote` checks: verify referenced platform IDs exist via authenticated API lookup (tools, agents, assistants, hitl, workflows).

**Acceptance criteria**

- [ ] `resmate validate --remote` flags IDs not found on platform
- [ ] Offline mode skips remote checks (default local-only)
- [ ] Stable codes: e.g. `REMOTE_ID_NOT_FOUND`
- [ ] Document latency/auth requirements

**Technical notes**

Requires API round-trips per ref; cache results within single validate run.

---

### Story P3-2: Advanced handler static analysis

| Field | Value |
|-------|-------|
| **Story points** | 5 |
| **Labels** | `resmate-cli`, `resmate`, `agent-authoring`, `phase-3`, `validation` |
| **Depends on** | P1-5 |
| **Repo** | `resmed_resmate-cli` |

**Description**

Beyond secrets grep: lint JS handlers for `workflowPatch` return shape hints, forbidden patterns, and FAAS vs JS package expectations.

**Acceptance criteria**

- [ ] Detect likely missing `workflowPatch` in save-tool handlers (heuristic)
- [ ] FAAS tools without `package.json` → error (existing rule, strengthened)
- [ ] New findings documented in `docs/errors/validation.md`
- [ ] False-positive rate acceptable on `pr-agent-v2` fixtures

**Technical notes**

P0 intentionally limited to secrets grep; this story adds AST/light parsing or convention-based rules.

---

### Story P3-3: resmate diff before push

| Field | Value |
|-------|-------|
| **Story points** | 5 |
| **Labels** | `resmate-cli`, `resmate`, `agent-authoring`, `phase-3`, `diff` |
| **Depends on** | P1-5, P1-6 |
| **Repo** | `resmed_resmate-cli` |

**Description**

`resmate diff [<resource>]` — show local vs remote diff before push. JSON output for agent consumption.

**Acceptance criteria**

- [ ] `resmate diff tool roc-search-users` shows field-level delta vs platform
- [ ] Works for agent, assistant, hitl, workflow where GET APIs exist
- [ ] Integrates with push-all plan (flag resources with remote drift)
- [ ] `--json` envelope with structured diff hunks

**Technical notes**

Depends on existing `api::get_*` endpoints. Workflow diff may be version-sensitive.

---

### Story P3-4: CLI workflow push alignment with smriti loader

| Field | Value |
|-------|-------|
| **Story points** | 5 |
| **Labels** | `resmate-cli`, `resmate`, `agent-authoring`, `phase-3`, `workflow`, `smriti_client` |
| **Depends on** | P1-4 |
| **Repo** | `resmed_resmate-cli` |

**Description**

Align `specs/workflow.rs::load_workflow_from_dir` (push path) with `smriti_client` loader requirements — notably `schema.yaml` in split layout. Eliminate validate-pass / push-fail divergence.

**Acceptance criteria**

- [ ] Workflow that passes `workflow validate` also loads successfully for push
- [ ] `pr-agent-v2` oracle workflow push path uses same layout rules as validator
- [ ] Migration note for workspaces missing `schema.yaml`
- [ ] Tests cover push + validate parity

**Technical notes**

Flagged in P0 plan risks. May share loader code from smriti_client in push path.

---

### Story P3-5: resmate-cli CLAD documentation full replacement

| Field | Value |
|-------|-------|
| **Story points** | 3 |
| **Labels** | `resmate-cli`, `resmate`, `agent-authoring`, `phase-3`, `documentation`, `clad` |
| **Depends on** | Phase 1–2 complete |
| **Repo** | `resmed_resmate-cli`, `resmedai-core-framework` |

**Description**

Replace legacy CLAD doc (`docs/CLAD-resmate-use-cases.md`) with current agent-authoring model: JSON CLI, validate/graph/push-all, cli-context as source of truth.

**Acceptance criteria**

- [ ] CLAD doc reflects Phase 1–3 command surface
- [ ] Links to cli-context, not duplicated platform internals
- [ ] Reviewed by COE Gen AI stakeholders
- [ ] Old doc deprecated or redirected

**Technical notes**

Epic strategic item; aligns external readers with Envisioning deliverables.

---

### Story P3-6: Integration test suite vs pr-agent-v2 in CI

| Field | Value |
|-------|-------|
| **Story points** | 8 |
| **Labels** | `resmate-cli`, `resmate`, `agent-authoring`, `phase-3`, `ci`, `testing` |
| **Depends on** | P1-5 |
| **Repo** | `resmed_resmate-cli` |

**Description**

CI job running full smoke script (P0 plan §1) against `pr-agent-v2` fixtures — graph, validate, workflow validate, push-all dry-run. Optional git submodule or sparse checkout.

**Acceptance criteria**

- [ ] `cargo test` includes integration tests with embedded or checkout fixtures
- [ ] CI workflow runs on PR to `resmed_resmate-cli`
- [ ] Smoke script documented in README
- [ ] Fails on broken_ref or validation regression

**Technical notes**

Option A: submodule `pr-agent-v2` oracle subset. Option B: `testdata/pr-agent-minimal/` (P0 plan §7).

**Phase 3 total:** 31 SP

---

## Completed doc work (reference — no new Jira issues)

The following `cli-context` documentation was completed in `resmedai-core-framework` and serves as the agent-authoring KB baseline. Track here for epic traceability; **status: DONE**.

| Phase | Deliverable | Location | Status |
|-------|-------------|----------|--------|
| Phase 1 | Push order, commands-reference, state-and-workflows | `cli-context/cli/`, `cli-context/platform/` | **DONE** |
| Phase 2 | Workflow authoring vendored, search-dynamic-hitl-save recipe | `cli-context/workflows/` | **DONE** |
| Phase 3 | Link cleanup, runtime checklist | `cli-context/links.md`, authoring-checklist | **DONE** |

Ongoing doc sync for new CLI commands remains **P2-6**.

---

## Summary table

| Key | Story | Phase | SP | Depends on |
|-----|-------|-------|-----|------------|
| [CGA-1095](https://resmedglobal.atlassian.net/browse/CGA-1095) | PR1: JSON output layer and exit codes | 1 | 5 | — |
| [CGA-1096](https://resmedglobal.atlassian.net/browse/CGA-1096) | PR2: workspace info and doctor | 1 | 3 | CGA-1095 |
| [CGA-1097](https://resmedglobal.atlassian.net/browse/CGA-1097) | PR3: dependency link graph | 1 | 5 | CGA-1096 |
| [CGA-1098](https://resmedglobal.atlassian.net/browse/CGA-1098) | PR4: local workflow validate | 1 | 5 | CGA-1095 |
| [CGA-1099](https://resmedglobal.atlassian.net/browse/CGA-1099) | PR5: aggregate workspace validate | 1 | 8 | CGA-1097, CGA-1098 |
| [CGA-1100](https://resmedglobal.atlassian.net/browse/CGA-1100) | PR6: push-all dry-run | 1 | 3 | CGA-1099 |
| [CGA-1101](https://resmedglobal.atlassian.net/browse/CGA-1101) | MCP server wrapping CLI | 2 | 8 | Phase 1 |
| [CGA-1102](https://resmedglobal.atlassian.net/browse/CGA-1102) | init/scaffold + resmate.yaml | 2 | 5 | CGA-1095 |
| [CGA-1103](https://resmedglobal.atlassian.net/browse/CGA-1103) | push-all execute | 2 | 8 | CGA-1100 |
| [CGA-1104](https://resmedglobal.atlassian.net/browse/CGA-1104) | Fix sync HITL pull | 2 | 3 | — |
| [CGA-1105](https://resmedglobal.atlassian.net/browse/CGA-1105) | explain error codes | 2 | 3 | CGA-1095 |
| [CGA-1106](https://resmedglobal.atlassian.net/browse/CGA-1106) | cli-context Phase 1–2 sync | 2 | 2 | Phase 1–2 |
| [CGA-1107](https://resmedglobal.atlassian.net/browse/CGA-1107) | Remote ID existence checks | 3 | 5 | CGA-1099 |
| [CGA-1112](https://resmedglobal.atlassian.net/browse/CGA-1112) | Advanced handler static analysis | 3 | 5 | CGA-1099 |
| [CGA-1108](https://resmedglobal.atlassian.net/browse/CGA-1108) | resmate diff | 3 | 5 | CGA-1099, CGA-1100 |
| [CGA-1109](https://resmedglobal.atlassian.net/browse/CGA-1109) | Workflow push/smriti alignment | 3 | 5 | CGA-1098 |
| [CGA-1110](https://resmedglobal.atlassian.net/browse/CGA-1110) | CLAD doc replacement | 3 | 3 | Phase 1–2 |
| [CGA-1111](https://resmedglobal.atlassian.net/browse/CGA-1111) | CI integration tests pr-agent-v2 | 3 | 8 | CGA-1099 |
| | **Total (18 stories)** | | **89 SP** | |

---

## Jira create parameters (for automation)

```yaml
cloudId: b9752768-29a0-4f57-8e40-c32ef14766c7
projectKey: CGA
issueTypeName: Story
parent: CGA-1094
additional_fields:
  customfield_11353: <story points>
  customfield_11586: { id: "11553" }  # Envisioning
  labels: [resmate-cli, resmate, agent-authoring, phase-N, ...]
```
