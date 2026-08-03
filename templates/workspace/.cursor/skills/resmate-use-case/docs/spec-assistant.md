# Assistant Specification & Schema Reference

This document defines the complete schema, dynamic orchestration lifecycle, and routing contracts for ResMate Assistants (`assistant.yaml`). Assistants are high-level orchestrators governed by the **v4 assistant planner**; they coordinate user interactions, delegate tasks to specialized agents, manage guardrails, and dynamically bind structured business workflows.

---

## 1. `assistant.yaml` Schema Reference

Every ResMate assistant is defined in a single YAML file named `assistants/<assistant-slug>.yaml` at the workspace root.

### Complete Field Reference

| Field | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `id` | `string` | After first push | Stable MongoDB identifier for the assistant. Leave empty or omit during initial creation; the platform populates this on the first `resmate assistant push`. |
| `name` | `string` | Yes | Display name of the assistant. |
| `slug` | `string` | Yes | Stable, unique slug identifier. |
| `description` | `string` | Yes | Short description of the assistant's scope and purpose. |
| `status` | `string` | Yes | The deployment status. Typically `active` or `draft`. |
| `visibility` | `string` | Yes | Access visibility. Must be `private` or `public`. |
| `agents` | `array` | Yes | List of **Agent Mongo IDs** (from `agents/*.yaml`) that this assistant is authorized to delegate to (authorization boundary). |
| `workflows` | `array` | No | List of first-class, decoupled workflow references supported by this assistant. |
| `systemContext` | `string` | Yes | Multiline string defining the assistant's routing table, cross-agent gates, scope, and tone. |
| `guardrailsContext` | `string` | Recommended | Instructions defining allowed and blocked operations. Must explicitly allow HITL submissions if using HITL. |
| `guardrailsViolationFallback` | `string` | Recommended | Friendly fallback message displayed to the user when a request is blocked by guardrails. |
| `owner` | `string` | No | Owner identifier. |
| `managedBy` | `array` | No | List of managing entities or teams. |
| `outcome_profile` | `string` | No | Part of the Orchestration Contract. Typically `narrative_synthesis`. |
| `deliverable_fields` | `array` | No | List of fields required for the final deliverable. |
| `tool_feeds` | `object` | No | Field selection overrides for specific tools. |
| `version` | `string` | Recommended | Version identifier (e.g., `'5.0'`). |

### Canonical Key Order

To ensure consistency and clean diffs, the platform serializes and reorders YAML keys in the following canonical order during push/pull operations:
1. `id`
2. `name`
3. `slug`
4. `description`
5. `status`
6. `visibility`
7. `agents`
8. `workflows`
9. `systemContext`
10. `guardrailsContext`
11. `guardrailsViolationFallback`
12. `owner`
13. `managedBy`
14. `outcome_profile`
15. `deliverable_fields`
16. `tool_feeds`

---

## 2. Decoupled Workflows Block

ResMate models business workflows as **first-class, decoupled properties** of the assistant. Instead of tightly binding an assistant to a single workflow definition, the assistant can reference multiple independent workflows.

### 2.1 Workflows Array Schema

The `workflows` block is a YAML array of objects, where each object defines:

| Property | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `slug` | `string` | Yes | The unique slug of the workflow definition (matching a folder name under `workflows/`). |
| `purpose` | `string` | Yes | A description of what the workflow achieves, used by the planner to match user intent. |
| `trigger_intents` | `array` | Yes | An array of user intent strings or phrases that should trigger and bind this workflow. |

### 2.2 YAML Snippet Example

```yaml
workflows:
  - slug: "oracle-purchase-requisition-v1"
    purpose: "Guide the user through raising or submitting an Oracle purchase requisition."
    trigger_intents:
      - "create a pr"
      - "new purchase requisition"
      - "raise a requisition"
      - "create a purchase requisition"
      - "quote upload flow"
```

---

## 3. Ambient Mode vs. Active Playbook Mode

The decoupled architecture allows the assistant to handle side-band conversations (such as general Q&A or looking up documentation) naturally, only instantiating a workflow when the user explicitly expresses intent to perform that workflow.

### 3.1 Dynamic Binding Lifecycle

```
                           +--------------------------+
                           |  Prajna Compose Turn     |
                           +------------+-------------+
                                        |
                                        v
                          +-------------+-------------+
                          | Is there an active        |
                          | workflow_id in Redis?     |
                          +-------------+-------------+
                                       / \
                                 Yes  /   \ No
                                     /     \
                                    v       v
         +-----------------------------+   +-----------------------------+
         | Preserve active workflow    |   | Does user query match       |
         | instance binding. Proceed   |   | trigger_intents of any      |
         | to Active Workflow prompting|   | registered workflows?       |
         +-----------------------------+   +--------------+--------------+
                                                         / \
                                                   Yes  /   \ No
                                                       /     \
                                                      v       v
                     +----------------------------------+   +--------------------+
                     | 1. Instantiate workflow via      |   | Stay in AMBIENT    |
                     |    smriti.get_or_create_workflow |   | mode. (No active   |
                     | 2. Bind workflow_id to Redis     |   | workflow prompt    |
                     | 3. Project stage snapshot        |   | block injected)    |
                     +----------------------------------+   +--------------------+
```

### 3.2 Prompt State A: Ambient Mode (No Active Workflow)

When no workflow is active, the assistant prompt remains clean and free of state machine clutter (such as missing fields or stage completion percentages). Instead, the orchestrator is presented with an index of **available workflows** to guide natural intent matching:

```text
=== AVAILABLE WORKFLOWS ===
The following structured workflows are supported by this assistant and can be initiated on intent match:

- slug: oracle-purchase-requisition-v1
  purpose: Guide the user through raising or submitting an Oracle purchase requisition.
  trigger_intents: ["create a pr", "new purchase requisition", "raise a requisition", "quote upload flow"]
=== END AVAILABLE WORKFLOWS ===
```

### 3.3 Prompt State B: Active Playbook Mode (Workflow Bound)

Once a workflow is dynamically bound, the system formats and injects the active stage, its playbook rules, and constraints (as defined in `playbooks.yaml`):

```text
=== ACTIVE WORKFLOW ===
workflowId: 6a425ca1df630d3001fe2ad1
workflowType: oracle_purchase_requisition
stage: collect_requester

=== ACTIVE PLAYBOOK ===
Assistant Brief: The user is currently in the requester selection stage. You must determine the requester's ID.
Stage Execution Constraints:
- Never prompt user for supplier lookup until requester is saved.
Reply Policy:
- Tone: professional, concise, and helpful
- Word Limit: 120 words
=== END PLAYBOOK ===
=== END WORKFLOW ===
```

---

## 4. Compatibility Bridge (Dual-Parsing)

To support both legacy assistants and decoupled assistants simultaneously without breaking existing production environments, the platform implements a dual-mode **compatibility bridge**:

1. **First Priority (Decoupled)**: The platform looks for the `workflows` array in the assistant document. If present, it loads its trigger metadata.
2. **Second Priority (Legacy Fallback)**: If the `workflows` block is absent, the platform falls back to parsing the legacy `## Orchestration Contract` block inside `systemContext`. If found, it generates a single transient `AssistantWorkflowRef` with catch-all trigger intents to preserve legacy behavior.

### 4.1 Legacy Orchestration Contract Syntax

In legacy assistants, the workflow binding is declared inline inside `systemContext` as follows:

```yaml
systemContext: |
  You are ProqMate, a procurement assistant.
  
  ## Orchestration Contract
  workflow_type: oracle_purchase_requisition
  workflow_definition_slug: oracle-purchase-requisition-v1
```

*Note: In the decoupled architecture, `workflow_type` and `workflow_definition_slug` are completely removed from the Orchestration Contract, and the `workflows` array is used instead.*

---

## 5. Guardrails & HITL

The `guardrailsContext` defines the boundaries of allowed user operations. When using Human-In-The-Loop (HITL) forms, the guardrails **must explicitly allow HITL submission messages** so that the platform's security policy does not block form submissions.

### 5.1 Guardrails Configuration Example

```yaml
guardrailsContext: |
  You are authorized to assist the user with procurement operations.
  You are allowed to read purchase requisition details and save inputs.
  You are explicitly allowed to process HITL form submission messages containing structured input data.
  NEVER execute any payment or financial transaction directly.
guardrailsViolationFallback: "I am sorry, but I am not authorized to perform that operation. I can only assist with purchase requisition extraction and validation."
```

---

## 6. CLI Push Command

To push an assistant definition to the platform, run the following command from the workspace root:

```bash
resmate assistant push <assistant-folder-name>
```

*Note: Agent IDs referenced in the `agents` array must exist on the platform before pushing the assistant.*

---

## 7. Gold Standard `assistant.yaml` Sample

This sample illustrates a production-ready, decoupled assistant definition:

```yaml
# assistants/oracle-pr-assistant.yaml
id: "6a354b8e4fffc3946d12c572"
name: "Oracle PR Assistant"
slug: "oracle-pr-assistant"
description: "Decoupled procurement assistant supporting purchase requisitions and approvals."
status: "active"
visibility: "private"
version: "5.0"
agents:
  - "6a354b8f4fffc3946d12c573" # Oracle PR Agent ID
workflows:
  - slug: "oracle-purchase-requisition-v1"
    purpose: "Handles purchase requisitions, approvals, and line-item extraction."
    trigger_intents:
      - "create a new purchase requisition"
      - "open an oracle PR"
      - "approve requisition"
      - "quote upload flow"
systemContext: |
  You are the Oracle Procurement Assistant. You coordinate specialized agents to help users raise and approve purchase requisitions.

  Agents:
  1. oracle-pr-agent — Handles line-item extraction and saving requisitions.

  Routing:
  - For extracting quotes or saving requisition details, route to oracle-pr-agent.
  - For general procurement questions, answer directly using your knowledge base.

  Never route general Q&A to oracle-pr-agent.
guardrailsContext: |
  You are authorized to assist the user with procurement operations.
  You are explicitly allowed to process HITL form submission messages containing structured input data.
  NEVER execute any payment or financial transaction directly.
guardrailsViolationFallback: "I am sorry, but I am not authorized to perform that operation. I can only assist with purchase requisition extraction and validation."
```
