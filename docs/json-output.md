# JSON output envelope

All agent-facing ResMate CLI commands support a global `--json` flag that emits a single JSON object on **stdout** per invocation.

## Envelope shape

```json
{
  "ok": true,
  "command": "config show",
  "data": { },
  "error": null,
  "warnings": []
}
```

| Field | Type | Description |
|-------|------|-------------|
| `ok` | boolean | `true` on success, `false` on failure |
| `command` | string | Stable command identifier (e.g. `"config show"`, `"graph"`) |
| `data` | object \| null | Command-specific payload on success; `null` on failure |
| `error` | object \| null | Structured error on failure; `null` on success |
| `warnings` | array | Non-fatal warnings (may be non-empty on success) |

### Error object

```json
{
  "code": "CONFIG_MISSING_API_KEY",
  "message": "RESMATE_API_KEY is not set.",
  "details": null
}
```

| Field | Type | Description |
|-------|------|-------------|
| `code` | string | Stable machine-readable error code (see `docs/errors/`) |
| `message` | string | Human-readable summary |
| `details` | object \| null | Optional structured context (URL, status code, counts, etc.) |

### Warning object

```json
{
  "code": "JWT_SECRET_MISSING",
  "message": "RESMATE_SECRET is not set; JWT auth may fail."
}
```

## Exit codes

| Code | Meaning | Examples |
|------|---------|----------|
| `0` | Success | `config validate` reachable, `validate` with warnings only, `push-all --dry-run` |
| `1` | Runtime / user error | API failure, I/O error, connectivity failure, missing API key |
| `2` | Usage / invalid input | Missing subcommand, bad flags, missing required argument |
| `3` | Validation failed | `validate` has ≥1 error finding; `workflow validate` schema/semantic fail |

### JSON mode behavior

- **Always** print exactly one envelope object on stdout (success or failure).
- Do not mix human-oriented text on stdout in JSON mode.
- Legacy commands that have not been migrated yet may still print human text even with `--json`; new commands must use the envelope.

### Human mode behavior

- Existing `println!` / `eprintln!` output is unchanged for legacy commands.
- Errors go to stderr; success output goes to stdout.

## Commands with JSON support (PR1)

| Command | `data` type | Notes |
|---------|-------------|-------|
| `config show` | `ConfigSummary` | Secrets redacted (`api_key` prefix + `***`, `roc_session` as `(set)`) |
| `config validate` | `ValidateConnectionData` | `url`, `status`, `message` on success |

Additional commands gain JSON envelopes in PR2–PR6.

## Push plan (`push-all --dry-run`)

`data` is a `PushPlan`:

```json
{
  "steps": [
    {
      "order": 1,
      "resource_type": "hitl",
      "name": "roc-select-requester-form",
      "path": "hitl/roc-select-requester-form/meta.yaml",
      "action": "update",
      "has_id": true,
      "id": "…",
      "blockers": []
    }
  ],
  "skipped": []
}
```

| Field | Type | Description |
|-------|------|-------------|
| `steps` | array | Ordered push steps (hitl → workflow → tool → agent → assistant) |
| `skipped` | array | Out-of-scope resources (orphans) with `action: "skip"` |
| `steps[].order` | number | 1-based position in the plan |
| `steps[].resource_type` | string | `hitl`, `workflow`, `tool`, `agent`, or `assistant` |
| `steps[].name` | string | Folder or file stem used by `resmate <type> push <name>` |
| `steps[].action` | string | `create`, `update`, or `skip` |
| `steps[].has_id` | boolean | Whether local YAML/meta already has a platform id |
| `steps[].blockers` | array | Validation error codes that would block push for this resource |

## Examples

```bash
# Success
resmate --json config show | jq '.ok'
# true

# Connectivity failure
resmate --json config validate
echo $?
# 1
# envelope: { "ok": false, "error": { "code": "CONNECTIVITY_FAILED", ... } }

# Missing subcommand (clap usage error)
resmate --json
echo $?
# 2
```
