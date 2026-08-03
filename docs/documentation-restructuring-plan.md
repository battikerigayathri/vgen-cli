# ResMate Knowledge Base Restructuring and Update Plan (v2)

This document details the comprehensive restructuring plan for the ResMate developer and agent knowledge base inside the `resmed_resmate-cli` repository. By transitioning from a single massive `SKILL.md` to the **Master Router & Index** pattern, we resolve the dual problems of "cognitive fragmentation" (where agents traverse dozens of scattered files) and "context bloat" (where loading a single massive file consumes excessive token budgets and degrades performance).

---

## 1. Executive Summary & Architectural Vision

### The Problem with Single Massive Files
While consolidating all documentation into a single `SKILL.md` solves fragmentation, it introduces:
1. **Context Window Exhaustion**: AI agents are forced to read the entire multi-thousand-line file even when they only need a simple CLI flag or a single YAML field specification.
2. **Attention Dilution**: Large context blocks lead to "needle-in-a-haystack" retrieval failures, causing agents to miss critical constraints or wiring rules.
3. **Maintenance Overhead**: Merging diverse topics (CLI, YAML specs, execution models, decision matrices) into one file makes it difficult for multiple developers to update specifications without merge conflicts.

### The Solution: Master Router & Index
We propose a clean, modular subfolder structure under the Cursor Agent Skill `templates/workspace/.cursor/skills/resmate-use-case/`:
- **`SKILL.md`**: A lightweight, high-density entry point that acts as the Master Router, Index, and Decision Guide. It provides high-level routing rules and maps developer/agent intents to specific sub-documents.
- **`docs/`**: A consolidated subfolder containing on-demand, highly specialized reference files. This keeps the workspace root completely clean and allows the agent to selectively load only the relevant file for its current subtask, optimizing context window usage and token efficiency.

### Target Directory Tree
```text
templates/workspace/.cursor/skills/resmate-use-case/
├── SKILL.md                       # Master Router, Index, and Decision Guide
└── docs/                          # Consolidated, on-demand reference files
    ├── decision-matrix.md         # Single vs. Multi-Agent, Standard vs. Workflow, JS vs. FaaS
    ├── spec-tool.md               # tool.yaml, JS handlers, FaaS handlers, payload contracts
    ├── spec-agent.md              # agent.yaml, instructions
    ├── spec-assistant.md          # assistant.yaml, decoupled workflows
    ├── spec-workflow.md           # flow.yaml, schema.yaml, playbooks.yaml
    ├── cli-commands.md            # push, pull, validate, doctor, graph, diff, tool test, assistant chat, env push
    ├── env-secrets.md             # env variables, .env, resmate.yaml mapping, write-only secrets push policy
    └── examples-guide.md          # reference examples in examples/
```

---

## 2. High-Fidelity Specifications for the 9 Restructured Files

### I. `SKILL.md` (The Master Router, Index, and Decision Guide)
- **Role**: The lightweight entry point for the agent. It contains metadata and a quick-routing index mapping common developer intents to the specific sub-documents in `docs/`.
- **Content Outline**:
  - Skill name, description, and metadata.
  - **Quick-Routing Index**: A markdown table mapping intents to files.
  - **High-Level Decision Guide**: A summary of the architectural choices (Single vs. Multi-Agent, Standard vs. Workflow, JS vs. FaaS) with links to `docs/decision-matrix.md` for full details.

#### Quick-Routing Index Table
| Developer/Agent Intent | Target Document | Description |
| :--- | :--- | :--- |
| "I need to choose between Single/Multi-Agent, Standard/Workflow, or JS/FaaS" | `docs/decision-matrix.md` | Architectural comparison tables, decision trees, and trade-offs. |
| "I am writing a tool schema or handler script" | `docs/spec-tool.md` | `tool.yaml` schema, JS V8 vs. FaaS Node runtimes, and payload contracts. |
| "I am writing agent instructions or binding tools" | `docs/spec-agent.md` | `agent.yaml` schema, system instructions design, and tool binding syntax. |
| "I am writing assistant routing or decoupled workflows" | `docs/spec-assistant.md` | `assistant.yaml` schema, decoupled workflows array, and compatibility fallback. |
| "I am writing a workflow flow, schema, or stage playbook" | `docs/spec-workflow.md` | `flow.yaml`, `schema.yaml`, `playbooks.yaml` schemas, and state save tool contracts. |
| "I need to run, test, validate, or push my use case" | `docs/cli-commands.md` | Complete reference for all CLI commands, including `tool test`, `assistant chat`, and `env push`. |
| "I need to manage environment variables or secrets" | `docs/env-secrets.md` | `.env` file usage, `resmate.yaml` mapping, and write-only secrets push policy. |
| "I want to explore reference examples" | `docs/examples-guide.md` | Mapping of available examples and how they demonstrate specific architectural choices. |

---

### II. `docs/decision-matrix.md`
- **Role**: High-density architectural comparison tables and decision trees.
- **Content Outline**:
  - **Single vs. Multi-Agent**: Context-switching limits, authorization boundaries, task complexity, routing overhead.
  - **Standard Flow vs. Business Workflow**: Ephemeral vs. persistent state (Smriti database, `session_id`), unstructured vs. structured stages, static vs. stage-specific reply policies, basic vs. immutable audit ledger logging, automated hooks.
  - **JS (V8 Isolate) vs. FaaS (Node/Lambda)**: External npm package dependencies, execution timeouts (<500ms vs 30s), Smriti secrets access, cold start performance.

#### Standard Flow vs. Business Workflow Decision Tree
```text
               Is there a formal business process
             with mandatory states/stages & audit?
                        /            \
                       /              \
                     YES               NO
                     /                  \
        Use Business Workflow         Use Standard Flow
      (flow.yaml + playbooks.yaml)    (System Context only)
```

---

### III. `docs/spec-tool.md`
- **Role**: How to write `tool.yaml`, JS handlers, FaaS handlers, and payload contracts.
- **Content Outline**:
  - Complete `tool.yaml` schema with fields: `id`, `name`, `description`, `type` (JS vs FAAS), `input_schema`, `output_schema`.
  - **JS Handlers (V8 Isolate)**: Self-contained scripts, execution budgets (<500ms), and platform polyfills (`getSecret`, `queryRecords`).
  - **FaaS Handlers (Node/Lambda)**: Multi-file support, `package.json` dependencies, and execution budgets (up to 30s).
  - **Payload Contracts**: Input/output schemas, and validation rules.

#### Example JS Handler with Polyfills
```javascript
// tools/vendor-lookup/handler.js
const { getSecret, queryRecords } = require('resmate-polyfills');

module.exports = async function(context) {
  const apiKey = await getSecret('VENDOR_API_KEY');
  const { vendorId } = context.input;
  
  const records = await queryRecords('vendors', { id: vendorId });
  if (records.length === 0) {
    return { success: false, error: 'Vendor not found' };
  }
  return { success: true, vendor: records[0] };
};
```

---

### IV. `docs/spec-agent.md`
- **Role**: How to write `agent.yaml` and system instructions.
- **Content Outline**:
  - Complete `agent.yaml` schema with fields: `id`, `name`, `slug`, `description`, `instructions`, `skills` (Tool IDs).
  - **System Instructions Design**: Formatting, best practices, and avoiding prompt dilution.
  - **Tool Binding Syntax**: The `skills` array referencing Tool IDs.

#### Example `agent.yaml`
```yaml
id: "6a354b8f4fffc3946d12c573"
name: "Oracle PR Agent"
slug: "oracle-pr-agent"
description: "Specialized agent for extracting and saving Oracle Purchase Requisitions."
instructions: |
  You are a specialized Oracle PR Agent. Your primary responsibility is to extract line items
  from purchase requisition documents and save them to the system.
  
  Follow these guidelines:
  1. Use the oracle-pr-quote-extract-save tool to process uploaded quotes.
  2. Ensure all extracted fields are validated against the workflow schema.
skills:
  - "oracle-pr-quote-extract-save"
  - "oracle-pr-header-save"
```

---

### V. `docs/spec-assistant.md`
- **Role**: How to write `assistant.yaml` and decoupled workflows.
- **Content Outline**:
  - Complete `assistant.yaml` schema with fields: `id`, `name`, `slug`, `status`, `agents` (Agent IDs), `workflows` (Decoupled Workflows list), `systemContext` (System instructions).
  - **Decoupled Workflows Block**: Mapping of `workflows` array with `slug`, `purpose`, and `trigger_intents`.
  - **Ambient Mode vs. Active Playbook Mode**: Transition triggers and execution boundaries.
  - **Compatibility Bridge**: The dual-parsing architecture for legacy Orchestration Contracts under `systemContext`.

#### Example `assistant.yaml` with Decoupled Workflows
```yaml
id: "6a354b8e4fffc3946d12c572"
name: "Oracle PR Assistant"
slug: "oracle-pr-assistant"
status: "active"
agents:
  - "6a354b8f4fffc3946d12c573"
workflows:
  - slug: "oracle-purchase-requisition-v1"
    purpose: "Handles purchase requisitions, approvals, and line-item extraction."
    trigger_intents:
      - "create a new purchase requisition"
      - "open an oracle PR"
      - "approve requisition"
```

---

### VI. `docs/spec-workflow.md`
- **Role**: How to write `flow.yaml`, `schema.yaml`, and `playbooks.yaml`.
- **Content Outline**:
  - **Split Folder Layout**: `meta.yaml`, `schema.yaml`, `flow.yaml`, and `playbooks.yaml`.
  - **Stage Transitions & Execution States**: `collect`, `agent_task`, `review`, `terminal` stages, and transition boundaries (`doneWhen`).
  - **Stage-Level Playbooks (`playbooks.yaml`)**: Complete schema and runtime behavior of `playbooks.yaml` (stage-level playbooks, `replyPolicy`, `constraints`, `assistantBrief`, `agentBrief`, `automatedHooks` like mutate/notify/webhook, and `validatorRules`).
  - **State Save Tools to Workflow Schema**: Save tools returning `workflowPatch` and mapping to the workflow schema and HITL form.

#### Example `playbooks.yaml`
```yaml
stage_playbooks:
  - stage_id: collect_vendor
    replyPolicy:
      tone: "highly_professional"
      wordLimit: 150
      instructions:
        - "Prompt the user politely for their Vendor ID if missing."
    constraints:
      - "Never execute vendor-lookup-tool unless a numeric vendorId is supplied."
    assistantBrief: |
      You are helping the user compile vendor information. Avoid routing to alternative agents
      until the vendor validation passes.
    agentBrief: |
      Use the vendor-lookup-tool to check the input supplied by the user.
    automatedHooks:
      onEntry:
        - action: notify
          payload:
            channel: "internal"
            message: "Now entering vendor collection phase."
```

---

### VII. `docs/cli-commands.md`
- **Role**: Complete reference for all CLI commands.
- **Content Outline**:
  - Standard commands: `push`, `pull`, `validate`, `doctor`, `graph`, `diff`.
  - **`resmate tool test <tool-folder>`**: Local JS V8 simulation, flags (`--payload`, `--integration`, `--session`, `--skill-id`, `--no-pull`), color-coded console output (Cyan logs, Green duration, Magenta outcome).
  - **`resmate assistant chat <assistant-basename>`**: Multi-turn interactive test console (REPL), flags (`--new-session`), dual-channel transport (WebSocket for streaming/thoughts, HTTP POST for state/sync), console commands (`/status` with ASCII progress, `/help`, `/quit`), history (`~/.resmate/repl_history.txt`) and debug logging (`.resmate/debug/sessions/<session_id>.json`).
  - **`resmate env push`**: Compile-deploys secrets.

#### `resmate tool test` Output Format
- **Logs (Cyan)**: Plaintext output from `console.log()` statements and system polyfill tracers.
- **Duration (Green)**: Total runtime latency of the handler.
- **Outcome (Magenta)**: Pretty-printed JSON returned by the script.

---

### VIII. `docs/env-secrets.md`
- **Role**: How environment variables, `.env`, and `resmate.yaml` mapping work, and the write-only secrets push policy.
- **Content Outline**:
  - Local environment variables and `.env` file usage.
  - **`env_mappings` section of `resmate.yaml`**: mapping config with fields: `local_key`, `remote_key`, `scope`, `agent_interpolation`.
  - **Dynamic Interpolation**: Replacing `${assigned_agent}` with active agent slugs.
  - **Write-Only Secrets Push Policy**: Smriti enforces a strict write-only policy (no GET endpoint, success hashes and modified key listings).

#### Example `env_mappings` Configuration
```yaml
env_mappings:
  - local_key: JIRA_PASSWORD
    remote_key: JIRA_API_TOKEN
    scope: agents
    agent_interpolation: true
```

---

### IX. `docs/examples-guide.md`
- **Role**: How reference examples in `examples/` help the agent understand different situations.
- **Content Outline**:
  - Mapping of available examples (e.g., `oracle-pr`, `minimal`, `form-wizard`) and how they demonstrate specific architectural choices (e.g., multi-agent, workflow-bound, JS vs FaaS).

---

## 3. Updates to CLI Scaffolding (`resmate init` and `scaffold`)

To ensure that when a developer initializes a workspace, it scaffolds this clean folder structure (with all documentation tucked away inside `.cursor/skills/resmate-use-case/docs/`, keeping the workspace root clean).

### Scaffolding Mechanics
1. **Directory Structure**:
   The CLI `resmate init` command copies the workspace kit from `templates/workspace/` to the target workspace root.
   By moving the documentation files under `templates/workspace/.cursor/skills/resmate-use-case/docs/` in the CLI repository, the `copy_workspace_kit` function will automatically copy this structure to the developer's workspace under `.cursor/skills/resmate-use-case/docs/`.
2. **Code Updates**:
   - `src/kit/copy.rs` and `src/kit/load.rs` already recursively copy directories under `templates/workspace/`.
   - We will update the seed files and template directories in `templates/workspace/` to remove any scattered markdown files from the workspace root (like `AGENTS.md` or `links.md` if they are no longer needed there, or keep them as minimal redirection stubs pointing to `.cursor/skills/resmate-use-case/SKILL.md`).
   - Specifically, we will update `templates/workspace/AGENTS.md` to be a lightweight redirection stub pointing to `.cursor/skills/resmate-use-case/SKILL.md`.
3. **Scaffolding Flow**:
   ```text
   resmate init --name my-project
   ┌─────────────────────────────────────────────────────────┐
   │ 1. Create directory structure                           │
   │ 2. Copy resmate.yaml & .env templates                   │
   │ 3. Copy .cursor/skills/resmate-use-case/SKILL.md        │
   │ 4. Copy .cursor/skills/resmate-use-case/docs/*.md       │
   └─────────────────────────────────────────────────────────┘
   ```

---

## 4. Parallelization Plan for Subagents

To execute this heavy lifting efficiently, we propose a parallelization plan to distribute the writing of these 8 markdown files across multiple specialized subagents.

### Subagent Roles and Responsibilities

| Subagent | Responsibility | Inputs | Outputs |
| :--- | :--- | :--- | :--- |
| **Subagent A: CLI, Environment & Secrets Specialist** | Authoring of CLI commands, environment variables, secrets management, and examples guide. | CLI command specifications, `dotenvy` integration, Smriti secrets API contracts. | `docs/cli-commands.md`<br>`docs/env-secrets.md`<br>`docs/examples-guide.md` |
| **Subagent B: Specs & YAMLs Specialist** | Authoring of Tool, Agent, and Assistant specifications, schemas, validation rules, and wiring contracts. | YAML schemas, V8 Isolate polyfills, legacy vs decoupled assistant schemas. | `docs/spec-tool.md`<br>`docs/spec-agent.md`<br>`docs/spec-assistant.md` |
| **Subagent C: Workflows & Playbooks Specialist** | Authoring of Workflow schemas, state machine transitions, stage-level playbooks, and architectural decision matrices. | `flow.yaml`, `schema.yaml`, `playbooks.yaml` specs, Smriti state machine documentation. | `docs/spec-workflow.md`<br>`docs/decision-matrix.md` |
| **Subagent D (Lead/Coordinator): Master Router & Scaffolding Integrator** | Authoring of the Master Router `SKILL.md`, updating CLI templates, and verifying referential integrity across all files. | Restructured folder layout, subagent outputs. | `SKILL.md`<br>CLI scaffolding updates<br>Redirection stubs |

### Execution & Synchronization Checkpoints
1. **Phase 1: Initialization & Schema Alignment (Day 1)**: Lead Subagent (D) scaffolds the directory structure and defines the precise markdown templates and cross-linking anchors.
2. **Phase 2: Parallel Writing (Day 1-2)**: Subagents A, B, and C write their respective documents in parallel.
3. **Phase 3: Cross-Reference & Validation (Day 3)**: Lead Subagent (D) runs validation checks to ensure all links and anchors are valid, and verifies that the CLI scaffolding copies the files correctly.
