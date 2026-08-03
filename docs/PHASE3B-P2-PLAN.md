# Phase 3B (P2) — Validation hardening (deferred)

**Status:** Planned (after Phase 3A)  
**Target repo:** [`resmed_resmate-cli`](../.)  
**Jira epic:** [CGA-1094](https://resmedglobal.atlassian.net/browse/CGA-1094)  
**Prerequisite:** [PHASE3A-P2-PLAN.md](./PHASE3A-P2-PLAN.md) PR12 merged

**Related:** [PHASE3-P2-PLAN.md](./PHASE3-P2-PLAN.md) (index) · [AUTHORING-KIT-ARCHITECTURE.md](./AUTHORING-KIT-ARCHITECTURE.md)

---

## 1. Goals

Close the **validation and push-confidence** gaps in CGA-1094 after the authoring kit (3A) lands:

- Validate ↔ push loader parity (smriti)
- Handler static analysis
- Remote ID checks and local/remote diff
- Local integration smoke (no GHA)
- Kit doc sync for P2 commands

---

## 2. PR breakdown

| PR | Jira | Title | SP | Depends on |
|----|------|-------|-----|------------|
| **PR14** | [CGA-1109](https://resmedglobal.atlassian.net/browse/CGA-1109) | Workflow push ↔ smriti loader alignment | 5 | P0 PR4 |
| **PR15** | [CGA-1112](https://resmedglobal.atlassian.net/browse/CGA-1112) | Advanced handler static analysis | 5 | P0 PR5 |
| **PR16** | [CGA-1107](https://resmedglobal.atlassian.net/browse/CGA-1107) | Remote ID existence checks | 5 | P0 PR5 |
| **PR17** | [CGA-1108](https://resmedglobal.atlassian.net/browse/CGA-1108) | `resmate diff` before push | 5 | PR16, P0 PR6 |
| **PR18** | [CGA-1111](https://resmedglobal.atlassian.net/browse/CGA-1111) | Local integration tests + smoke script (**no GHA**) | 5 | PR14+ |
| **PR20 remainder** | CGA-1106 cont. | Kit doc sync (`validate --remote`, `diff`, MCP) | 2 | PR12–18 |

**Phase 3B total:** ~27 SP · ~15–18 person-days

**Deferred to Phase 4:** `.github/workflows/ci.yml`, CI badge, scheduled remote validate job.

---

## 3. PR summaries

### PR14 — Workflow smriti alignment (CGA-1109)

- Add `src/workflow_loader.rs` delegating to `smriti_client::WorkflowDefinitionLoader`
- Replace duplicate loader in `src/specs/workflow.rs`
- Parity tests: validate-pass = push-load-pass on `pr-agent-v2` oracle workflow

### PR15 — Handler lint (CGA-1112)

- `src/validate/handler_lint.rs` — `workflowPatch`, FAAS shape heuristics
- Codes: `MISSING_WORKFLOW_PATCH`, `FAAS_HANDLER_SHAPE`

### PR16 — Remote validate (CGA-1107)

- `resmate validate --remote` — ID existence via API
- `src/remote_cache.rs`, `src/validate/remote_rules.rs`
- `--offline` wins → `REMOTE_SKIPPED_OFFLINE`

### PR17 — Diff (CGA-1108)

- `resmate diff [<type>] <name>` — field-level local vs remote
- Push dry-run `remote_drift` per step

### PR18 — Local smoke + testdata (CGA-1111)

- `scripts/smoke-pr-agent.sh`, `testdata/pr-agent-minimal/`
- `tests/pr_agent_smoke_test.rs`
- **No** GitHub Actions in P2
- Commit `Cargo.lock` for reproducible local builds

### PR20 remainder — Kit doc sync

- Update `templates/workspace/cli/commands-reference.md`, `authoring-checklist.md`, skill, `AGENTS.md`
- Refresh `pr-agent-v2` kit paths from templates

---

## 4. Recommended merge order

1. PR14 + PR15 (parallel)
2. PR16 → PR17
3. PR18 (skeleton after PR14)
4. PR20 remainder (last)

---

## 5. Success criteria (smoke against `pr-agent-v2`)

```bash
resmate --json workflow validate oracle-purchase-requisition | jq '.ok == true'
resmate --json validate | jq '.data.summary.error_count == 0'
resmate --json validate --remote | jq '.data.summary.error_count == 0'
resmate --json diff tool roc-search-users | jq '.data.has_changes == false'
./scripts/smoke-pr-agent.sh
cargo test
```

---

## 6. Phase 4 note (GHA)

Phase 3B documents local gates only. Phase 4 adds:

- `.github/workflows/ci.yml` running `cargo fmt/clippy/test` + `smoke-pr-agent.sh`
- CI badge in README
- Optional scheduled `validate --remote` against dev API

---

## 7. Jira traceability

| Key | PR |
|-----|-----|
| [CGA-1109](https://resmedglobal.atlassian.net/browse/CGA-1109) | PR14 |
| [CGA-1112](https://resmedglobal.atlassian.net/browse/CGA-1112) | PR15 |
| [CGA-1107](https://resmedglobal.atlassian.net/browse/CGA-1107) | PR16 |
| [CGA-1108](https://resmedglobal.atlassian.net/browse/CGA-1108) | PR17 |
| [CGA-1111](https://resmedglobal.atlassian.net/browse/CGA-1111) | PR18 |
| CGA-1106 cont. | PR20 remainder |
