# GitHub PR create (reference sample)

**Reference only** — copy patterns into workspace-root folders. Annotations: [walkthrough.md](walkthrough.md).

## What this demonstrates

- **Bundle JSON workflow** — single `workflows/github-pr-create/workflow.json`
- **agent_task stage** — `doneWhen: validator_pass`; advance via macro validator, not tool patch
- **workflowPatch.artifacts** — `GitHub PR Open` returns `prUrl`; stage stays until validator pass
- **Orchestration Contract** — `workflow_definition_slug: github-pr-create-v1`

## File map

| Path | Role |
|------|------|
| `workflows/github-pr-create/workflow.json` | Bundle JSON workflow definition |
| `hitl/github-pr-intent-form/` | Repo + feature collect card |
| `tools/github-pr-intent-hitlconfig/` | HITLConfig for collect_intent |
| `tools/github-pr-intent-save/` | `workflowPatch.inputs` |
| `tools/github-pr-open/` | Mock PR — `workflowPatch.artifacts.prUrl` |
| `agents/github-pr-agent.yaml` | Collect + agent_task agent |
| `assistants/github-pr-assistant.yaml` | Orchestration Contract + routing |

## Push order

**Always:** hitl → workflow → tool → agent → assistant

### Interim (in-repo binary)

```bash
cargo run -p smriti_client --bin push_workflow_definition -- \
  --path cli-context/examples/github-pr-create/workflows/github-pr-create \
  --format json \
  --validate-only

cargo run -p smriti_client --bin push_workflow_definition -- \
  --path cli-context/examples/github-pr-create/workflows/github-pr-create \
  --format json
```

### ResMate CLI (external repo — Phase 9.7 WS1)

```bash
cd examples/github-pr-create

export RESMATE_HITL_DIR=./hitl
export RESMATE_WORKFLOWS_DIR=./workflows
export RESMATE_TOOLS_DIR=./tools
export RESMATE_AGENTS_DIR=./agents
export RESMATE_ASSISTANTS_DIR=./assistants

resmate hitl push github-pr-intent-form
resmate workflow push github-pr-create
resmate tool push github-pr-intent-hitlconfig
resmate tool push github-pr-intent-save
resmate tool push github-pr-open
resmate agent push github-pr-agent
resmate assistant push github-pr-assistant
```

## Smoke (optional)

Bind chat to `github-pr-assistant`, then:

```bash
CHAT_SMOKE_SCENARIO=workflow-validator-gate ./scripts/chat-smoke-test.sh <chat_id> <session_id>
```

## Fixture parity

**SSOT:** `cli-context/examples/github-pr-create/workflows/github-pr-create/workflow.json`

Synced to `lib/smriti_client/tests/fixtures/github-pr-create-bundle/workflow.json`.

## Related

- [walkthrough.md](walkthrough.md)
- [../../workflows/workflow-agent-task.md](../../workflows/workflow-agent-task.md)
- [../../workflows/workflow-authoring.md](../../workflows/workflow-authoring.md)
