---
name: resmate-use-case
description: Develop or modify ResMate use cases — create tools, agents, assistants, HITL; design for v4 assistant planner and agent planner ReAct loop; wire by ID; push/pull with CLI.
---

# ResMate Use Case Development Skill

This document serves as the **Master Router, Index, and Decision Guide** for both human developers and AI coding agents authoring ResMate use cases. 

All comprehensive reference material, schemas, and CLI commands are consolidated in the `docs/` subfolder to prevent context window exhaustion and attention dilution, allowing agents to selectively load only the relevant file for their current subtask.

---

## 1. Quick-Routing Index

Use this index to quickly locate the specific reference document matching your current intent.

| Developer/Agent Intent | Target Document | Description |
| :--- | :--- | :--- |
| "I need to choose between Single/Multi-Agent, Standard/Workflow, or JS/FaaS" | [docs/decision-matrix.md](docs/decision-matrix.md) | Architectural comparison tables, decision trees, conversational patterns, and anti-patterns. |
| "I am writing a tool schema or handler script" | [docs/spec-tool.md](docs/spec-tool.md) | `tool.yaml` schema, JS V8 vs. FaaS Node runtimes, and payload contracts. |
| "I am writing agent instructions or binding tools" | [docs/spec-agent.md](docs/spec-agent.md) | `agent.yaml` schema, system instructions design, and tool binding syntax. |
| "I am writing assistant routing or decoupled workflows" | [docs/spec-assistant.md](docs/spec-assistant.md) | `assistant.yaml` schema, decoupled workflows array, and compatibility fallback. |
| "I am writing a workflow flow, schema, or stage playbook" | [docs/spec-workflow.md](docs/spec-workflow.md) | `flow.yaml`, `schema.yaml`, `playbooks.yaml` schemas, and state save tool contracts. |
| "I'm calling `queryRecords`, fetching secrets, or shaping tool errors" | [docs/sdk-response-patterns.md](docs/sdk-response-patterns.md) | Safe `queryRecords` unwrapping (dual-unwrap pattern), JS vs FaaS secrets access, and JS vs FaaS error shapes. |
| "I need to understand how a HITL form submission reaches my agent" | [docs/hitl-resume-and-message-format.md](docs/hitl-resume-and-message-format.md) | HITL Resume & Message Format — resume-as-chat-message contract, preamble/key-value parsing, and workflow patching after submit. |
| "I need to run, test, validate, or push my use case" | [docs/cli-commands.md](docs/cli-commands.md) | Complete reference for all CLI commands, including `tool test`, `assistant chat`, and `env push`. |
| "I need to manage environment variables or secrets" | [docs/env-secrets.md](docs/env-secrets.md) | `.env` file usage, `resmate.yaml` mapping, and write-only secrets push policy. |
| "I want to explore reference examples" | [docs/examples-guide.md](docs/examples-guide.md) | Mapping of available examples and how they demonstrate specific architectural choices. |
| "I'm not sure how IDs get assigned, or my push failed with 404/403" | [docs/id-lifecycle.md](docs/id-lifecycle.md) | ID Lifecycle (Never Invent) — omit -> push -> write-back lifecycle and anti-patterns. |
| "I need to know what order to push in or how to wire IDs between layers" | [docs/push-pull-wire.md](docs/push-pull-wire.md) | Push Order & ID Wiring — chronological push order, binding rules (slug vs Mongo id), and `push-all` limits. |

---

## 2. High-Level Decision Guide

Before writing any code, you must align on three core architectural decisions. For a comprehensive analysis, decision trees, and sequence flows, refer to the [Architectural Decision Matrix](docs/decision-matrix.md).

### 2.1 Single-Agent vs. Multi-Agent Orchestration
*   **Single-Agent**: Best for straightforward, linear tasks. It minimizes routing overhead and token costs.
*   **Multi-Agent**: Best when tasks require distinct domains of expertise, have complex branching logic, or require strict authorization boundaries. It prevents prompt dilution by keeping individual agent instructions focused.

### 2.2 Standard Flow vs. Business Workflow
*   **Standard Flow**: Best for ephemeral, stateless Q&A or single-turn lookups. State is managed dynamically in the conversation context.
*   **Business Workflow**: Mandatory when there is a formal business process with structured stages (e.g., `collect`, `review`, `terminal`), persistent state schemas, stage-specific reply policies, automated entry/exit hooks, or an immutable audit ledger.

### 2.3 JS Sandbox (V8 Isolate) vs. FaaS (Node/Lambda)
*   **JS Sandbox**: Best for high-performance, stateless scripts. Executes in `<500ms` with access to platform polyfills (`getSecret`, `queryRecords`), but has no external npm package support.
*   **FaaS Node**: Best for complex integrations requiring external npm packages or long-running tasks. Supports up to `30s` execution timeouts, but incurs cold-start latency.

---

## 3. Core Development Conventions

To ensure successful compilation and deployment, follow these strict development conventions:

1.  **Workspace Root Execution**: Always run all `resmate` CLI commands from the workspace root.
2.  **Chronological Push Order**: When deploying resources, you must push them in dependency order:
    $$\text{HITL Form} \rightarrow \text{Workflow} \rightarrow \text{Tool} \rightarrow \text{Agent} \rightarrow \text{Assistant}$$
3.  **Strict Validation**: Always run `resmate validate` and resolve all errors and warnings before pushing.
4.  **No Placeholders**: Never use placeholder values or draft schemas. All YAML and handler files must be fully specified.

---

## 4. ID Lifecycle & Deployment Wiring (CRITICAL)

To prevent deployment failures and broken platform references, you must strictly follow these rules. Full detail lives in [docs/id-lifecycle.md](docs/id-lifecycle.md) and [docs/push-pull-wire.md](docs/push-pull-wire.md) — read them before your first push.

1. **Never Invent MongoDB ObjectIds**: Never write or guess a 24-character hex ID locally.
2. **Omit -> Push -> Write-Back**:
   - Leave the `id` field empty or omitted in your local YAML.
   - Push the resource to the platform (`resmate <type> push <name>`).
   - The platform generates the ID and the CLI writes it back to your local YAML.
3. **Chronological Push & Wire Checklist**:
   - Push **HITL Forms** and **Workflows** first (bound via slugs).
   - Push **Tools** next -> copy written-back Tool IDs into Agent `skills[]`.
   - Push **Agents** next -> copy written-back Agent IDs into Assistant `agents[]`.
   - Push **Assistants** last.
   - Re-push any dependent layer whose YAML you just hand-edited to wire in an ID — `push-all` does not do this for you.

---

## 5. Runtime Contracts Quick-Reference (CRITICAL)

Before writing or debugging a handler, internalize these two runtime contracts — most authoring bugs come from violating one of them. Full detail in [docs/sdk-response-patterns.md](docs/sdk-response-patterns.md) and [docs/hitl-resume-and-message-format.md](docs/hitl-resume-and-message-format.md).

### 5.1 SDK Response Patterns

- **Dual-Unwrap `queryRecords`**: `queryRecords` returns a nested 2D array. Always fetch a single record with `result?.data?.[0]?.[0] ?? result?.[0]?.[0]` — never `result.data[0]` or `result[0]` (those are arrays of records, not a record).
- **Collection name is singular**: `"hitlConfig"`, not `"hitlConfigs"`.
- **Secrets diverge by runtime**: JS sandbox uses sync `getSecret(key)` -> `{ success, value, error }` (read `.value`). FaaS uses `await smriti.secrets.get({ key, servicetype })` -> `{ data: { value } }` (read `secretData?.data?.value`).
- **Error shapes diverge by runtime**: JS sandbox returns `JSON.stringify({ success: false, error })` (a string). FaaS returns a plain `{ success: false, error }` object (never stringified).

### 5.2 HITL Resume & Message Format

- **No resume API exists.** The frontend resumes a halted HITL form by posting a normal chat message: a preamble line followed by `key: value` lines for each submitted field.
- **`context.input` is NOT auto-filled on resume.** The agent must be instructed to parse the resume message's key-value lines and route them to the correct Save Tool.
- **Workflow Save After Submit**: the Save Tool must return only a `workflowPatch` (`{ inputs: {...} }` or `{ artifacts: {...} }`) — never an `advanceStage` instruction. The platform evaluates `doneWhen` and auto-advances the stage.
