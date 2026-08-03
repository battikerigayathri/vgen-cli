# Purchase requisition (reference sample)

**Reference only** — copy patterns into workspace-root `workflows/`, `tools/`, `agents/`, `assistants/`, `hitl/`. Annotations: [walkthrough.md](walkthrough.md).

> Do not develop here. Author live artifacts at workspace root (same level as this `examples/` folder).

## What this demonstrates

- **Split YAML workflow** — `meta.yaml` + `schema.yaml` + `flow.yaml` under `workflows/purchase-requisition/`
- **Form wizard** — `collect` ×2 → **`review`** → `terminal` with platform-owned stage advance
- **HITL + workflowPatch** — save tools return `{ success, workflowPatch: { inputs } }` only
- **Orchestration Contract** — assistant binds `workflow_definition_slug: purchase-requisition-v1`

## File map

| Path | Role |
|------|------|
| `workflows/purchase-requisition/` | Split YAML workflow definition |
| `hitl/purchase-req-vendor-form/` | Vendor collect card |
| `hitl/purchase-req-lines-form/` | Line items collect card |
| `hitl/purchase-req-review/` | Review summary confirm card |
| `tools/purchase-req-*-hitlconfig/` | HITLConfig tools per stage |
| `tools/purchase-req-*-save/` / `purchase-req-review-confirm/` | Save tools with `workflowPatch.inputs` |
| `agents/purchase-req-agent.yaml` | Stage-aware agent (6 skills) |
| `assistants/purchase-req-assistant.yaml` | Orchestration Contract + routing |

## Push order

**Always:** hitl → workflow → tool → agent → assistant

### Interim (in-repo binary — until external ResMate CLI ships)

From monorepo root:

```bash
node scripts/mongo/ensure-workflow-indexes.js

cargo run -p smriti_client --bin push_workflow_definition -- \
  --path cli-context/examples/purchase-requisition/workflows/purchase-requisition \
  --validate-only

cargo run -p smriti_client --bin push_workflow_definition -- \
  --path cli-context/examples/purchase-requisition/workflows/purchase-requisition
```

Set `WORKFLOWS_DIR` only affects the binary default path; use `--path` for this sample.

### ResMate CLI (external repo — Phase 9.7 WS1)

Copy this folder to your workspace or set dirs for one-shot push:

```bash
cd examples/purchase-requisition

export RESMATE_HITL_DIR=./hitl
export RESMATE_WORKFLOWS_DIR=./workflows
export RESMATE_TOOLS_DIR=./tools
export RESMATE_AGENTS_DIR=./agents
export RESMATE_ASSISTANTS_DIR=./assistants

resmate hitl push purchase-req-vendor-form
resmate hitl push purchase-req-lines-form
resmate hitl push purchase-req-review
resmate workflow push purchase-requisition
resmate tool push purchase-req-vendor-hitlconfig
resmate tool push purchase-req-vendor-save
resmate tool push purchase-req-lines-hitlconfig
resmate tool push purchase-req-lines-save
resmate tool push purchase-req-review-hitlconfig
resmate tool push purchase-req-review-confirm
resmate agent push purchase-req-agent
resmate assistant push purchase-req-assistant
```

Requires `.env` with `RESMATE_API_KEY` at workspace root.

## Validate

```bash
cargo run -p smriti_client --bin push_workflow_definition -- \
  --path cli-context/examples/purchase-requisition/workflows/purchase-requisition \
  --validate-only
```

## Smoke (optional)

Bind chat to `purchase-req-assistant` after push, then:

```bash
CHAT_SMOKE_SCENARIO=workflow-purchase-requisition ./scripts/chat-smoke-test.sh <chat_id> <session_id>
```

See [walkthrough.md](walkthrough.md) for multi-turn flow and log grep hints.

## Fixture parity

**Single source of truth:** `cli-context/examples/purchase-requisition/workflows/purchase-requisition/`.

Crate tests copy from here into `lib/smriti_client/tests/fixtures/purchase-requisition-split/` (review stage included). Re-sync after workflow edits:

```bash
cp -r cli-context/examples/purchase-requisition/workflows/purchase-requisition/* \
  lib/smriti_client/tests/fixtures/purchase-requisition-split/
```

## Related

- [walkthrough.md](walkthrough.md) — annotated guide
- [../../workflows/workflow-form-wizard.md](../../workflows/workflow-form-wizard.md) — recipe
- [../../workflows/workflow-authoring.md](../../workflows/workflow-authoring.md) — platform authoring reference
