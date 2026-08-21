# Phase 2 P1 — complete

Phase 2 P1 ResMate CLI implementation is complete in `resmed_vgen-cli`. All six stories delivered locally.

**Jira epic:** [CGA-1094](https://resmedglobal.atlassian.net/browse/CGA-1094)

**Stories:** [CGA-1101](https://resmedglobal.atlassian.net/browse/CGA-1101) through [CGA-1106](https://resmedglobal.atlassian.net/browse/CGA-1106)

**Plan:** [PHASE2-P1-PLAN.md](./PHASE2-P1-PLAN.md)

**Prerequisite:** [PHASE1-P0-COMPLETE.md](./PHASE1-P0-COMPLETE.md) (CGA-1095–CGA-1100)

## Delivered commands

| Story | Capability | Command(s) |
|-------|------------|------------|
| [CGA-1105](https://resmedglobal.atlassian.net/browse/CGA-1105) | Error code explanations | `vgen explain <code>`, `vgen explain --list` |
| [CGA-1102](https://resmedglobal.atlassian.net/browse/CGA-1102) | Workspace bootstrap + manifest | `vgen init`, `vgen scaffold <recipe>`, `vgen.yaml` |
| [CGA-1103](https://resmedglobal.atlassian.net/browse/CGA-1103) | Batch push execution | `vgen push-all --yes` (without `--dry-run`) |
| [CGA-1104](https://resmedglobal.atlassian.net/browse/CGA-1104) | Sync pulls HITL configs | `vgen sync` includes HITL |
| [CGA-1101](https://resmedglobal.atlassian.net/browse/CGA-1101) | MCP server for IDE integration | `vgen-mcp` (stdio) |
| [CGA-1106](https://resmedglobal.atlassian.net/browse/CGA-1106) | cli-context KB sync | commands-reference, checklist, skill updates |

## Smoke test

Run from a use-case workspace root (e.g. `pr-agent-v2`) with `vgen` on `PATH` and valid `.env` for API commands:

```bash
# CGA-1105 — explain
vgen explain BROKEN_AGENT_REF | grep -qi remediation
vgen --json explain WORKFLOW_SCHEMA_INVALID | jq '.ok == true and .data.code != null'
vgen explain --list | grep -q BROKEN_TOOL_REF

# CGA-1104 — sync HITL (requires assistant with id + remote HITL refs)
vgen sync 2>&1 | grep -i hitl
vgen --json workspace info | jq '.data.counts.hitl >= 1'

# CGA-1102 — init/scaffold (run in temp dir)
tmpdir=$(mktemp -d) && cd "$tmpdir"
vgen init --json | jq '.ok == true'
test -d tools && test -d agents && test -f vgen.yaml
vgen scaffold oracle-pr --json | jq '.ok == true'
vgen --json workspace info | jq '.data.detected == true'

# CGA-1103 — push-all execute (dry-run first, then execute with --yes in dev env)
vgen --json push-all --dry-run | jq '.data.steps | map(.resource_type)'
# expect: hitl → workflow → tool → agent → assistant
vgen --json push-all --yes  # only against dev/staging with intentional changes

# CGA-1101 — MCP (manual or integration test harness)
# echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | vgen-mcp | jq '.result.tools | length >= 6'

# P0 regression (must still pass)
vgen --json validate | jq '.data.summary.error_count == 0'
vgen --json graph | jq '[.data.edges[] | select(.kind=="broken_ref")] | length'  # expect 0
```

## MCP setup

See [mcp-setup.md](./mcp-setup.md) for building `vgen-mcp` and wiring it into Cursor.

## Authoring docs

CLI command reference and pre-push checklist live in cli-context:

- [commands-reference.md](../../resmedai-core-framework/cli-context/cli/commands-reference.md)
- [authoring-checklist.md](../../resmedai-core-framework/cli-context/cli/authoring-checklist.md)

## Deferred to P2

- Remote ID existence checks (`validate --remote`) — CGA-1107
- `vgen diff` local vs remote — CGA-1108
- Workflow push loader ↔ smriti validator alignment — CGA-1109
- Handler static analysis beyond secrets grep — CGA-1112
- CI integration test suite vs `pr-agent-v2` — CGA-1111
