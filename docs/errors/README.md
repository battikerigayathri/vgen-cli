# Error code taxonomy

Stable machine-readable error codes for ResMate CLI JSON envelopes.

## Domain files

| File | Domain | Contents |
|------|--------|----------|
| [config.md](config.md) | config, workspace, graph, usage | Connectivity, workspace detection, graph build |
| [validation.md](validation.md) | validation | Graph/artifact validation finding codes |
| [workflow.md](workflow.md) | workflow | Workflow schema/semantic/layout codes |
| [push.md](push.md) | push | push-all preflight and execution codes |

Use `vgen explain <CODE>` for description and remediation hints, or `vgen explain --list [--domain validation]`.

## CLI usage

```bash
vgen explain BROKEN_AGENT_REF
vgen --json explain WORKFLOW_SCHEMA_INVALID
vgen explain --list
vgen explain --list --domain validation
```

## Adding codes

1. Add a row to the relevant `docs/errors/*.md` table (include Remediation column).
2. Use `emit_error(ctx, "CODE", message, details, CliExitCode::...)` from `output.rs`.
3. Rebuild — codes are embedded at compile time via `errors_registry.rs`.

## Exit codes (top-level envelope)

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Runtime / user error |
| `2` | Usage / invalid input |
| `3` | Validation failed |

Finding codes (validation/workflow) use `—` in the Exit column; they appear inside `validate` report findings, not as top-level envelope codes unless noted.
