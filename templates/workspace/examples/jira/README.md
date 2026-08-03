# Jira use case (reference sample)

**Reference only** — copy patterns into workspace-root `tools/`, `agents/`, `assistants/`, `hitl/`. Annotations: [walkthrough.md](walkthrough.md).

> Do not develop here. Author live artifacts at workspace root (same level as this `examples/` folder).

## What this demonstrates

- **HITL two-step** — `*HITLConfig` JS tool → user submit → `jira-read-issue` FAAS tool
- **Adaptive card** — `${issueKey}` placeholder substitution
- **FaaS tool** — Smriti session secrets, axios, `functionId`, `payload.json`
- **Single-agent assistant** — routes read/summarise requests to jira-agent

## File map

| Path | Role |
|------|------|
| `hitl/jira-read-issue-confirm/` | Adaptive card |
| `tools/jira-read-issue-hitlconfig/` | JS HITLConfig |
| `tools/jira-read-issue/` | FAAS read tool |
| `agents/jira-agent.yaml` | jira-agent (2 skills) |
| `assistants/jira-assistants.yaml` | Jira Read Assistant |

## Optional: smoke-test this reference sample

```bash
cd examples/jira    # from workspace root
```

Or set `RESMATE_*_DIR` to paths under this folder.

```bash
resmate hitl push jira-read-issue-confirm
resmate tool push jira-read-issue-hitlconfig
resmate tool push jira-read-issue
resmate agent push jira-agent
resmate assistant push jira-assistants
```

Requires `.env` with `RESMATE_API_KEY` at workspace root.

## Test

```bash
resmate tool test jira-read-issue
```

Assistant test: see [cli/test.md](../../cli/test.md).

## Related

- [walkthrough.md](walkthrough.md) — annotated guide
- [../minimal/README.md](../minimal/README.md) — simpler reference
- [../../cli/authoring-checklist.md](../../cli/authoring-checklist.md)
