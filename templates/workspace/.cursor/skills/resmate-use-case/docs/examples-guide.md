# ResMate Architecture and Reference Examples Guide

The ResMate SDK ships with a collection of reference use cases under the `examples/` directory. These folders serve as a living blueprint, demonstrating distinct architectural patterns, state transitions, runtime environments, and wiring contracts.

> 🚫 **Critical Development Rule**: Never edit or develop live use cases directly under the `examples/` directory. Author live workspace artifacts inside root-level folders (`tools/`, `agents/`, `assistants/`, `hitl/`, `workflows/`). Copy design patterns from `examples/` to implement real-world solutions.

---

## 1. Reference Examples Directory Mapping

The SDK contains five standard reference examples, each tailored to demonstrate specific capabilities:

| Example Folder | Deployment Type | State Bound | Runtime / Code Style | Key Architectural Concepts Demonstrated |
| :--- | :--- | :--- | :--- | :--- |
| **`examples/minimal/`** | Standard Flow | Stateless (Ambient) | JS (V8 Isolate) | Smallest possible use case. Single agent, simple JS tool without external API dependencies or complex routing. |
| **`examples/jira/`** | Standard Flow | Ephemeral | Mixed (JS + FaaS Node) | Two-step Human-In-The-Loop (HITL) flow. Demonstrates separating low-latency UI generation (JS) from complex external API reading (FaaS). |
| **`examples/purchase-requisition/`** | Business Workflow | Persistent (Smriti State) | JS (V8 Isolate) | Split YAML multi-stage "form wizard". Demonstrates progressive state gathering, stage-level playbooks, and state patches. |
| **`examples/github-pr-create/`** | Business Workflow | Persistent (Smriti State) | JS (V8 Isolate) | Bundle JSON workflow. Features `agent_task` execution stages, automated code triggers, and recording tool output artifacts. |
| **`examples/oracle-purchase-requisition/`** | Business Workflow | Persistent (Smriti State) | JS (V8 Isolate) | Advanced "Search → Dynamic HITL Form" flow. Showcases multi-agent orchestration, dynamic tool selection, and state persistence. |

---

## 2. Detailed Reference Sample Breakdowns

### A. `examples/minimal/` (Minimal Greet)
This is the smallest possible ResMate use case, serving as the starting layout for workspace directory structures.

#### What it Demonstrates
*   **JS tool handler**: Top-level script returning a simple stringified JSON response.
*   **Single-agent configuration**: Agent with one skill and simple instructions.
*   **Single-agent assistant routing**: Direct prompt-to-agent mapping.
*   **Create-on-push ID workflow**: Demonstrating empty-to-filled local resource IDs on initial deployment.

#### File Map
| Path | Role |
| :--- | :--- |
| `tools/greet/tool.yaml` | Configuration for the greet tool. |
| `tools/greet/handler.js` | JS execution logic (returns plain greetings). |
| `agents/greet-agent.yaml` | Binds greet tool inside the `skills` array. |
| `assistants/greet-assistant.yaml` | Binds `greet-agent` inside `agents` array. |

#### Push & Deployment
To test this sample, configure your local environment and deploy from the `examples/minimal` folder:
```bash
cd examples/minimal
vgen config validate
vgen tool push greet
vgen agent push greet-agent
vgen assistant push greet-assistant
```

---

### B. `examples/jira/` (Jira Issue Reader)
A multi-tier integration showcasing human-in-the-loop task routing combined with heavy-lifting external API reading.

#### What it Demonstrates
*   **HITL Two-Step Pattern**: Separates low-latency UI card construction from slow external API requests. First, a lightweight JS tool returns an Adaptive Card. Once the user submits, a slow FaaS Node tool executes.
*   **Adaptive Card Placeholder Substitution**: Demonstrates substituting inputs (like `${issueKey}`) into an Adaptive Card JSON schema.
*   **FaaS Tool Configuration**: Resolving system environment variables, importing external packages (e.g. `axios`), and mapping `functionId` inside `tool.yaml`.

#### File Map
| Path | Role |
| :--- | :--- |
| `hitl/jira-read-issue-confirm/config.json` | Adaptive card JSON layout defining form fields and user text. |
| `hitl/jira-read-issue-confirm/meta.yaml` | Schema and title bindings for the confirmation card. |
| `tools/jira-read-issue-hitlconfig/tool.yaml` | JS HITLConfig tool schema. |
| `tools/jira-read-issue-hitlconfig/handler.js` | JS script returns the card configuration to render inside the chat. |
| `tools/jira-read-issue/tool.yaml` | FaaS tool configuration referencing Node runtime. |
| `tools/jira-read-issue/handler.js` | Complete Node.js axios code fetching Jira API details. |
| `tools/jira-read-issue/package.json` | Lists external dependencies (e.g., `"axios": "^1.6.0"`). |
| `tools/jira-read-issue/payload.json` | Sample issue keys and headers used for testing. |
| `agents/jira-agent.yaml` | Binds both the HITLConfig and FaaS tool. |
| `assistants/jira-assistants.yaml` | Binds `jira-agent` and specifies global context. |

#### Push & Deployment
Deploy the resources sequentially in chronological dependency order:
```bash
cd examples/jira

export VGEN_HITL_DIR=./hitl
export VGEN_TOOLS_DIR=./tools
export VGEN_AGENTS_DIR=./agents
export VGEN_ASSISTANTS_DIR=./assistants

vgen hitl push jira-read-issue-confirm
vgen tool push jira-read-issue-hitlconfig
vgen tool push jira-read-issue
vgen agent push jira-agent
vgen assistant push jira-assistants
```

#### Local Testing
Run a simulated local test of your slow FaaS API tool using payload records:
```bash
vgen tool test jira-read-issue
```

---

### C. `examples/purchase-requisition/` (Split YAML Workflow Form Wizard)
An advanced, multi-stage business workflow demonstrating persistent, stage-by-stage state progression.

#### What it Demonstrates
*   **Split YAML Workflow Layout**: Splitting workflow schemas, stage flows, playbooks, and meta details into separate files.
*   **Form Wizard Pipeline**: Sequential validation progression through distinct stages: `collect_supplier` $\rightarrow$ `collect_lines` $\rightarrow$ `review_pr` $\rightarrow$ `submit_oracle`.
*   **State-Constrained Save Tools**: Save tools returning a `workflowPatch` object rather than raw content. The patch directly mutates Smriti's persistent ledger.
*   **Orchestration Contract Binding**: Assistant YAML binding directly to `workflow_definition_slug`.

#### File Map
| Path | Role |
| :--- | :--- |
| `workflows/purchase-requisition/meta.yaml` | Unique slug, name, and initial entry versioning. |
| `workflows/purchase-requisition/schema.yaml` | Defines core types (vendor data, lines) to validate state against. |
| `workflows/purchase-requisition/flow.yaml` | Controls stage-to-stage transitions (`collect`, `review`, etc.). |
| `workflows/purchase-requisition/playbooks.yaml` | Defines stage-level instructions, constraints, and reply policies. |
| `hitl/purchase-req-vendor-form/` | Adaptive card collecting Vendor ID details. |
| `hitl/purchase-req-lines-form/` | Adaptive card collecting line-item arrays. |
| `hitl/purchase-req-review/` | Confirmation card showing summary of PR data prior to submit. |
| `tools/purchase-req-vendor-hitlconfig/` | JS tool generating the Vendor selection card configuration. |
| `tools/purchase-req-vendor-save/` | JS tool returning a `workflowPatch` committing Vendor input to the state. |
| `tools/purchase-req-lines-hitlconfig/` | JS tool generating the Line Items collection card configuration. |
| `tools/purchase-req-lines-save/` | JS tool returning a `workflowPatch` committing Line Item input to the state. |
| `tools/purchase-req-review-hitlconfig/` | JS tool generating the Review summary card configuration. |
| `tools/purchase-req-review-confirm/` | JS tool returning a confirmation `workflowPatch` committing PR approval. |
| `agents/purchase-req-agent.yaml` | Stage-aware agent utilizing all 6 tools as skills. |
| `assistants/purchase-req-assistant.yaml` | Orchestrates the session and binds `purchase-requisition-v1`. |

#### Push & Deployment
To push the split workflow and its associated resources, execute:
```bash
cd examples/purchase-requisition

export VGEN_HITL_DIR=./hitl
export VGEN_WORKFLOWS_DIR=./workflows
export VGEN_TOOLS_DIR=./tools
export VGEN_AGENTS_DIR=./agents
export VGEN_ASSISTANTS_DIR=./assistants

vgen hitl push purchase-req-vendor-form
vgen hitl push purchase-req-lines-form
vgen hitl push purchase-req-review
vgen workflow push purchase-requisition
vgen tool push purchase-req-vendor-hitlconfig
vgen tool push purchase-req-vendor-save
vgen tool push purchase-req-lines-hitlconfig
vgen tool push purchase-req-lines-save
vgen tool push purchase-req-review-hitlconfig
vgen tool push purchase-req-review-confirm
vgen agent push purchase-req-agent
vgen assistant push purchase-req-assistant
```

#### Smoke Testing
To smoke-test the live workflow multi-turn progression:
```bash
CHAT_SMOKE_SCENARIO=workflow-purchase-requisition ./scripts/chat-smoke-test.sh <chat_id> <session_id>
```

---

### D. `examples/github-pr-create/` (Bundle JSON Workflow)
A business workflow that showcases executing automated programmatic tasks rather than manual card inputs.

#### What it Demonstrates
*   **Bundle JSON Workflow Format**: Storing entire workflow definitions inside a single JSON schema.
*   **`agent_task` Stages**: Stages where an agent executes tools autonomously to gather inputs (e.g., checking git branches) rather than waiting for a user card submit.
*   **Recording Output Artifacts**: Tools return structured payloads that compile to persistent `workflowPatch.artifacts` (such as recording a created PR URL) within Smriti.

---

### E. `examples/oracle-purchase-requisition/` (Search & Dynamic HITL)
An advanced orchestration scenario featuring multiple collaborative agents and dynamic tool selection.

#### What it Demonstrates
*   **Search-to-HITL Pattern**: The agent first queries a database (using a search tool). If multiple results are found, it dynamically launches an Adaptive Card to let the user select the target record.
*   **Multi-Agent Orchestration**: Demonstrates separating tasks between multiple highly-specialized agents (e.g., a routing assistant delegating tasks to search agents or data entry agents).

---

## 3. Deep Dive: Architectural Design Choices

When designing a new ResMate use case, developers must navigate three key architectural forks. The examples illustrate these decisions.

```text
                                  Architectural Choices
                                            |
         +----------------------------------+----------------------------------+
         |                                  |                                  |
         v                                  v                                  v
  Execution Model                     State Bound                       Runtime Layer
  - Single-Agent                      - Standard Flow                   - JS (V8 Isolate)
  - Multi-Agent                       - Business Workflow               - FaaS (Node/Lambda)
```

### A. Execution Model: Single-Agent vs. Multi-Agent Orchestration

Choosing the right execution boundary prevents LLM context-switching bloat and keeps execution plans highly targeted.

#### Single-Agent Pattern (e.g., `examples/minimal/`, `examples/purchase-requisition/`)
*   **When to Use**: The task has a single operational domain and a straightforward execution sequence.
*   **Benefits**: Zero routing overhead, minimal token consumption, and clear system instructions.
*   **Design Pattern**:
    *   The assistant binds to exactly one agent inside `assistant.yaml`.
    *   The agent directly binds to all required workspace tools under its `skills` array.

#### Multi-Agent Pattern (e.g., `examples/oracle-purchase-requisition/`)
*   **When to Use**: The use-case crosses distinct operational boundaries (e.g., one task extracts text from documents, another performs deep database record lookup, and a third pushes to external ERP gateways).
*   **Benefits**: Isolation of concerns, specialized tool access, and tighter security scopes.
*   **Design Pattern**:
    *   The assistant acts as a router, binding to multiple agents under the `agents` array.
    *   The planner dynamically delegates execution to the optimal agent based on active stage demands and user requests.

---

### B. State Bound: Standard Flow vs. Business Workflow

This choice dictates how the system maintains context and tracks execution history.

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

#### Standard Flow (e.g., `examples/minimal/`, `examples/jira/`)
*   **Characteristics**: Stateless or ephemeral session tracking. The system does not maintain an immutable stage pipeline.
*   **Use Cases**: Conversational Q&A, rapid utility tasks, simple stateless lookups.
*   **How it Works**: Relies purely on the assistant's `systemContext` and the underlying LLM's conversational window. Once the session is closed, the state is cleared.

#### Business Workflow (e.g., `examples/purchase-requisition/`, `examples/oracle-purchase-requisition/`)
*   **Characteristics**: Structured stage-by-stage progression backed by Smriti's database. State transitions are controlled by defined conditions, and each step is recorded in an immutable ledger.
*   **Use Cases**: Purchase approvals, compliance form-filling, complex multi-step data extractions.
*   **How it Works**:
    *   Utilizes a dedicated `workflows/` folder containing `flow.yaml`, `schema.yaml`, and `playbooks.yaml`.
    *   State is modified using specialized "Save Tools" which return a `workflowPatch` object rather than raw data. This patch is applied directly to the central Smriti state machine:
        ```json
        // Output from a Save Tool
        {
          "workflowPatch": {
            "inputs": {
              "supplierId": "SUPP-9921",
              "totalAmount": 15000
            }
          }
        }
        ```

---

### C. Runtime Layer: JS (V8 Isolate) vs. FaaS (Node/Lambda)

Specifying the correct execution environment is critical for managing latency, external dependencies, and security boundaries.

#### JS (V8 Isolate) Runtime
*   **Execution Budget**: Exceptionally fast (<500ms execution timeout limit).
*   **Capabilities**: Self-contained, lightweight script evaluation. Access to external npm packages is restricted to ensure high-velocity cold starts.
*   **Smriti Integration**: Can utilize local platform polyfills (`getSecret`, `queryRecords`) to perform secure actions.
*   **Typical Usage**: Local validation, HITL form payload assembly, dynamic routing logic, and data normalization.

#### FaaS (Node/Lambda) Runtime
*   **Execution Budget**: Extended runtime allocations (up to 30s timeout).
*   **Capabilities**: Full multi-file Node.js support. Developers can define a local `package.json` file and install any required npm dependencies (e.g., `axios`, `aws-sdk`, `pg`).
*   **Smriti Integration**: Supports long-lived network queries and direct secure integrations.
*   **Typical Usage**: Deep external API lookups, document parsing, database migrations, and complex heavy-lifting operations.

---

## 4. How to Copy Patterns for New Use Cases

When initializing a new use case, follow this checklist to adapt patterns from the reference directory:

1.  **Bootstrap the Workspace**: Run `vgen init --name my-use-case` to scaffold the correct directory tree.
2.  **Analyze the Requirements**: Identify your targets:
    *   If you need a simple lookup, copy the structure of `examples/minimal/`.
    *   If you need multi-turn HITL forms, copy the code files from `examples/jira/`.
    *   If you are building a structured business process, copy the split YAML structures from `examples/purchase-requisition/`.
3.  **Draft the Schemas**: Write your local YAMLs (`tool.yaml`, `agent.yaml`, `assistant.yaml`) and implement your handlers under `tools/<name>/handler.js`.
4.  **Validate Locally**: Execute `vgen validate` and run smoke tests with `vgen tool test <name>` to ensure all local schemas and Javascript handlers parse correctly before deploying.
