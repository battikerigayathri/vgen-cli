# Minimal use case (reference sample)

**Reference only** — smallest layout to copy into workspace-root `tools/`, `agents/`, `assistants/`. No HITL, no FAAS.

For HITL + FAAS see [jira/walkthrough.md](../jira/walkthrough.md).

## What this demonstrates

- JS tool handler — top-level script, JSON string return
- Agent with one skill and simple instructions
- Single-agent assistant routing
- Create-on-push ID workflow

## File map

| Path | Role |
|------|------|
| `tools/greet/` | JS greet tool |
| `agents/greet-agent.yaml` | greet-agent |
| `assistants/greet-assistant.yaml` | Greet Assistant |

## Develop your own use case

1. Copy files from this reference into workspace-root `tools/`, `agents/`, `assistants/`.
2. Push from **workspace root**:

```bash
vgen config validate
vgen tool push greet
# add tool id to agent skills, then agent push, etc.
```

See [cli/setup.md](../../cli/setup.md) and [cli/authoring-checklist.md](../../cli/authoring-checklist.md).

## Optional: smoke-test this reference sample

```bash
cd examples/minimal
vgen config validate
```

## Related

- [../../tools/js-handlers.md](../../tools/js-handlers.md)
- [../jira/walkthrough.md](../jira/walkthrough.md)
