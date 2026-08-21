# Tool Specification & Schema Reference

This document defines the complete schema and execution contracts for ResMate Tools (`tool.yaml`). It covers the differences between lightweight JS (V8 Isolate) handlers and full-featured FaaS (Node/Lambda) handlers, payload contracts, platform polyfills, and the Layer 2 field selection (`feeds`) protocol.

---

## 1. `tool.yaml` Schema Reference

Every ResMate tool is defined in a directory under `tools/<tool-folder>/` containing a `tool.yaml` manifest.

### Complete Field Reference

| Field | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `id` | `string` | After first push | Stable MongoDB identifier for the tool. Leave empty or omit during initial creation; the platform populates this on the first `vgen tool push`. |
| `name` | `string` | Yes | Display name of the tool. Becomes the stable **`skill_key`** at runtime. For HITL card configuration tools, this **must** end with the suffix **`HITLConfig`**. |
| `description` | `string` | Yes | A clear, concise description of what the tool does, exposed to the agent planner's tool catalog. |
| `type` | `string` | Yes | The execution runtime environment. Must be either `JS` (V8 Isolate) or `FAAS` (Node/Lambda). |
| `input_modes` | `array` | Yes | Supported input MIME types. Typically `["text", "application/json"]`. |
| `output_modes` | `array` | Yes | Supported output MIME types. Typically `["application/json"]`. |
| `systemInstructions` | `array` | Yes | An array of strings defining when the agent planner should call the tool, its input/output contract, and safety constraints. Must be a YAML list of strings, not a multiline scalar. |
| `arguments` | `object` | Yes | Schema defining the parameters the agent planner must resolve before invoking the tool. Supports flat legacy and canonical JSON Schema. |
| `output` | `object` | No | Optional JSON Schema defining the structure of the tool's return payload. Can include `x-feeds` hints. |
| `feeds` | `object` | No | Layer 2 field selection mapping. Defines which fields from the tool result are exposed to specific prompt consumers. |
| `functionId` | `string` | FAAS only | UUID of the deployed FaaS function. Required for local testing via `vgen tool test`. |
| `version` | `string` | Recommended | Version identifier (e.g., `'1.0'`). Required by the API for publishing. |
| `createdBy` | `string` | No | Author identifier or email. |
| `managedBy` | `array` | No | List of managing entities or teams. |
| `tags` | `array` | No | List of organizational tags. |
| `examples` | `array` | No | Example prompts illustrating tool invocation. |

### systemInstructions Array

The `systemInstructions` field must be a YAML list of strings, not a multiline scalar. It aligns with agent step rules and states when the tool is callable in the flow:

```yaml
systemInstructions:
  - "ONLY call AFTER the user submits the HITL form with issueKey."
  - "NEVER call on first read request — call Jira Read Issue HITLConfig instead."
  - "Returns success, issueKey, summary, and agentResponseContext."
```

---

## 2. Input Arguments Schema (`arguments`)

The argument resolver uses the `arguments` (or `parameters` / `inputArguments`) block to validate parameters before Kriya executes the tool.

### 2.1 Flat Legacy Schema
Common in existing tools for simple flat parameters:

```yaml
arguments:
  issueKey:
    type: string
    required: true
    description: "The Jira issue key (e.g., JIRA-123)."
  comment:
    type: string
    required: false
    description: "Optional comment text to append."
```

### 2.2 Canonical JSON Schema (Preferred)
Recommended for new tools, especially those accepting complex nested objects or arrays:

```yaml
arguments:
  type: object
  properties:
    vendorId:
      type: integer
      description: "The unique numeric identifier of the vendor."
    items:
      type: array
      items:
        type: object
        properties:
          partNumber:
            type: string
          quantity:
            type: integer
        required:
          - partNumber
          - quantity
  required:
    - vendorId
    - items
```

---

## 3. JS Handlers (V8 Isolate Runtime)

Choose `type: JS` for fast, lightweight, and self-contained scripts that run within Kriya's secure V8 isolate without external npm dependencies.

### 3.1 Execution Budgets & Constraints
- **Execution Budget**: Must execute in **<500ms**.
- **No Function Wrapper**: Do not wrap your script in a function or module export. The script is executed as top-level code.
- **Input Binding**: The resolved arguments are injected into the global **`context.input`** object.
- **Output Contract**: The script **must return a stringified JSON string** (using `JSON.stringify`), not a plain JavaScript object.
- **Error Handling**: Always wrap the script in a top-level `try/catch` block to guarantee a clean failure payload.

### 3.2 Gold Standard JS Handler Template

```javascript
try {
  const input = context?.input || {};
  const vendorId = input.vendorId;

  if (!vendorId) {
    return JSON.stringify({
      success: false,
      error: "vendorId is required",
      agentResponseContext: "Ask the user to provide a valid vendor ID."
    });
  }

  // Call platform polyfill for database query
  const records = queryRecords([{
    collectionName: "vendors",
    query: { id: parseInt(vendorId, 10) }
  }]);

  if (!records || records.length === 0) {
    return JSON.stringify({
      success: false,
      error: `Vendor ${vendorId} not found`,
      agentResponseContext: "Inform the user that the vendor could not be found in the database."
    });
  }

  return JSON.stringify({
    success: true,
    vendor: records[0],
    agentResponseContext: "Present the vendor details to the user."
  });
} catch (err) {
  return JSON.stringify({
    success: false,
    error: err?.message || String(err)
  });
}
```

### 3.3 V8 Platform Polyfills (Globals Reference)

The Kriya V8 isolate injects several global helper functions (polyfills) to interact with platform services:

| Polyfill | Signature | Description | Deep Doc |
| :--- | :--- | :--- | :--- |
| `queryRecords` | `queryRecords(queries: Array<{ collectionName: string, query: object }>) => Array<object>` | Queries the Smriti database. | `kriya/docs/query_records_polyfill.md` |
| `getSecret` | `getSecret({ key: string }) => string` | Retrieves a secret from AWS Secrets Manager. | `docs/get_secret_polyfill.md` |
| `publishStatus` | `publishStatus(message: string) => void` | Streams a live progress status update to the user UI. | `publish-status.md` |
| `askLLM` | `askLLM({ prompt: string, model?: string }) => string` | Invokes an LLM directly from the handler. | `docs/v8.md` |
| `askLLMStructuredOutput` | `askLLMStructuredOutput({ prompt: string, jsonSchema: object, model?: string }) => object` | Invokes an LLM and guarantees a JSON response matching the schema. | `docs/v8.md` |
| `generateEmbedding` | `generateEmbedding({ text: string }) => Array<number>` | Generates text embeddings. | `docs/generate_embedding_polyfill.md` |
| `hybridVectorSearch` | `hybridVectorSearch({ collection: string, text: string, vector: Array<number>, limit?: number }) => Array<object>` | Performs hybrid vector + keyword search. | `docs/hybrid_vector_search_polyfill.md` |
| `vectorQuery` | `vectorQuery(...)` | Vector similarity query. | `context/links.md` |
| `Redis` | `Redis.get(key: string) => string`, `Redis.set(key: string, val: string, expireSec?: number) => void` | Lightweight Redis key-value store access. | `docs/redis_store_polyfill.md` |
| `console` | `console.log(...)` / `console.error(...)` | Logging to stdout/stderr. | `docs/v8.md` |
| `fetch` | `fetch(url, options)` | Standard HTTP requests. | `docs/v8.md` |
| `Promise` | Standard JS Promise | Async patterns. | `docs/v8.md` |

### 3.4 HITLConfig Pattern
A typical flow for a HITLConfig JS tool:
1. Read slug from `context.input` or use a fixed slug for the flow.
2. Fetch the config using `await queryRecords([{ collectionName: "hitlConfig", query: { slug } }])`.
3. Unwrap the single record using the **dual-unwrap pattern** — `queryRecords` returns a nested 2D array (array of query results, each an array of records), and some code paths wrap that in a `{ data: [...] }` envelope:
   ```javascript
   const result = await queryRecords([{ collectionName: "hitlConfig", query: { slug } }]);
   const record = result?.data?.[0]?.[0] ?? result?.[0]?.[0];
   ```
4. Substitute `${placeholders}` in the record's `config` JSON.
5. Return stringified `{ success, config, preMessage, postMessage, values? }`.

Full rationale, the common flat-array bug, and secrets/error-shape parity live in [docs/sdk-response-patterns.md](sdk-response-patterns.md) — read it before authoring any handler that calls `queryRecords`. For what happens after the card is submitted (resume message format, `context.input` caveats, Save Tool contract), see [docs/hitl-resume-and-message-format.md](hitl-resume-and-message-format.md).

### 3.5 JS Anti-Patterns

| Don't | Do instead |
| :--- | :--- |
| Wrap in `function handler()` or `async function` | Write a top-level `try/catch` script |
| `require(...)` / `import ...` / `module.exports = ...` | Call platform globals directly (`queryRecords`, `getSecret`) — no module loader exists in the V8 isolate |
| `return { success: true }` (object) | `return JSON.stringify({ success: true })` |
| Skip `try/catch` | Always catch and return `{ success: false, error }` stringified |
| Call action tool before HITL submit | Call HITLConfig first to present the form |
| **High severity:** `const record = result.data[0]` (or `result[0]`) used as if it were a single record | This is an **array of records** for that query, not a record — every field access silently reads `undefined`. Use the dual-unwrap pattern: `result?.data?.[0]?.[0] ?? result?.[0]?.[0]` (see [sdk-response-patterns.md §1](sdk-response-patterns.md#1-queryrecords--the-dual-unwrap-pattern)) |
| `queryRecords([{ collectionName: "hitlConfigs", ... }])` (plural) | The collection is singular: `"hitlConfig"`. Plural silently returns zero records. |

---

## 4. FaaS Handlers (Node/Lambda Runtime)

Choose `type: FAAS` when your tool requires external npm packages (e.g., `axios`, cloud SDKs), needs to make long-running external API calls, or requires session-scoped credentials.

### 4.1 Execution Budgets & Constraints
- **Execution Budget**: Budgets up to **30 seconds**.
- **File Layout**: Requires a `package.json` with `"type": "module"` and a `handler.js` exporting an async function.
- **Handler Contract**: Export an async function named `handler` that receives an `event` object.
- **Input Binding**: Resolved arguments arrive at `event.context.input`.
- **Output Contract**: Return a **plain JavaScript object** (do NOT stringify).

### 4.2 File Layout
```text
tools/<tool>/
├── tool.yaml
├── handler.js
├── package.json       # required: "type": "module"
└── payload.json       # optional; for vgen tool test
```

### 4.3 package.json Structure
A minimum `package.json` is required for FAAS tools:

```json
{
  "name": "my-tool",
  "version": "1.0.0",
  "type": "module",
  "main": "handler.js",
  "dependencies": {
    "axios": "^1.5.0"
  }
}
```

### 4.4 Gold Standard FaaS Handler Template

```javascript
import axios from "axios";
import { smriti } from "/runtime/runtime-sdks/smriti.js";
import { kriya } from "/runtime/runtime-sdks/kriya.js";

export async function handler(event) {
  try {
    const context = event?.context || {};
    const input = context.input || {};
    const { issueKey } = input;

    if (!issueKey) {
      return {
        success: false,
        error: "issueKey is required",
        agentResponseContext: "Ask the user for the Jira issue key."
      };
    }

    // Stream live progress status update
    await kriya.process.publishStatus("Fetching issue details from Jira...");

    // Retrieve session-scoped credential — FaaS secrets return { data: { value } }
    const sessionUser = context.roc_user_session || {};
    const secretKey = `jira-token-${context.assigned_agent}-${sessionUser.id}`;
    const secretData = await smriti.secrets.get({ key: secretKey, servicetype: "AwsSecretsManager" });
    const apiToken = secretData?.data?.value;

    if (!apiToken) {
      return {
        success: false,
        error: "Jira API credential not found for this session.",
        agentResponseContext: "The Jira API credential is missing. Ensure the secret exists."
      };
    }

    const response = await axios.get(`https://your-domain.atlassian.net/rest/api/3/issue/${issueKey}`, {
      headers: {
        Authorization: `Bearer ${apiToken}`,
        Accept: "application/json"
      }
    });

    return {
      success: true,
      issueKey,
      summary: response.data.fields.summary,
      status: response.data.fields.status.name,
      agentResponseContext: "Summarize the retrieved Jira issue details for the user."
    };
  } catch (err) {
    return {
      success: false,
      error: err?.response?.data?.errorMessages?.[0] || err?.message || String(err)
    };
  }
}
```

### 4.5 FaaS SDK Imports
FaaS functions run in a sandbox where specialized platform SDKs are mounted at `/runtime/runtime-sdks/`:

- **Smriti SDK**: `import { smriti } from "/runtime/runtime-sdks/smriti.js";`
  - Used for retrieving session-scoped credentials: `smriti.secrets.get(key)`.
- **Kriya SDK**: `import { kriya } from "/runtime/runtime-sdks/kriya.js";`
  - Used for streaming progress: `kriya.process.publishStatus(msg)`.
- **Prajna SDK**: `import { prajna } from "/runtime/runtime-sdks/prajna.js";`
  - Used for core framework integration and platform-specific APIs.

### 4.6 FaaS vs JS Polyfill Equivalents

| JS Polyfill | FaaS SDK Equivalent |
| :--- | :--- |
| `getSecret` | `smriti.secrets.get()` |
| `publishStatus` | `kriya.process.publishStatus()` |
| `queryRecords` | `prajna` / `kriya` modules per `runtime-sdks/README.md` |

### 4.7 FaaS Testing
Local testing is supported via the CLI:
```bash
vgen tool test <tool-folder>
```
This command uses `payload.json` to simulate realistic `event.context` inputs.

---

## 5. Payload Contracts & Layer 2 Field Selection (`feeds`)

To optimize the LLM's context window and prevent "context bloat" from massive JSON payloads, ResMate implements a Layer 2 field selection protocol. Instead of dumping the entire raw JSON output of a tool into the prompt, developers declare exactly which fields are needed by which consumer.

### 5.1 Prompt Consumers
The platform supports four distinct prompt consumers:
- **`planner`**: Fields exposed to the agent planner ReAct loop to decide the next action.
- **`compose`**: Fields exposed to the assistant composer to synthesize the final user reply.
- **`compose_verbatim`**: Fields that must be passed to the composer completely unmodified.
- **`macro_validator`**: Fields exposed to the macro validator for state and policy attestation checks.

### 5.2 Declaring `feeds` at the Root Level
You can declare a top-level `feeds` block in `tool.yaml`:

```yaml
feeds:
  planner:
    - success
    - vendor.id
    - vendor.status
  compose:
    - success
    - vendor.id
    - vendor.name
    - vendor.address
    - agentResponseContext
  macro_validator:
    - vendor.status
```

### 5.3 Declaring `x-feeds` inside the `output` Schema
Alternatively, you can embed `x-feeds` (or `x_feeds`) directly inside the JSON Schema of the `output` block:

```yaml
output:
  type: object
  properties:
    success:
      type: boolean
    vendor:
      type: object
      properties:
        id:
          type: integer
        name:
          type: string
        status:
          type: string
  x-feeds:
    planner:
      - success
      - vendor
    compose:
      - success
      - vendor
```

### 5.4 Validation Rules & Lints
The `vgen validate` command runs strict lints against the `feeds` and `x-feeds` blocks:
1. **`TOOL_FEEDS_INVALID` (Error)**: Raised if the `feeds` block contains invalid consumer keys, empty field lists, or duplicate entries. Field names must be alphanumeric and can only contain underscores or dot paths.
2. **`TOOL_FEEDS_MISSING` (Warning)**: Raised if a tool defines an `output` schema but lacks both `feeds` and `x-feeds` blocks. In this case, the platform falls back to injecting the entire tool payload, which increases token costs and risks context dilution.
3. **Verbatim Consistency**: Any field listed in `compose_verbatim` should usually also be declared in `compose`.
4. **Schema Alignment**: If `x-feeds` is used inside `output`, every field listed in the consumer arrays **must** exist in the `output.properties` definition.

---

## 6. Workflow-Bound Save Tools (`workflowPatch`)

When an assistant is bound to a Smriti Workflow, certain tools are designated as **Save Tools** (e.g., `*-save`, `*-confirm`). Instead of executing external side effects, these tools patch the active workflow's state.

### 6.1 State Ownership
Prajna owns the runtime workflow state. Save tools must **never** attempt to write directly to the database or return an instruction to advance the stage. They must only return a `workflowPatch` object. The platform evaluates the stage's `doneWhen` condition in `flow.yaml` to handle transitions.

### 6.2 Return Shape for JS Save Tools
JS save tools must return a stringified JSON containing `workflowPatch` nested under `inputs` (for collect/review stages) or `artifacts` (for agent task outputs):

```javascript
return JSON.stringify({
  success: true,
  workflowPatch: {
    inputs: {
      deliveryOrg: context.input.orgName,
      supplierName: context.input.supplier
    }
  },
  agentResponseContext: "Saved delivery organization and supplier to workflow inputs."
});
```

### 6.3 Return Shape for FaaS Save Tools
FaaS save tools return a plain object with the same structural shape:

```javascript
return {
  success: true,
  workflowPatch: {
    artifacts: {
      purchaseRequisitionUrl: "https://oracle.erp.internal/pr/908123"
    }
  },
  agentResponseContext: "Oracle PR created and saved to workflow artifacts."
};
```

### 6.4 Key Alignment Rule
All keys inside `workflowPatch.inputs` or `workflowPatch.artifacts` **must align exactly** with the field keys defined in the workflow's `schema.yaml` and the corresponding HITL `Input.*` field IDs.
