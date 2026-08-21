# SDK Response Patterns: queryRecords, Secrets, and Error Shapes

This cookbook is the authoritative reference for how ResMate runtime handlers must read platform SDK responses. Most runtime bugs in authored tools come from **assuming a flat shape** where the platform actually returns a nested or wrapped shape. Read this before writing any handler that calls `queryRecords`, fetches a secret, or returns an error.

---

## 1. `queryRecords` — The Dual-Unwrap Pattern

### 1.1 Shape Contract

`queryRecords` accepts an **array of queries** and returns a **nested 2D array**: an array of query results, where each query result is itself an array of matching records.

```javascript
// Input: an array of one or more queries
const result = await queryRecords([
  { collectionName: "hitlConfig", query: { slug: "purchase-req-vendor-form" } },
]);

// Conceptual shape of `result` (unwrapped):
// [
//   [ { slug: "...", config: "...", preMessage: "...", postMessage: "..." } ]  // results for query[0]
// ]
```

Some platform code paths return the array directly; others wrap it in a `{ data: [...] }` envelope. **Handlers must not assume either shape exclusively** — they must safely handle both.

### 1.2 The Dual-Unwrap Pattern (canonical)

To safely fetch the **first record of the first query**, always use:

```javascript
const result = await queryRecords([{ collectionName: "hitlConfig", query: { slug } }]);
const record = result?.data?.[0]?.[0] ?? result?.[0]?.[0];
```

Read this as: "try the wrapped shape (`result.data[0][0]`) first; if that's `undefined`/`null`, fall back to the unwrapped shape (`result[0][0]`)."

This single line replaces multi-line defensive branching and is the **only** approved pattern for unwrapping a single record from `queryRecords` in this kit's docs, gold snippets, and examples.

### 1.3 The Common Bug — Flat-Array Access

The most frequent runtime bug is treating the *query result* (an array of records) as if it were *the record itself*:

```javascript
// ❌ BUG: this is the array of records for query[0], not a record.
const records = result && result.data ? result.data : [];
const record = Array.isArray(records) && records.length > 0 ? records[0] : null;
// `record` here is actually `result.data[0]`, i.e. still an ARRAY of records —
// every field access like `record.config` will be `undefined`.
```

```javascript
// ❌ BUG: same problem without the `.data` envelope.
const record = result[0]; // this is an array of records, not a record
```

Both examples silently produce a truthy-but-wrong `record` (an array), so `record.config` reads as `undefined`, and downstream code fails in confusing ways — often only surfacing in production once the array happens to be empty or non-empty in different environments. Always index **twice**: once for the query position, once for the record position within that query's results.

### 1.4 Collection Name Is Singular: `"hitlConfig"`

The collection storing HITL card configurations is named **`hitlConfig`** (singular), not `hitlConfigs`. Using the plural silently returns zero records instead of raising an error — always double-check the collection name against this doc when copy-pasting.

```javascript
// ✅ Correct
await queryRecords([{ collectionName: "hitlConfig", query: { slug } }]);

// ❌ Wrong — plural collection name returns no records, no error
await queryRecords([{ collectionName: "hitlConfigs", query: { slug } }]);
```

### 1.5 Full Worked Example

```javascript
try {
  const input = context?.input || {};
  const slug = "purchase-req-vendor-form";

  const result = await queryRecords([{ collectionName: "hitlConfig", query: { slug } }]);
  const record = result?.data?.[0]?.[0] ?? result?.[0]?.[0];

  if (!record) {
    return JSON.stringify({
      success: false,
      error: "HITL record not found for slug: " + slug,
      agentResponseContext: "Push hitl/purchase-req-vendor-form before using this tool.",
    });
  }

  const configObj = typeof record.config === "string" ? JSON.parse(record.config) : record.config || {};

  return JSON.stringify({
    success: true,
    config: JSON.stringify(configObj),
    preMessage: record.preMessage || "",
    postMessage: record.postMessage || "",
    values: {},
    agentResponseContext: "Show the vendor collect card.",
  });
} catch (err) {
  return JSON.stringify({ success: false, error: err?.message || String(err) });
}
```

---

## 2. Secrets Access Divergence — JS Sandbox vs FaaS

Secret retrieval **uses a different function signature and a different return shape** depending on the runtime. Mixing these up is a top source of "works locally, fails at runtime" bugs.

### 2.1 JS Sandbox (`type: JS`)

- **Signature**: `getSecret(key: string) => { success: boolean, value?: string, error?: string }`
- **Access value via**: `.value`, only after checking `.success`.

```javascript
const res = getSecret("MY_SECRET_KEY");
if (!res.success) {
  return JSON.stringify({ success: false, error: "Secret not found: " + res.error });
}
const secretVal = res.value;
```

### 2.2 FaaS Runtime (`type: FAAS`)

- **Signature**: `smriti.secrets.get({ key: string, servicetype: string }) => Promise<{ data: { value: string } }>` (or throws on error).
- **Access value via**: `secretData?.data?.value` — note the extra `.data` envelope, mirroring the `queryRecords` wrapped shape.

```javascript
import { smriti } from "/runtime/runtime-sdks/smriti.js";

const secretData = await smriti.secrets.get({
  key: secretKey,
  servicetype: "AwsSecretsManager",
});
const secretVal = secretData?.data?.value;

if (!secretVal) {
  return {
    success: false,
    error: "Secret not found for key: " + secretKey,
  };
}
```

### 2.3 Side-by-Side Comparison

| Aspect | JS Sandbox | FaaS Runtime |
| :--- | :--- | :--- |
| Function | `getSecret(key)` | `await smriti.secrets.get({ key, servicetype })` |
| Async? | No (synchronous polyfill) | Yes (`await` required) |
| Success shape | `{ success: true, value: "..." }` | `{ data: { value: "..." } }` |
| Failure shape | `{ success: false, error: "..." }` | Throws, or returns without `.data.value` — always guard with `?.` |
| Value access | `res.value` (after checking `res.success`) | `secretData?.data?.value` |

**Never** call `smriti.secrets.get()` from a JS sandbox tool, and never call bare `getSecret()` from a FaaS handler — the globals/imports are runtime-specific and will not resolve.

---

## 3. Error Shape Parity

The **error return contract also diverges by runtime** — this is intentional and enforced by the platform's handler-invocation layer.

### 3.1 JS Sandbox — Stringified JSON

JS sandbox handlers must **always** return a `JSON.stringify`'d string containing `success` and `error` fields:

```javascript
return JSON.stringify({ success: false, error: "Detailed error message" });
```

Returning a plain object (`return { success: false, error: "..." }`) from a JS sandbox tool is a bug — the platform expects a string it can parse, not a live object.

### 3.2 FaaS Runtime — Plain Object

FaaS handlers must **always** return a plain JavaScript object — do **not** stringify:

```javascript
return { success: false, error: "Detailed error message" };
```

Returning `JSON.stringify({ ... })` from a FaaS handler is a bug — the platform will receive a string instead of the expected object and downstream field access (`result.success`) will fail.

### 3.3 Quick Reference

| Runtime | Success return | Error return |
| :--- | :--- | :--- |
| JS Sandbox | `return JSON.stringify({ success: true, ... })` | `return JSON.stringify({ success: false, error: "..." })` |
| FaaS | `return { success: true, ... }` | `return { success: false, error: "..." }` |

---

## 4. JS Anti-Patterns

These patterns will break at runtime inside the V8 isolate, even though they look like normal Node.js code:

| Don't | Why it breaks | Do instead |
| :--- | :--- | :--- |
| `require('vgen-polyfills')` or any `require(...)` / `import ...` | The V8 isolate has no module loader; polyfills (`queryRecords`, `getSecret`, etc.) are injected as **globals**, not importable modules. | Call the globals directly: `queryRecords(...)`, `getSecret(...)`. |
| `module.exports = async function(context) { ... }` or `export default ...` | The script is executed as **top-level code**, not as a module — there is no `module` object and no wrapper function is invoked. | Write a top-level `try { ... } catch (err) { ... }` script that reads `context.input` directly. |
| `const records = result.data[0]` used as if it were a single record | Yields an **array** of records (see §1.3), not a record — silent `undefined` field access downstream. | Use the dual-unwrap pattern: `result?.data?.[0]?.[0] ?? result?.[0]?.[0]`. |
| Omitting the top-level `try/catch` | An uncaught exception surfaces as an opaque platform-level failure instead of a clean `{ success: false, error }` payload the agent can react to. | Always wrap the entire script body in `try { ... } catch (err) { return JSON.stringify({ success: false, error: err?.message || String(err) }); }`. |
| `return { success: true }` (plain object) from a JS sandbox tool | The platform expects a string for JS tools; a raw object breaks downstream parsing. | `return JSON.stringify({ success: true })`. |

---

## 5. Related Docs

- [hitl-resume-and-message-format.md](hitl-resume-and-message-format.md) — what happens after a HITLConfig-returned card is submitted.
- [spec-tool.md](spec-tool.md) — full `tool.yaml` schema, gold JS/FaaS handler templates, and the `workflowPatch` contract.
- [spec-workflow.md](spec-workflow.md) — Save Tool return conventions and end-to-end workflow recipes.
- [decision-matrix.md](decision-matrix.md) — when to choose JS Sandbox vs FaaS for a new tool.
