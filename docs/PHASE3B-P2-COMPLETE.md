# Phase 3B (P2) — Validation hardening — COMPLETE

**Status:** Done  
**Repo:** `resmed_resmate-cli`  
**Epic:** [CGA-1094](https://resmedglobal.atlassian.net/browse/CGA-1094)

## PRs delivered

| PR | Jira | Summary |
|----|------|---------|
| PR14 | [CGA-1109](https://resmedglobal.atlassian.net/browse/CGA-1109) | `workflow_loader.rs` delegates to `smriti_client::WorkflowDefinitionLoader`; push and validate share layout rules |
| PR15 | [CGA-1112](https://resmedglobal.atlassian.net/browse/CGA-1112) | Handler static analysis (`handler_lint.rs`) — `MISSING_WORKFLOW_PATCH`, `FAAS_HANDLER_SHAPE`, `JS_HANDLER_SHAPE` |
| PR16 | [CGA-1107](https://resmedglobal.atlassian.net/browse/CGA-1107) | `validate --remote`, `remote_cache.rs`, `remote_rules.rs`, `REMOTE_SKIPPED_OFFLINE` |
| PR17 | [CGA-1108](https://resmedglobal.atlassian.net/browse/CGA-1108) | `resmate diff <type> <name>`, push dry-run `remote_drift` / `drift_fields` |
| PR18 | [CGA-1111](https://resmedglobal.atlassian.net/browse/CGA-1111) | `scripts/smoke-pr-agent.sh`, `tests/pr_agent_smoke_test.rs`, `Cargo.lock` committed |
| PR20 remainder | CGA-1106 cont. | Kit doc sync (`validate --remote`, `diff`) in templates + `pr-agent-v2` |

## Key files

| Area | Files |
|------|-------|
| Workflow loader | `src/workflow_loader.rs`, `src/specs/workflow.rs` |
| Handler lint | `src/validate/handler_lint.rs` |
| Remote validate | `src/remote_cache.rs`, `src/validate/remote_rules.rs` |
| Diff | `src/diff.rs`, `src/commands/diff.rs` |
| Push drift | `src/push_plan.rs` (`remote_drift`, `drift_fields`) |
| Smoke | `scripts/smoke-pr-agent.sh`, `tests/pr_agent_smoke_test.rs` |
| Errors | `docs/errors/validation.md` |
| Kit docs | `templates/workspace/cli/*`, skill, `AGENTS.md`; synced to `pr-agent-v2` |

## Verification

```bash
cd resmed_resmate-cli
cargo build && cargo test

# Against pr-agent-v2 (local paths)
cd /path/to/pr-agent-v2
resmate --json workflow validate oracle-purchase-requisition | jq '.ok == true'
resmate --json validate | jq '.data.summary.error_count == 0'
resmate --json validate --remote | jq '.data.summary.error_count == 0'   # needs API key
resmate --json diff tool roc-search-users | jq '.data.has_changes == false'  # needs API key

cd resmed_resmate-cli
./scripts/smoke-pr-agent.sh
```

## Deferred to Phase 4

- `.github/workflows/ci.yml`
- CI badge
- Scheduled `validate --remote` job

## Jira keys to close

- CGA-1109, CGA-1112, CGA-1107, CGA-1108, CGA-1111
- CGA-1106 (doc sync remainder)
