# Purchase requisition — annotated walkthrough

**Reference sample** — paths below are under `examples/purchase-requisition/`. Copy patterns into workspace-root folders when building a real use case.

Field-by-field guide to every artifact. Operational commands stay in [README.md](README.md).

## What this demonstrates

- **Split YAML workflow** — default ResMate authoring layout ([authoring-formats.md](../../workflows/authoring-formats.md))
- **Form wizard** — `collect_vendor` → `collect_line_items` → **`review_summary`** → `submit`
- **workflowPatch.inputs** — collect/review save tools patch inputs; platform auto-advances
- **Review gate** — completeness check before terminal hop ([workflow-form-wizard.md](../../workflows/workflow-form-wizard.md))
- **Orchestration Contract** — assistant `workflow_definition_slug` matches `meta.slug`

## Multi-turn flow (4+ turns)

```
Turn 1: User "Start a purchase requisition"
        → compose binds purchase-requisition-v1
        → purchase-req-agent → Vendor HITLConfig → card shown

Turn 2: User submits vendorId → resume chat message (see below)
        → purchase-req-agent parses vendorId → Vendor Save → workflowPatch.inputs.vendorId
        → platform auto-advance → collect_line_items

Turn 3: User submits lineItems JSON → resume chat message
        → purchase-req-agent parses lineItems → Lines Save → workflowPatch.inputs.lineItems
        → auto-advance → review_summary

Turn 4: User confirms review → resume chat message
        → purchase-req-agent parses reviewConfirmed → Review Confirm → workflowPatch.inputs.reviewConfirmed
        → completeness gate → auto-advance → submit (terminal)
```

### Resume chat messages (exact format)

Every HITL submit in this flow arrives as a normal chat message — **not** a special resume API call — and `context.input` is **not** auto-filled from the card. See [hitl-resume-and-message-format.md](../../.cursor/skills/resmate-use-case/docs/hitl-resume-and-message-format.md) for the full contract. `purchase-req-agent` must parse the `key: value` lines out of the message text and route them to the matching Save Tool (never re-invoke the HITLConfig tool for the same field).

**Turn 2 — vendor form submit:**

```text
This is my data, please process further
vendorId: 1002
```

**Turn 3 — line items form submit:**

```text
This is my data, please process further
lineItems: [{"partNumber":"PN-100","quantity":5},{"partNumber":"PN-200","quantity":2}]
```

**Turn 4 — review confirm submit:**

```text
This is my data, please process further
reviewConfirmed: true
```

Each Save Tool (`purchase-req-vendor-save`, `purchase-req-lines-save`, `purchase-req-review-confirm`) only returns a `workflowPatch.inputs` object — it never issues an `advanceStage` instruction. The platform re-evaluates the active stage's `doneWhen` after the patch is applied and auto-advances when satisfied.

## ID linking

All `id` fields below are left empty (`id: ""`) in this sample on purpose — never hand-author or copy-paste a MongoDB ObjectId. Push each layer to get a real, platform-assigned ID, then wire it into the dependent per [push-pull-wire.md](../../.cursor/skills/resmate-use-case/docs/push-pull-wire.md).

| Artifact | ID / slug | Links to |
|----------|-----------|----------|
| Workflow definition | slug `purchase-requisition-v1` | Assistant Orchestration Contract |
| HITL vendor | slug `purchase-req-vendor-form` | `flow.collect_vendor.hitlSlug` |
| HITL lines | slug `purchase-req-lines-form` | `flow.collect_line_items.hitlSlug` |
| HITL review | slug `purchase-req-review` | `flow.review_summary.hitlSlug` |
| Vendor HITLConfig | `<written-back after push>` | Agent skill |
| Vendor Save | `<written-back after push>` | Agent skill |
| Lines HITLConfig | `<written-back after push>` | Agent skill |
| Lines Save | `<written-back after push>` | Agent skill |
| Review HITLConfig | `<written-back after push>` | Agent skill |
| Review Confirm | `<written-back after push>` | Agent skill |
| purchase-req-agent | `<written-back after push>` | Assistant agent |
| purchase-req-assistant | `<written-back after push>` | Entry point |

### Schema ↔ HITL field ids

| Workflow schema key | HITL Input id | Stage |
|---------------------|---------------|-------|
| `vendorId` | `vendorId` | collect_vendor |
| `lineItems` | `lineItems` | collect_line_items |
| `reviewConfirmed` | `reviewConfirmed` | review_summary |

---

## Workflow — `workflows/purchase-requisition/`

### meta.yaml

| Field | Value | Purpose |
|-------|-------|---------|
| `slug` | `purchase-requisition-v1` | Must match assistant `workflow_definition_slug` |
| `workflowType` | `purchase_requisition` | Matches Orchestration Contract `workflow_type` |
| `version` | `1` | Increment on each push (append-only in Mongo) |

### schema.yaml

Defines `inputs` bags: `vendorId`, `lineItems`, `reviewConfirmed`. `requiredFromStage` drives completeness.

### flow.yaml

| Stage id | kind | doneWhen | next |
|----------|------|----------|------|
| `collect_vendor` | collect | `[vendorId]` | `collect_line_items` |
| `collect_line_items` | collect | `[lineItems]` | `review_summary` |
| `review_summary` | review | `[reviewConfirmed]` | `submit` |
| `submit` | terminal | `terminal` | — |

Push workflow **after** HITL records (semantic lint references `hitlSlug`).

**Interim validate/push:**

```bash
cargo run -p smriti_client --bin push_workflow_definition -- \
  --path cli-context/examples/purchase-requisition/workflows/purchase-requisition \
  --validate-only
```

**ResMate CLI (when available):** `resmate workflow push purchase-requisition`

---

## HITL records

Push first:

```bash
resmate hitl push purchase-req-vendor-form
resmate hitl push purchase-req-lines-form
resmate hitl push purchase-req-review
```

Each folder: `meta.yaml` (slug) + `config.json` (Adaptive Card). Form field `id` values become HITL submit payload keys for save tools.

---

## Tools

| Tool folder | Returns | Stage |
|-------------|---------|-------|
| `purchase-req-vendor-hitlconfig` | HITL card | collect_vendor |
| `purchase-req-vendor-save` | `workflowPatch.inputs.vendorId` | after vendor submit |
| `purchase-req-lines-hitlconfig` | HITL card | collect_line_items |
| `purchase-req-lines-save` | `workflowPatch.inputs.lineItems` | after lines submit |
| `purchase-req-review-hitlconfig` | Review card (substitutes snapshot) | review_summary |
| `purchase-req-review-confirm` | `workflowPatch.inputs.reviewConfirmed` | after review submit |

**Never** include `advanceStage` in tool results. See [workflow-authoring.md](../../workflows/workflow-authoring.md).

Push all tools after workflow:

```bash
resmate tool push purchase-req-vendor-hitlconfig
# ... (each tool)
```

---

## Agent — `agents/purchase-req-agent.yaml`

| Field | Notes |
|-------|-------|
| `slug` | `purchase-req-agent` — matches `flow.*.agentSlug` |
| `skills` | Six tool IDs — HITLConfig + save per collect/review stage |
| `systemInstructions` | Stage-aware; never-rules for same-turn HITLConfig + save |

Push after tools: `resmate agent push purchase-req-agent`

---

## Assistant — `assistants/purchase-req-assistant.yaml`

| Field | Notes |
|-------|-------|
| `agents` | Single ID — purchase-req-agent |
| `systemContext` | `## Orchestration Contract` block with `workflow_definition_slug: purchase-requisition-v1` |

Push last: `resmate assistant push purchase-req-assistant`

---

## Author checklist (verify)

- [ ] Split YAML under `workflows/purchase-requisition/`
- [ ] `meta.slug` = assistant `workflow_definition_slug`
- [ ] HITL field ids match schema keys
- [ ] Save tools return `workflowPatch.inputs` only
- [ ] Push order: hitl → workflow → tool → agent → assistant
- [ ] `push_workflow_definition --validate-only` passes
- [ ] Multi-turn smoke: `CHAT_SMOKE_SCENARIO=workflow-purchase-requisition`

### Manual verification (Prajna logs)

After each HITL save:

```
grep '[workflow] patch applied'   # stage should advance when doneWhen satisfied
grep 'collect_line_items'         # after vendor save
grep 'review_summary'             # after lines save
grep 'validator_pass'             # N/A for this sample (no agent_task)
```

Terminal submit blocked when incomplete:

```
grep 'missingRequired'            # negative test — skip review confirm
```

---

## Related

- [README.md](README.md) — push commands, fixture sync
- [../github-pr-create/walkthrough.md](../github-pr-create/walkthrough.md) — agent_task + validator pass sample
- [../../cli/authoring-checklist.md](../../cli/authoring-checklist.md)
