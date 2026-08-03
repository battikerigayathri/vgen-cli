# Oracle purchase requisition — reference walkthrough

**Reference sample** — documents the live Oracle PR pattern (search → dynamic HITL → workflow save). Paths below describe a typical workspace layout at the root of a copied `cli-context` folder.

## What this demonstrates

- **FAAS search** — ROC GraphQL user lookup with session-scoped secrets
- **Dynamic HITL** — HITLConfig injects ChoiceSet choices from search results
- **workflowPatch.inputs** — save tool patches `requesterId`; platform auto-advances
- **Single collect stage** — `collect_requester` → `submit` terminal

## Read first

[walkthrough.md](walkthrough.md) — annotated artifact guide.

## Recipe

[workflows/search-dynamic-hitl-save.md](../../workflows/search-dynamic-hitl-save.md)

## Related samples

| Sample | Contrast |
|--------|----------|
| [purchase-requisition/walkthrough.md](../purchase-requisition/walkthrough.md) | Static HITL forms; multi-stage form wizard |
| [jira/walkthrough.md](../jira/walkthrough.md) | Non-workflow HITL + FAAS read |
