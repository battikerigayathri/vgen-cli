# Jira read flow — annotated walkthrough

**Reference sample** — paths below are under `examples/jira/`. Copy patterns into workspace-root `tools/`, `agents/`, etc. when building a real use case.

Field-by-field guide to every artifact in this sample. Operational commands stay in [README.md](README.md).

Production PM assistants may add KB routing, search, and write flows — this repo ships **read-only HITL + FAAS** only.

## What this demonstrates

- **HITL two-step** — JS HITLConfig card tool, then FAAS action tool ([decision-matrix.md](../../.cursor/skills/vgen-use-case/docs/decision-matrix.md) Pattern 1)
- **Adaptive card** — `${issueKey}` placeholder substituted in the HITLConfig handler
- **JS + FAAS mix** — `queryRecords` polyfill vs Smriti secrets + axios
- **Agent never-rules** — no FAAS before HITL submit, no same-turn HITL + action
- **Single-agent assistant** — routes read/summarise intents to jira-agent

## Read flow (two turns)

```
Turn 1: User "Summarise CGA-123"
        → assistant → jira-agent → Jira Read Issue HITLConfig → card shown

Turn 2: User submits form (issueKey)
        → resume chat message (see below) → jira-agent parses issueKey
        → jira-agent → jira-read-issue → summary to user
```

### The resume chat message (exact format)

There is **no special resume API** — the frontend posts the submitted form data back as a normal chat message. See [hitl-resume-and-message-format.md](../../.cursor/skills/vgen-use-case/docs/hitl-resume-and-message-format.md) for the full contract. For this sample, submitting the card with `issueKey: CGA-909` produces:

```text
This is my data, please process further
issueKey: CGA-909
```

**What `jira-agent` does on receipt**: `context.input` is **not** auto-filled from the card. The agent planner receives this raw text and must recognize the `key: value` pattern, extract `issueKey`, and call `jira-read-issue` with that value as the argument — it must **not** re-invoke `jira-read-issue-hitlconfig`. `jira-agent.yaml`'s step-2 instruction ("After read HITL submit, call jira-read-issue with issueKey from form") relies on this parsing behavior; if you need the agent to be more explicit about the resume-message shape, add a line like: *"On resume, the user message starts with 'This is my data, please process further' followed by `key: value` lines — extract `issueKey` from those lines."*

## ID linking

All `id` fields below are left empty (`id: ""`) in this sample on purpose — never hand-author or copy-paste a MongoDB ObjectId. Push each layer to get a real, platform-assigned ID, then wire it into the dependent per [push-pull-wire.md](../../.cursor/skills/vgen-use-case/docs/push-pull-wire.md).

| Artifact | ID / key | Links to |
|----------|----------|----------|
| HITL record | slug `jira-read-issue-confirm` | Fetched by HITLConfig handler |
| HITLConfig tool | `<written-back after push>` | Agent skill 1 |
| FAAS read tool | `<written-back after push>` | Agent skill 2 |
| jira-agent | `<written-back after push>` | Assistant agent |
| Assistant | `<written-back after push>` | Entry point |

---

## HITL record — `hitl/jira-read-issue-confirm/`

### meta.yaml

| Field | Value | Purpose |
|-------|-------|---------|
| `slug` | `jira-read-issue-confirm` | HITLConfig handler queries by this |
| `preMessage` | Enter issue key… | Shown before card |
| `postMessage` | Issue key submitted… | Shown after submit |

Push first: `vgen hitl push jira-read-issue-confirm`

### config.json

Adaptive card v1.5 with:

- **Body** — title, instructions, `Input.Text` with `id: issueKey`
- **`value`: `${issueKey}`** — pre-fill placeholder; handler substitutes before UI render ([hitl/meta-and-config.md](../../hitl/meta-and-config.md))
- **Actions** — Submit (`action: submit`) and Cancel

Form field `id` becomes the key in the next-turn `context.input` for `jira-read-issue`.

---

## HITLConfig tool — `tools/jira-read-issue-hitlconfig/`

### tool.yaml

- `type: JS` — no `package.json`
- `name` ends with **`HITLConfig`**
- Optional `issueKey` argument for agent pre-fill
- `systemInstructions`: call before `jira-read-issue`; never same turn as action tool

### handler.js

1. Read optional `context.input.issueKey`
2. `await queryRecords([{ collectionName: "hitlConfig", query: { slug } }])`
3. Parse `record.config`, replace `${issueKey}` in JSON string
4. `return JSON.stringify({ success, config, preMessage, postMessage, values })`

See [js-handlers.md](../../tools/js-handlers.md) and [hitl/hitl-config-tools.md](../../hitl/hitl-config-tools.md).

Push: `vgen tool push jira-read-issue-hitlconfig`

---

## FAAS action tool — `tools/jira-read-issue/`

### tool.yaml

- `type: FAAS`, `functionId` required for CLI test
- `systemInstructions`: ONLY after HITL submit; never on first read request

### handler.js

- `async function handler(event)` — [faas-handlers.md](../../tools/faas-handlers.md)
- Reads `event.context.input.issueKey`
- Builds secret key: `auth-token-${assigned_agent}-${roc_user_session.id}`
- `smriti.secrets.get` + Basic auth from session email
- `axios.get` Jira REST API; returns **object** with `agentResponseContext`

### package.json

`type: module`, `axios` dependency — see [examples/jira/tools/jira-read-issue/package.json](tools/jira-read-issue/package.json).

### payload.json

Realistic `event.context` for `vgen tool test` — session id, email, `assigned_agent`.

Push: `vgen tool push jira-read-issue`

Test: `vgen tool test jira-read-issue`

---

## Agent — `agents/jira-agent.yaml`

| Field | Notes |
|-------|-------|
| `skills` | Exactly two tool IDs (HITLConfig + FAAS read) |
| `systemInstructions` | Step 1 HITLConfig, step 2 FAAS; never-rules |
| `slug` | `jira-agent` — used in secret key and FaaS context |

Push after both tools: `vgen agent push jira-agent`

---

## Assistant — `assistants/jira-assistants.yaml`

| Field | Notes |
|-------|-------|
| `agents` | Single ID — jira-agent only |
| `systemContext` | Route read/summarise to jira-agent; describe HITL-first flow |
| `guardrailsContext` | Allow read HITL + FAAS; reject writes |

Optional test: create `assistants/jira-assistants/prompt.json`:

```json
{
  "sessionId": "",
  "question": "Summarise Jira issue CGA-909",
  "attachments": []
}
```

Then `vgen assistant test jira-assistants` — see [cli/test.md](../../cli/test.md).

Push last: `vgen assistant push jira-assistants`

---

## Example chat

1. User: *"Summarise CGA-909"* → card appears
2. User submits issue key → frontend posts the resume message shown above → jira-agent extracts `issueKey` → FAAS fetch → assistant summarises fields from tool output

---

## Related

- [README.md](README.md) — push order, cwd, file map
- [../minimal/README.md](../minimal/README.md) — starter sample without HITL
- [../../cli/authoring-checklist.md](../../cli/authoring-checklist.md)
