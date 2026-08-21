# Agent Specification & Schema Reference

This document defines the complete schema and instruction design contracts for ResMate Agents (`agent.yaml`). Agents are specialized entities executed by the **agent planner** ReAct loop to perform domain-specific tasks using bound tools.

---

## 1. `agent.yaml` Schema Reference

Every ResMate agent is defined in a single YAML file named `agents/<agent-slug>.yaml` at the workspace root.

### Complete Field Reference

| Field | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `id` | `string` | After first push | Stable MongoDB identifier for the agent. Leave empty or omit during initial creation; the platform populates this on the first `vgen agent push`. |
| `name` | `string` | Yes | Display name of the agent. Used by the orchestrator v4 enum to identify and route to the agent. |
| `slug` | `string` | Yes | Stable, unique slug identifier. Used in composite `tool_id` generation at runtime. |
| `description` | `string` | Yes | Short description of the agent's domain and responsibilities, exposed to the assistant planner. |
| `systemInstructions` | `array` | Yes | **Array of strings** defining the agent's role, step-by-step tool execution order, and safety constraints. (Often referred to as agent instructions). |
| `skills` | `array` | Yes | List of **Tool Mongo IDs** (from `tools/*/tool.yaml`) that this agent is authorized to call. |
| `is_public` | `boolean` | Recommended | Whether the agent is publicly accessible. Typically `false` for proprietary enterprise agents. |
| `roles` | `array` | No | Role identifiers required to access or execute this agent. |
| `admins` | `array` | No | List of admin user identifiers. |
| `version` | `string` | Recommended | Version identifier (e.g., `'1.0'`). |
| `createdBy` | `string` | No | Author identifier or email. |
| `managedBy` | `array` | No | List of managing entities or teams. |
| `guardrailsContext` | `string` | No | Optional instructions limiting agent behavior. Usually left null as guardrails reside primarily on the assistant. |
| `guardrailsViolationFallback` | `string` | No | Fallback message if guardrails are violated. |
| `icon` | `string` | No | Optional URL or identifier for the agent's UI icon. |

---

## 2. Tool Binding Syntax (`skills`)

The `skills` array binds tools to an agent. This boundary enforces authorization; an agent cannot execute a tool unless its stable MongoDB ID is registered in the agent's `skills` list.

### 2.1 Binding Syntax Rules
- **Use Mongo IDs, Not Folder Names**: Each entry in the `skills` array **must** be the literal `id` field from a pushed `tool.yaml`. Do NOT use the tool's folder name or display name.
- **Dependency Order**: You must push tools first to obtain their stable IDs before referencing them in the agent's `skills` array and pushing the agent.
- **Cross-Check Before Push**: Every ID in the `skills` array must exist in `tools/*/tool.yaml` on the platform.

### 2.2 Example Tool Binding
```yaml
# agents/oracle-pr-agent.yaml
id: "6a354b8f4fffc3946d12c573"
name: "Oracle PR Agent"
slug: "oracle-pr-agent"
description: "Specialized agent for extracting and saving Oracle Purchase Requisitions."
skills:
  - "6a354b844fffc3946d12c56c" # Tool ID for oracle-pr-quote-extract-save
  - "6a354b754fffc3946d12c562" # Tool ID for oracle-pr-header-save
```

### 2.3 Runtime Composite `tool_id`
At runtime, the platform generates a composite `tool_id` combining the agent's slug and the tool's name (e.g., `oracle-pr-agent__OraclePRHeaderSave`). This ensures that tool execution is namespace-isolated per agent.

---

## 3. System Instructions Design (`systemInstructions`)

The `systemInstructions` field is the core of an agent's behavior. Because agents are executed by a **ReAct (Reasoning and Acting) planner loop**, instructions must be precise, step-oriented, and highly structured to prevent prompt dilution and conversational drift.

### 3.1 Formatting as a YAML Array of Strings
To ensure clean parsing, token efficiency, and to avoid multi-line scalar formatting issues, `systemInstructions` **must be written as a YAML array of strings**:

```yaml
# ❌ BAD — Multiline scalar string (prone to parsing issues and prompt dilution)
systemInstructions: |
  You are a Jira Agent. First, you should read the issue.
  Then, you should ask the user for confirmation.
  Finally, write the comment.

# ✅ GOOD — Structured array of strings (clean, step-oriented, token-efficient)
systemInstructions:
  - "Role: You are the specialized jira-agent executing Jira operations."
  - "Read — Step 1: Call 'Jira Read Issue HITLConfig' with optional issueKey pre-fill."
  - "Read — Step 2: After the user submits the HITL form, call 'jira-read-issue' with the submitted issueKey."
  - "Never call write tools before their corresponding HITL form has been submitted."
```

### 3.2 Best Practices for Step-Oriented Instructions
To maximize planning accuracy and avoid conversational hallucination, design your instructions around the following principles:

1. **Explicit Intent Mapping**: Define clear, numbered steps for each primary developer/user intent (e.g., Search, Read, Create).
2. **HITL-First Sequencing**: If a tool is gated by a Human-In-The-Loop (HITL) form, explicitly instruct the planner to call the `*HITLConfig` tool first, wait for the form submission, and only then call the execution tool.
3. **Reference Tools by Display Name**: The agent planner sees the active tool catalog. In your instructions, reference tools by their display **`name`** (e.g., `Jira Read Issue`) rather than their folder names or MongoDB IDs.
4. **Enforce Never-Rules**: State absolute constraints clearly (e.g., *"NEVER execute vendor-lookup-tool unless a numeric vendorId is supplied"*).
5. **Keep It Token-Efficient**: Avoid long, flowery identity prose or redundant system descriptions. Redundant identity prose belongs on the assistant level, not the agent level.

---

## 4. Agent vs. Assistant Division of Labor

A common anti-pattern is duplicating the assistant's routing table inside the agent's instructions, or vice versa. Maintain a strict separation of concerns:

| Dimension | Agent `systemInstructions` | Assistant `systemContext` |
| :--- | :--- | :--- |
| **Scope** | Domain-specific execution (e.g., Jira operations only). | Global coordination and multi-agent routing. |
| **Focus** | Step-by-step tool order and HITL-to-action sequences. | Mapping user intents to the correct specialized agent. |
| **Awareness** | Knows about its bound tools and their parameters. | Knows about all available agents and workflows. |
| **Gates** | Enforces parameter validation and tool constraints. | Enforces cross-agent prerequisites and global gates. |

---

## 5. CLI Push Command

To push an agent definition to the platform, run the following command from the workspace root:

```bash
vgen agent push <agent-folder-name>
```

*Note: Tool IDs referenced in the `skills` array must exist on the platform before pushing the agent.*

---

## 6. Gold Standard `agent.yaml` Sample

This sample illustrates a production-ready, highly optimized agent definition:

```yaml
# agents/oracle-pr-agent.yaml
id: "6a354b8f4fffc3946d12c573"
name: "Oracle PR Agent"
slug: "oracle-pr-agent"
description: "Specialized procurement agent for extracting line items and saving Oracle Purchase Requisitions."
is_public: false
version: "1.0"
skills:
  - "6a354b844fffc3946d12c56c" # oracle-pr-quote-extract-save
  - "6a354b754fffc3946d12c562" # oracle-pr-header-save
systemInstructions:
  - "Role: You are the Oracle PR Agent, a specialized procurement assistant."
  - "Extraction Flow — Step 1: When a user uploads a quote, immediately invoke the 'oracle-pr-quote-extract-save' tool to extract line items."
  - "Extraction Flow — Step 2: Present the extracted line items to the user and ask for confirmation."
  - "Save Flow — Step 1: Once confirmed, invoke 'oracle-pr-header-save' to write the requisition header and items to the system."
  - "Constraint: NEVER call 'oracle-pr-header-save' until the user has explicitly confirmed the extracted line items."
  - "Constraint: If the quote extraction fails, ask the user to manually input the missing fields instead of retrying the tool."
```
