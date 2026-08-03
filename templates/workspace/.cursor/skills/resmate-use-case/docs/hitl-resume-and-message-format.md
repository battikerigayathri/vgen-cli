# HITL Resume & Message Format Cookbook

This cookbook documents the **exact runtime contract** for how a Human-in-the-Loop (HITL) form submission reaches the agent, and how the agent must respond. Misunderstanding this contract is the leading cause of workflows that "hang" after a user submits a form — the planner never sees structured form data because there is no such thing; it only ever sees a chat message.

---

## 1. The Halt-and-Resume Flow

1. The agent planner invokes a **HITLConfig** tool (a tool whose `name` ends in `HITLConfig`). The tool returns an Adaptive Card configuration (`config`), plus optional `preMessage`/`postMessage`/`values`.
2. The platform renders the card to the user and **halts** the turn with a `hitl_required` status — no further planner turns run until the user responds.
3. The user fills out the form in the frontend and clicks **Submit**.

At no point does the agent or tool "wait" on a callback — the halt is a terminal state for that turn.

---

## 2. Resume as Chat Message (CRITICAL)

**There is no special resume API.** The frontend does not call a distinct "submit form" endpoint that routes straight into a tool. Instead:

- The frontend packages the submitted form field values into **plain text** and posts it back to the assistant as a completely ordinary **chat message** (the same channel as if the user had typed a sentence).
- From the planner's point of view, this looks exactly like a new user question — it is a "question" turn, not a "form submission" turn.

### 2.1 Standard Message Format

```text
This is my data, please process further
vendorId: 1002
justification: Urgent production order
```

- **Preamble line**: A fixed or near-fixed lead-in sentence (e.g. `This is my data, please process further`). Treat the exact wording as informative, not load-bearing — agents should recognize the *pattern* (a preamble followed by `key: value` lines), not match the preamble string verbatim.
- **Key-Value Lines**: Every submitted form field appears on its own line as `key: value`, where `key` matches the HITL form's `Input.*` field `id` (which in turn matches the `hitlFieldId` / schema `key` — see [spec-workflow.md §5.3](spec-workflow.md#53-field-alignment-checklist)).

### 2.2 Multi-Field Example

```text
This is my data, please process further
issueKey: CGA-909
comment: Please prioritize — customer-facing bug
```

---

## 3. The `context.input` Caveat (CRITICAL)

**`context.input` is NOT automatically populated with the submitted form fields on resume.** This is the single most common source of confusion:

- The agent planner receives the **raw chat message text** shown in §2, exactly as a user would type it.
- The planner must parse the key-value lines out of that text — either by:
  - Instructing the agent (via `systemInstructions` / `agentBrief`) to extract each expected field from the incoming message and pass it as a tool argument, or
  - Having the Save Tool itself defensively parse `context.input` fields that the argument resolver populated from the parsed message.
- Do **not** write a Save Tool handler that assumes `context.input.vendorId` will "just be there" because a form with a `vendorId` field was submitted. The argument resolver (Phase 7.5) does the parsing work to populate `context.input`, but only if the tool's `arguments` schema declares the expected fields and the agent's instructions correctly route the resume message to that tool.

### 3.1 Agent Instruction Pattern

Agents bound to a HITL-resume tool should have an explicit `systemInstructions` (or workflow `agentBrief`) entry similar to:

```yaml
systemInstructions:
  - "When the user's message begins with 'This is my data, please process further' followed by key: value lines, treat this as a HITL form resume."
  - "Extract each key: value pair from the message and call the matching Save Tool with those values as arguments — do NOT re-invoke the HITLConfig tool."
  - "NEVER call the HITLConfig tool again in the same turn a resume message is received."
```

---

## 4. Guardrails

- **Never re-show the card on resume.** If the incoming message matches the resume pattern, the agent must route directly to the Save Tool, not back to the HITLConfig tool.
- **Never call the terminal/action tool before HITL submit.** The HITLConfig tool must run first, in an earlier turn, to present the form — action or save tools must not run speculatively before a resume message is observed.
- **Never assume field presence.** Because parsing depends on the argument resolver and agent instructions, Save Tool handlers must still validate that required fields are non-empty before proceeding (see the gold handler templates in [spec-tool.md](spec-tool.md)).
- **Never leak the raw resume message to the user verbatim.** Use `agentResponseContext` to guide how the agent should summarize the outcome instead of echoing the key-value block back.

---

## 5. Workflow Save After Submit

Once the resume message has been parsed and the relevant fields are available to a tool, the agent must invoke a **Save Tool** (`type: JS` or `type: FAAS`, typically named `*-save` or `*-confirm`).

### 5.1 Contract

The Save Tool's only job is to return a `workflowPatch` describing which fields to update — it must **never** attempt to advance the stage itself or write to a database directly. State ownership belongs to the platform (Prajna); see [spec-workflow.md §5](spec-workflow.md#5-state-save-tools--workflow-schema-mapping).

**JS Save Tool** (stringified JSON):

```javascript
try {
  const input = context?.input || {};
  const vendorId = input.vendorId;
  const justification = input.justification;

  if (!vendorId) {
    return JSON.stringify({
      success: false,
      error: "vendorId is required",
      agentResponseContext: "Ask the user to resubmit the form with a vendor ID.",
    });
  }

  return JSON.stringify({
    success: true,
    workflowPatch: {
      inputs: {
        vendorId: vendorId,
        justification: justification || "",
      },
    },
    agentResponseContext: "Vendor and justification saved. Platform will auto-advance when doneWhen is satisfied.",
  });
} catch (err) {
  return JSON.stringify({ success: false, error: err?.message || String(err) });
}
```

**FaaS Save Tool** (plain object):

```javascript
export async function handler(event) {
  try {
    const input = event?.context?.input || {};
    if (!input.vendorId) {
      return { success: false, error: "vendorId is required" };
    }

    return {
      success: true,
      workflowPatch: {
        inputs: { vendorId: input.vendorId, justification: input.justification || "" },
      },
    };
  } catch (err) {
    return { success: false, error: err?.message || String(err) };
  }
}
```

### 5.2 Auto-Advance

Once the platform applies the `workflowPatch`, it re-evaluates the active stage's `doneWhen` condition:

- If `doneWhen` is an array of field keys and all are now present, the platform **automatically advances** the workflow to the stage's `next` (or matching `transitions[].target`) — no tool or agent action is required to trigger the hop.
- If `doneWhen` is `"validator_pass"`, the patch is applied but the stage stays active until the Macro Validator asserts `requirement_met`.

See [spec-workflow.md §4.2](spec-workflow.md#42-transition-boundaries-donewhen) for the full auto-advance semantics.

---

## 6. End-to-End Example

```text
Turn 1 (HITLConfig):
  Agent calls "Purchase Req Vendor HITLConfig" → returns Adaptive Card config.
  Platform renders card, halts with hitl_required.

Turn 2 (Resume message):
  User submits form → frontend posts chat message:
    "This is my data, please process further
     vendorId: 1002
     justification: Urgent production order"

  Agent planner recognizes the resume pattern (per systemInstructions),
  extracts vendorId and justification, and calls "Purchase Req Vendor Save".

  Save Tool returns:
    { "success": true, "workflowPatch": { "inputs": { "vendorId": "1002", "justification": "Urgent production order" } } }

  Platform applies patch. doneWhen: [vendorId] is satisfied → stage auto-advances
  to collect_line_items. Agent composes a confirmation reply to the user.
```

---

## 7. Related Docs

- [sdk-response-patterns.md](sdk-response-patterns.md) — dual-unwrap pattern for fetching the HITL config record itself, secrets access, and error shapes.
- [spec-tool.md §3.4](spec-tool.md#34-hitlconfig-pattern) — HITLConfig tool implementation pattern.
- [spec-workflow.md §5](spec-workflow.md#5-state-save-tools--workflow-schema-mapping) — full Save Tool return conventions and field alignment checklist.
- [spec-workflow.md §7 Recipe 1](spec-workflow.md#recipe-1-standard-workflow-form-wizard-collect--review--terminal) — full form-wizard golden path.
