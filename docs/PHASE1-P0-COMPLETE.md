# Phase 1 P0 — complete

Phase 1 P0 ResMate CLI implementation is complete in `resmed_vgen-cli`. All six PRs merged locally.

**Jira epic:** [CGA-1094](https://resmedglobal.atlassian.net/browse/CGA-1094)

**Stories:** CGA-1095 through CGA-1100

## Delivered commands

| PR | Command(s) |
|----|------------|
| 1 | Global `--json` envelope + stable exit codes |
| 2 | `vgen workspace info`, `vgen doctor` |
| 3 | `vgen graph` |
| 4 | `vgen workflow validate <folder>` |
| 5 | `vgen validate` |
| 6 | `vgen push-all --dry-run` |

## Smoke test

Run from a use-case workspace root (e.g. `pr-agent-v2`) with `vgen` on `PATH`:

```bash
# PR1 — envelope + exit codes
vgen --json config show | jq '.ok == true'
vgen --json config validate          # exit 0 if API reachable

# PR2 — workspace
vgen --json workspace info | jq '.data.artifact_dirs.tools.count >= 3'
vgen --json doctor | jq '.data.checks | length >= 3'

# PR3 — graph
vgen --json graph | jq '.data.nodes | map(.kind) | unique'
vgen --json graph | jq '[.data.edges[] | select(.kind=="broken_ref")] | length'  # expect 0

# PR4 — workflow validate
vgen --json workflow validate oracle-purchase-requisition | jq '.ok == true'

# PR5 — full validate
vgen --json validate | jq '.data.summary.error_count == 0'

# PR6 — push plan
vgen --json push-all --dry-run | jq '.data.steps | map(.resource_type)'
# expect: hitl → workflow → tool → agent → assistant
```

## Authoring docs

CLI command reference and pre-push checklist live in cli-context:

- [commands-reference.md](../../resmedai-core-framework/cli-context/cli/commands-reference.md)
- [authoring-checklist.md](../../resmedai-core-framework/cli-context/cli/authoring-checklist.md)

## Deferred to P1

- `push-all` execute (batch push with rollback)
- Remote validation via `validate --offline` (flag reserved)
