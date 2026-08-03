# Oracle purchase requisition — annotated walkthrough

**Reference sample** — paths below describe live artifacts at **workspace root** (e.g. a copied `cli-context` renamed `pr-agent-v2`). Copy patterns into your workspace; do not develop under `examples/`.

Field-by-field guide to the search → dynamic HITL → workflow save pattern. Recipe: [search-dynamic-hitl-save.md](../../workflows/search-dynamic-hitl-save.md).

## What this demonstrates

- **Split YAML workflow** — [authoring-formats.md](../../workflows/authoring-formats.md)
- **Dynamic ChoiceSet** — search results populate HITL at runtime
- **workflowPatch.inputs** — save tool patches inputs; platform auto-advances
- **Orchestration Contract** — assistant `workflow_definition_slug` matches `meta.slug`

## Multi-turn flow

```
Turn 1: User "Search for requester Jane Doe"
        → oracle-pr-agent → ROC Search Users → users[]

Turn 2: Agent → ROC Select Requester HITLConfig(users)
        → dynamic select card shown

Turn 3: User submits requesterId
        → ROC Select Requester Save → workflowPatch.inputs.requesterId
        → platform auto-advance → submit (terminal)
```

## ID linking

| Artifact | ID / slug | Links to |
|----------|-----------|----------|
| Workflow definition | slug `oracle-purchase-requisition-v1` | Assistant Orchestration Contract |
| HITL select form | slug `roc-select-requester-form` | `flow.collect_requester.hitlSlug` |
| ROC Search Users | tool id in agent `skills` | Agent skill |
| ROC Select Requester HITLConfig | tool id in agent `skills` | Agent skill |
| ROC Select Requester Save | tool id in agent `skills` | Agent skill |
| oracle-pr-agent | agent id | Assistant `agents` |
| oracle-pr-assistant | assistant id | Entry point |

### Schema ↔ HITL field ids

| Workflow schema key | HITL Input id | Stage |
|---------------------|---------------|-------|
| `requesterId` | `requesterId` | collect_requester |

---

## Workflow — `workflows/oracle-purchase-requisition/`

### meta.yaml

| Field | Value | Purpose |
|-------|-------|---------|
| `slug` | `oracle-purchase-requisition-v1` | Must match assistant `workflow_definition_slug` |
| `workflowType` | `oracle_purchase_requisition` | Matches Orchestration Contract `workflow_type` |
| `version` | increment on push | Append-only versioning |

### schema.yaml

```yaml
fields:
  - key: requesterId
    label: Requester ID
    type: string
    bag: inputs
    required: true
    requiredFromStage: collect_requester
    hitlFieldId: requesterId
```

### flow.yaml

| Stage id | kind | hitlSlug | doneWhen | next |
|----------|------|----------|----------|------|
| `collect_requester` | collect | `roc-select-requester-form` | `[requesterId]` | `submit` |
| `submit` | terminal | — | `terminal` | — |

---

## HITL — `hitl/roc-select-requester-form/`

### meta.yaml

| Field | Purpose |
|-------|---------|
| `slug` | `roc-select-requester-form` — must match `flow.collect_requester.hitlSlug` |
| `preMessage` / `postMessage` | Defaults; HITLConfig may override |

### config.json

Key field — `Input.ChoiceSet` with id `requesterId`:

```json
{
  "id": "requesterId",
  "type": "Input.ChoiceSet",
  "choices": [{ "title": "Select a requester...", "value": "placeholder" }]
}
```

Placeholder choices are replaced at runtime by the HITLConfig tool.

---

## Tools

### ROC Search Users — `tools/roc-search-users/` (FAAS)

| Aspect | Detail |
|--------|--------|
| Input | `name` — search string |
| Output | `users[]` with `id`, `email`, `firstName`, `lastName` |
| Secrets | `smriti.secrets.get` with session-scoped key — see [faas-handlers.md](../../tools/faas-handlers.md) |
| `agentResponseContext` | Hints agent to call HITLConfig next |

### ROC Select Requester HITLConfig — `tools/roc-select-requester-hitlconfig/` (JS)

| Aspect | Detail |
|--------|--------|
| Input | `users` — array from search tool |
| Logic | `queryRecords` by slug; inject `choices` into ChoiceSet id `requesterId` |
| Output | Stringified `{ success, config, preMessage, postMessage }` |

### ROC Select Requester Save — `tools/roc-select-requester-save/` (JS)

| Aspect | Detail |
|--------|--------|
| When | ONLY after HITL submit |
| Input | `requesterId` from submit payload |
| Output | `workflowPatch.inputs.requesterId` |

```javascript
return JSON.stringify({
  success: true,
  workflowPatch: {
    inputs: { requesterId: requesterId },
  },
  agentResponseContext:
    "Requester saved to workflow inputs. Platform advances when collect_requester doneWhen is satisfied.",
});
```

**Never** include `advanceStage` in tool results. See [workflow-authoring.md](../../workflows/workflow-authoring.md).

### Save tool decision tree

```
workflow_definition_slug set?
└─ YES → schema field requesterId required at collect_requester?
         └─ YES → *-save tool after HITL submit
                  returns workflowPatch.inputs.requesterId only
```

---

## Agent — `agents/oracle-pr-agent.yaml`

Stage-aware `systemInstructions`:

1. `collect_requester`: search if needed → HITLConfig with `users` → save on submit
2. Never call save on same turn as HITLConfig
3. Save returns `workflowPatch` only — do not request stage advance

`skills` lists all three tool ids.

---

## Assistant — `assistants/oracle-pr-assistant.yaml`

Orchestration Contract excerpt:

```yaml
workflow_type: oracle_purchase_requisition
workflow_definition_slug: oracle-purchase-requisition-v1
```

Orchestrator reads workflow snapshot stage each turn and assigns jobs aligned with stage kind.

Optional test: `assistants/oracle-pr-assistant/prompt.json` at workspace root, then `resmate assistant test oracle-pr-assistant` — see [cli/test.md](../../cli/test.md).

---

## Push order

```bash
resmate hitl push roc-select-requester-form
resmate workflow push oracle-purchase-requisition
resmate tool push roc-search-users
resmate tool push roc-select-requester-hitlconfig
resmate tool push roc-select-requester-save
resmate agent push oracle-pr-agent
resmate assistant push oracle-pr-assistant
```

---

## Related

- [search-dynamic-hitl-save.md](../../workflows/search-dynamic-hitl-save.md) — recipe
- [state-and-workflows.md](../../platform/state-and-workflows.md) — workflowPatch ownership
- [purchase-requisition/walkthrough.md](../purchase-requisition/walkthrough.md) — static multi-stage wizard contrast
