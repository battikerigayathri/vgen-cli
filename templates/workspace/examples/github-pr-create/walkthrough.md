# GitHub PR create — annotated walkthrough

**Reference sample** — paths under `examples/github-pr-create/`.

## What this demonstrates

- **Bundle JSON workflow** — [authoring-formats.md](../../workflows/authoring-formats.md)
- **collect → agent_task → terminal** — validator pass advances stage, not tool patch
- **workflowPatch.artifacts** — `GitHub PR Open` sets `prUrl`; platform waits for macro validator
- **Mock PR tool** — smoke-friendly; swap for live GitHub API in production workspaces

## Multi-turn flow (3+ turns)

```
Turn 1: User "Create a PR for org/repo — add dark mode"
        → compose binds github-pr-create-v1
        → github-pr-agent → Intent HITLConfig → card

Turn 2: User submits repoName + featureDescription → resume chat message (see below)
        → github-pr-agent parses repoName + featureDescription → Intent Save → workflowPatch.inputs
        → auto-advance → generate_and_open_pr (agent_task)

Turn 3: Agent calls GitHub PR Open
        → workflowPatch.artifacts.prUrl
        → stage UNCHANGED (still agent_task)

Turn 4: Macro validator requirement_met
        → confirm_validator_pass_and_advance → submitted
```

### The resume chat message (exact format)

There is **no special resume API** — the frontend posts the submitted form data back as a normal chat message, and `context.input` is **not** auto-filled from the card (see [hitl-resume-and-message-format.md](../../.cursor/skills/resmate-use-case/docs/hitl-resume-and-message-format.md)). Submitting the intent card produces:

```text
This is my data, please process further
repoName: org/repo
featureDescription: Add dark mode toggle to settings
```

`github-pr-agent` must parse these `key: value` lines and call `github-pr-intent-save` with `repoName` and `featureDescription` as arguments — it must **not** re-invoke `github-pr-intent-hitlconfig`. The Save Tool then returns only `workflowPatch.inputs`; the platform re-evaluates `doneWhen: [repoName, featureDescription]` and auto-advances to `generate_and_open_pr` once both fields are present.

## ID linking

All `id` fields below are left empty (`id: ""`) in this sample on purpose — never hand-author or copy-paste a MongoDB ObjectId. Push each layer to get a real, platform-assigned ID, then wire it into the dependent per [push-pull-wire.md](../../.cursor/skills/resmate-use-case/docs/push-pull-wire.md).

| Artifact | ID / slug | Links to |
|----------|-----------|----------|
| Workflow | slug `github-pr-create-v1` | Assistant Orchestration Contract |
| HITL intent | slug `github-pr-intent-form` | `flow.collect_intent.hitlSlug` |
| Intent HITLConfig | `<written-back after push>` | Agent skill |
| Intent Save | `<written-back after push>` | Agent skill |
| PR Open (mock) | `<written-back after push>` | Agent skill |
| github-pr-agent | `<written-back after push>` | `flow.*.agentSlug` |
| github-pr-assistant | `<written-back after push>` | Entry point |

### Schema ↔ HITL

| Schema key | HITL Input id | Bag |
|------------|---------------|-----|
| `repoName` | `repoName` | inputs |
| `featureDescription` | `featureDescription` | inputs |
| `prUrl` | — (artifact from tool) | artifacts |

---

## Workflow — `workflows/github-pr-create/workflow.json`

| Stage id | kind | doneWhen | Notes |
|----------|------|----------|-------|
| `collect_intent` | collect | `[repoName, featureDescription]` | HITL + inputs patch |
| `generate_and_open_pr` | agent_task | `validator_pass` | Requires `expectedOutcome` |
| `submitted` | terminal | `terminal` | Final status |

**Interim local validate (monorepo dev):**

```bash
cargo run -p smriti_client --bin push_workflow_definition -- \
  --path cli-context/examples/github-pr-create/workflows/github-pr-create \
  --format json \
  --validate-only
```

**ResMate CLI (when available):** `resmate workflow push github-pr-create`

---

## Tools

### GitHub PR Intent Save

Returns:

```json
{
  "success": true,
  "workflowPatch": {
    "inputs": { "repoName": "org/repo", "featureDescription": "..." }
  }
}
```

### GitHub PR Open (mock)

Returns:

```json
{
  "success": true,
  "workflowPatch": {
    "artifacts": { "prUrl": "https://github.com/org/repo/pull/123" }
  }
}
```

**Does not advance stage.** Macro validator must return `requirement_met` against `expectedOutcome: PR opened with URL on workflow artifacts`.

---

## Agent & assistant

- Agent slug `github-pr-agent` referenced in workflow stages
- Assistant includes `deliverable_fields: [prUrl]` for validator checklist
- Push order: hitl → workflow → tools → agent → assistant

---

## Author checklist (verify)

- [ ] Bundle JSON passes `push_workflow_definition --validate-only` (or pushes via `resmate workflow push`)
- [ ] `agent_task` has `expectedOutcome`
- [ ] Collect tool returns `workflowPatch.inputs` only
- [ ] PR open returns `workflowPatch.artifacts` only
- [ ] Push order: hitl → workflow → tool → agent → assistant
- [ ] Smoke: `CHAT_SMOKE_SCENARIO=workflow-validator-gate`

### Manual verification (Prajna logs)

```
grep '[workflow] patch applied'              # after intent save — stage → generate_and_open_pr
grep 'generate_and_open_pr'                  # after PR open — stage should STAY
grep '[workflow] validator_pass advanced'  # after macro validator pass
grep 'missingRequired'                     # negative — validator fail without prUrl
```

---

## Related

- [README.md](README.md)
- [../purchase-requisition/walkthrough.md](../purchase-requisition/walkthrough.md) — split YAML form wizard
- [../../workflows/workflow-agent-task.md](../../workflows/workflow-agent-task.md)
