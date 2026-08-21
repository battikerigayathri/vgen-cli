---
name: Init Phase 3 — SDK / FaaS / HITL Cookbooks
overview: Thicken the authoring kit with authoritative cookbooks and reference examples for SDK response patterns, secrets access, and HITL resume message contracts, ensuring developers and Cursor agents implement robust, bug-free runtime handlers.
todos:
  - id: sdk-response-patterns-doc
    content: "resmed_vgen-cli — Create templates/workspace/.cursor/skills/vgen-use-case/docs/sdk-response-patterns.md documenting queryRecords / HITL config, JS vs FaaS secrets, error shapes, anti-patterns, and the dual-unwrap pattern"
    status: pending
  - id: hitl-resume-message-doc
    content: "resmed_vgen-cli — Create templates/workspace/.cursor/skills/vgen-use-case/docs/hitl-resume-and-message-format.md documenting resume-as-chat-message, preamble text, key-value lines, context.input vs user_message, guardrails, and workflow save after submit"
    status: pending
  - id: spec-tool-doc-update
    content: "resmed_vgen-cli — Fix gold handler snippets in templates/workspace/.cursor/skills/vgen-use-case/docs/spec-tool.md to match the cookbook"
    status: pending
  - id: spec-workflow-doc-update
    content: "resmed_vgen-cli — Fix fake queryRecords / hitlConfigs samples in templates/workspace/.cursor/skills/vgen-use-case/docs/spec-workflow.md"
    status: pending
  - id: skill-index-update
    content: "resmed_vgen-cli — Update templates/workspace/.cursor/skills/vgen-use-case/SKILL.md to index both new docs"
    status: pending
  - id: examples-handler-fix
    content: "resmed_vgen-cli — Fix HITL config + secrets access paths in templates/workspace/examples/**/handler.js to use correct nested patterns"
    status: pending
  - id: examples-walkthrough-update
    content: "resmed_vgen-cli — Update walkthroughs under templates/workspace/examples/** to include sample resume messages and agent instructions"
    status: pending
isProject: false
---

# Phase 3 — SDK / FaaS / HITL Cookbooks

**Status:** planned  
**Repo (only):** `resmed_vgen-cli`  
**Parent plan:** `docs/INIT-AUTHORING-KIT-GAPS-PLAN.md` — Phase 3 / Gaps #6 & #7  
**Do not implement:** Phase 4+ (fuller kit packaging, CLI binary changes, other repos)

---

## Goal

Provide authoritative, high-fidelity cookbooks and reference examples within the scaffolded authoring kit so that developers and AI coding agents can implement robust runtime handlers. This eliminates common runtime bugs caused by incorrect nested data unwrapping, inconsistent secret fetching paths, and misunderstandings of the HITL resume-as-chat-message flow.

---

## Scope

| In scope (under `templates/workspace/`) | Out of scope |
|-----------------------------------------|--------------|
| `templates/workspace/.cursor/skills/vgen-use-case/docs/sdk-response-patterns.md` (NEW) | Changes in `teemo`, `pr-agent-v2`, `resmedai-core-framework` |
| `templates/workspace/.cursor/skills/vgen-use-case/docs/hitl-resume-and-message-format.md` (NEW) | CLI binary or Rust code changes |
| `templates/workspace/.cursor/skills/vgen-use-case/docs/spec-tool.md` (update snippets) | Phase 4+ (fuller kit packaging, Option B) |
| `templates/workspace/.cursor/skills/vgen-use-case/docs/spec-workflow.md` (update snippets) | Phase 5 (CLI-side push-validation warnings) |
| `templates/workspace/.cursor/skills/vgen-use-case/SKILL.md` (index updates) | |
| `templates/workspace/examples/**/handler.js` (bug fixes) | |
| `templates/workspace/examples/**/walkthrough.md` (walkthrough updates) | |

---

## Product decisions (locked for this plan)

Do not re-open these unless implementation reveals a hard conflict.

| # | Decision | Choice for Phase 3 |
|---|----------|--------------------|
| 1 | **Dual-Unwrap Pattern** | **`result?.data?.[0]?.[0] ?? result?.[0]?.[0]`** must be documented as the canonical way to fetch a single record from `queryRecords` (such as a `hitlConfig` record) to handle both wrapped and unwrapped platform response shapes safely. |
| 2 | **Secrets Access Divergence** | **JS Sandbox vs FaaS.** JS sandbox uses global `getSecret(key)` returning `{ success, value, error }` (access via `.value` when `.success` is true). FaaS uses `smriti.secrets.get({ key, servicetype })` returning `{ data: { value } }` (access via `secretData?.data?.value`). |
| 3 | **Error Shape Parity** | **JS Sandbox vs FaaS.** JS sandbox handlers must return a stringified JSON object containing `success` and `error` fields. FaaS handlers must return a plain JavaScript object. |
| 4 | **HITL Resume Contract** | **Resume-as-chat-message.** The frontend resumes a halted workflow by posting a normal chat message (preamble + key-value lines) rather than calling a special resume API. The planner/agent must parse this message, and `context.input` is NOT auto-filled from the form. |
| 5 | **Workflow Save After Submit** | **Workflow State Patching.** After a HITL form is submitted, the agent must run a Save Tool that returns a `workflowPatch` to update the inputs/state on the platform, which auto-advances the workflow stage. |

---

## Current baseline (verified)

- `SKILL.md` does not list or index any SDK response pattern or HITL resume cookbooks.
- No dedicated `sdk-response-patterns.md` or `hitl-resume-and-message-format.md` documents exist in the kit.
- `spec-tool.md` contains outdated or inconsistent snippets regarding secrets and database queries.
- `spec-workflow.md` contains fake, non-functional snippets using `require('vgen-polyfills')` and `module.exports = async function(context)` which violate the JS sandbox top-level execution model.
- Example handlers (e.g. `jira-read-issue-hitlconfig/handler.js`) contain bugs where they attempt to access `result.data` as a flat array of records rather than a nested 2D array, causing runtime failures.
- Example walkthroughs (e.g. Jira and Purchase Requisition) describe the multi-turn flow but omit the exact format of the resume chat message and how the agent should handle it.

---

## Implementation steps

### 1. Create SDK Response Patterns Cookbook — `templates/workspace/.cursor/skills/vgen-use-case/docs/sdk-response-patterns.md` (NEW)

Create a comprehensive guide detailing:
- **Database Queries (`queryRecords`)**:
  - Explain that `queryRecords` accepts an array of queries and returns a nested 2D array (an array of query results, where each result is an array of records).
  - Document the **Dual-Unwrap Pattern**:
    ```javascript
    const result = await queryRecords([{ collectionName: "hitlConfig", query: { slug } }]);
    const record = result?.data?.[0]?.[0] ?? result?.[0]?.[0];
    ```
  - Highlight the common bug of using `result.data[0]` or `result[0]` which yields an array of records rather than the record itself.
  - Document that the collection name for HITL configurations is singular: `"hitlConfig"`, not `"hitlConfigs"`.
- **Secrets Access Divergence**:
  - **JS Sandbox (type: JS)**:
    - Signature: `getSecret(key)`
    - Return shape: `{ success: boolean, value?: string, error?: string }`
    - Usage:
      ```javascript
      const res = getSecret("MY_SECRET_KEY");
      if (!res.success) {
        return JSON.stringify({ success: false, error: "Secret not found: " + res.error });
      }
      const secretVal = res.value;
      ```
  - **FaaS Runtime (type: FAAS)**:
    - Signature: `smriti.secrets.get({ key: string, servicetype: string })`
    - Return shape: `{ data: { value: string } }` (or throws on error)
    - Usage:
      ```javascript
      import { smriti } from "/runtime/runtime-sdks/smriti.js";
      // ...
      const secretData = await smriti.secrets.get({
        key: secretKey,
        servicetype: "AwsSecretsManager",
      });
      const secretVal = secretData?.data?.value;
      ```
- **Error Shapes**:
  - **JS Sandbox**: Must return a stringified JSON object:
    ```javascript
    return JSON.stringify({ success: false, error: "Detailed error message" });
    ```
  - **FaaS Runtime**: Must return a plain JavaScript object:
    ```javascript
    return { success: false, error: "Detailed error message" };
    ```
- **JS Anti-Patterns**:
  - Do not use `require` or `import` in JS sandbox tools (they run in a restricted V8 isolate with global polyfills).
  - Do not wrap JS sandbox scripts in `module.exports` or `export default`.
  - Always wrap the entire JS sandbox script in a `try/catch` block.

### 2. Create HITL Resume Cookbook — `templates/workspace/.cursor/skills/vgen-use-case/docs/hitl-resume-and-message-format.md` (NEW)

Create a detailed guide explaining the resume contract:
- **The Halt-and-Resume Flow**:
  1. The agent invokes a **HITLConfig** tool which returns the Adaptive Card config.
  2. The platform sends the card to the user and halts execution with a `hitl_required` status.
  3. The user fills out the form in the frontend and clicks submit.
- **Resume as Chat Message**:
  - Emphasize that there is **no special resume API** called by the agent.
  - The frontend automatically packages the submitted form data and posts it back to the planner as a **normal chat message** (a "question").
  - Document the standard message format:
    - **Preamble**: `This is my data, please process further` (or similar).
    - **Key-Value Lines**: Each form field is appended on a new line as `key: value`.
    - Example chat message:
      ```text
      This is my data, please process further
      vendorId: 1002
      justification: Urgent production order
      ```
- **The `context.input` Caveat**:
  - Explain that `context.input` is **NOT** automatically filled with the form fields on resume.
  - The agent planner receives the raw chat message. The agent must parse the message text (e.g., using system instructions or regex in a tool) or the assistant must be instructed to extract these fields.
- **Workflow Save After Submit**:
  - Explain that once the submitted data is received, the agent must invoke a **Save Tool** (type: JS or FAAS).
  - The Save Tool must return a `workflowPatch` to update the workflow instance's inputs:
    ```javascript
    return JSON.stringify({
      success: true,
      workflowPatch: {
        inputs: {
          vendorId: parsedVendorId,
          justification: parsedJustification
        }
      }
    });
    ```
  - The platform saves these inputs and automatically advances the workflow stage if the stage's `doneWhen` criteria are met.

### 3. Update Tool Spec — `templates/workspace/.cursor/skills/vgen-use-case/docs/spec-tool.md`

- Update Section 3.4 (HITLConfig Pattern) to use the dual-unwrap pattern:
  ```javascript
  const result = await queryRecords([{ collectionName: "hitlConfig", query: { slug } }]);
  const record = result?.data?.[0]?.[0] ?? result?.[0]?.[0];
  ```
- Update Section 3.5 (JS Anti-Patterns) to include the `result.data[0]` flat-array access as a high-severity anti-pattern.
- Ensure all gold snippets in `spec-tool.md` match the secrets and error shapes documented in the new cookbooks.

### 4. Update Workflow Spec — `templates/workspace/.cursor/skills/vgen-use-case/docs/spec-workflow.md`

- Locate and replace all fake/non-functional snippets (such as those around lines 918 and 1115):
  - Remove `require('vgen-polyfills')` and `module.exports` wrappers.
  - Rewrite them as standard, top-level JS sandbox scripts returning stringified JSON.
  - Correct the `queryRecords` calls to use the array-of-objects query signature and the singular `"hitlConfig"` collection name.
  - Apply the dual-unwrap pattern to retrieve the records safely.

### 5. Update Skill Router — `templates/workspace/.cursor/skills/vgen-use-case/SKILL.md`

- Add two new rows to the Quick-Routing Index table:
  - `docs/sdk-response-patterns.md`: "SDK Response Patterns" — Safe queryRecords unwrapping, JS vs FaaS secrets, and error shapes.
  - `docs/hitl-resume-and-message-format.md`: "HITL Resume & Message Format" — Resume-as-chat-message contract, preamble parsing, and workflow patching.
- Add a short summary of these contracts in Section 4 (or a new Section 5) to act as a quick-reference card for agents.

### 6. Fix Example Handlers — `templates/workspace/examples/**/handler.js`

Scan and update all example handler scripts to resolve bugs and align with the cookbooks:
- **Jira Example**:
  - `jira-read-issue-hitlconfig/handler.js`: Fix the `queryRecords` unwrapping logic to use the dual-unwrap pattern and access `.config` correctly.
  - `jira-read-issue/handler.js`: Ensure the FaaS secret retrieval matches `secretData?.data?.value`.
- **Purchase Requisition Example**:
  - Fix all HITLConfig handlers (`purchase-req-vendor-hitlconfig`, `purchase-req-lines-hitlconfig`, `purchase-req-review-hitlconfig`) to use the dual-unwrap pattern.
  - Fix all Save handlers (`purchase-req-vendor-save`, `purchase-req-lines-save`, `purchase-req-review-confirm`) to return stringified JSON with correct `workflowPatch` structures.
- **GitHub PR Create Example**:
  - Fix `github-pr-intent-hitlconfig/handler.js` and `github-pr-intent-save/handler.js` to use the correct patterns.

### 7. Update Example Walkthroughs — `templates/workspace/examples/**/walkthrough.md`

Update the walkthrough files for `jira`, `purchase-requisition`, and `github-pr-create`:
- Expand the "Multi-turn flow" sections to show the exact text of the resume chat message sent by the frontend.
- Explain what the agent planner does upon receiving the resume message (how it parses the key-value lines and routes them to the next tool).
- Document the role of the Save Tool in patching the workflow state.

---

## Acceptance / exit criteria

- [ ] The new files `sdk-response-patterns.md` and `hitl-resume-and-message-format.md` are present in the workspace's skill docs folder.
- [ ] `SKILL.md` successfully indexes both new documents.
- [ ] All gold snippets in `spec-tool.md` and `spec-workflow.md` are corrected and match the cookbook contracts.
- [ ] Every example `handler.js` under `examples/` is bug-free, using the dual-unwrap pattern for `queryRecords` and the correct secrets paths.
- [ ] Walkthrough files contain sample resume chat messages and clear agent parsing instructions.
- [ ] Running `vgen kit update` on an existing workspace successfully delivers all of these updated and new files.

---

## Test plan

1. **Verify Template Generation**:
   - Run `vgen init` in a temporary directory.
   - Verify that all new and modified files are generated correctly.
   - Check that `sdk-response-patterns.md` and `hitl-resume-and-message-format.md` exist and contain the correct content.
2. **Verify Snippet Integrity**:
   - Open the temporary workspace in Cursor.
   - Verify that all relative links in `SKILL.md` resolve correctly to the new files.
   - Verify that no snippets in `spec-tool.md` or `spec-workflow.md` contain `require('vgen-polyfills')` or flat-array `result.data[0]` accesses.
3. **Verify Example Handlers**:
   - Inspect the generated `examples/` directory.
   - Ensure all `handler.js` files use the dual-unwrap pattern: `result?.data?.[0]?.[0] ?? result?.[0]?.[0]`.
   - Ensure FaaS handlers use `secretData?.data?.value` and JS sandbox handlers use `getSecret(key).value`.
4. **Verify Kit Update Delivery**:
   - Create a mock ResMate workspace with an older kit version.
   - Run `vgen kit update`.
   - Verify that the new cookbooks and updated examples are successfully written to the workspace.
