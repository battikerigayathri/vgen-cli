# Phase 1 P0 — complete

Phase 1 P0 ResMate CLI implementation is complete in `resmed_resmate-cli`. All six PRs merged locally.

**Jira epic:** [CGA-1094](https://resmedglobal.atlassian.net/browse/CGA-1094)

**Stories:** CGA-1095 through CGA-1100

## Delivered commands

| PR | Command(s) |
|----|------------|
| 1 | Global `--json` envelope + stable exit codes |
| 2 | `resmate workspace info`, `resmate doctor` |
| 3 | `resmate graph` |
| 4 | `resmate workflow validate <folder>` |
| 5 | `resmate validate` |
| 6 | `resmate push-all --dry-run` |

## Smoke test

Run from a use-case workspace root (e.g. `pr-agent-v2`) with `resmate` on `PATH`:

```bash
# PR1 — envelope + exit codes
resmate --json config show | jq '.ok == true'
resmate --json config validate          # exit 0 if API reachable

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
# expect: hitl → workflow → tool → agent → assistant
```

## Authoring docs

CLI command reference and pre-push checklist live in cli-context:

- [commands-reference.md](../../resmedai-core-framework/cli-context/cli/commands-reference.md)
- [authoring-checklist.md](../../resmedai-core-framework/cli-context/cli/authoring-checklist.md)

## Deferred to P1

- `push-all` execute (batch push with rollback)
- Remote validation via `validate --offline` (flag reserved)
