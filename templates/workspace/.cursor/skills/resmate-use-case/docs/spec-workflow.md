# Workflow Specification: flow.yaml, schema.yaml, and playbooks.yaml

In the ResMate platform, a **Business Workflow** is a state-machine-driven execution flow defined by developers and managed by the Smriti state engine. Workflows enforce structured stages, capture audit trails, apply stage-specific playbooks, and coordinate human-in-the-loop (HITL) interactions.

This document provides a comprehensive, high-density reference for authoring ResMate workflows, covering the split and bundled folder layouts, detailed YAML/JSON schemas, stage transitions, stage-level playbooks (`playbooks.yaml`), and state save tool contracts.

---

## 1. Directory Layout & Formats

ResMate supports three authoring formats. All of them compile to the same internal canonical model at push time.

### 1.1 Supported Formats

| Format | Layout | Best For |
| :--- | :--- | :--- |
| **Split YAML (Default)** | `workflows/<name>/meta.yaml` + `schema.yaml` + `flow.yaml` + optional `gates.yaml` + `playbooks.yaml` | Workspace authors; highly readable; clean, Git diff-friendly pull requests. |
| **Bundle YAML** | `workflows/<name>/workflow.yaml` — single file with `meta`, `schema`, `flow`, `gates`, and `playbooks` sections. | Small workflows; rapid prototyping; easy copy-pasting of single-file recipes. |
| **Bundle JSON** | `workflows/<name>/workflow.json` — identical structure to bundle YAML but in JSON format. | Programmatic generation; CI/CD automation pipelines; API integration imports. |

### 1.2 Split YAML Folder Layout (Recommended)

When developing a workflow under the `workflows/` directory of a ResMate workspace, the default split folder layout separates metadata, data schema, transition logic, and stage playbooks:

```text
workflows/purchase-requisition/
├── meta.yaml         # Workflow identity, slug, and versioning
├── schema.yaml       # Data fields, bags, and validation rules
├── flow.yaml         # Stage definitions, kinds, and transitions
├── gates.yaml        # optional — reusable named predicates (see §2.4.5)
└── playbooks.yaml    # Stage-specific briefs, constraints, and hooks
```

### 1.3 Bundle YAML Example

For single-file representation, developers can use `workflow.yaml`:

```yaml
meta:
  name: "GitHub PR Create"
  slug: "github-pr-create-v1"
  workflowType: "github_pr_create"
  version: 1
  description: "Collect intent, generate branch, and open pull request."

schema:
  fields:
    - key: "repoName"
      label: "Repository Name"
      type: "string"
      bag: "inputs"
      required: true
      requiredFromStage: "collect_intent"
    - key: "prUrl"
      label: "Pull Request URL"
      type: "string"
      bag: "artifacts"
      requiredFromStage: "generate_and_open_pr"

flow:
  initialStage: "collect_intent"
  stages:
    - id: "collect_intent"
      label: "Collect Intent"
      kind: "collect"
      hitlSlug: "github-pr-intent-form"
      agentSlug: "github-pr-agent"
      doneWhen:
        - "repoName"
      next: "generate_and_open_pr"
    - id: "generate_and_open_pr"
      label: "Generate and Open PR"
      kind: "agent_task"
      agentSlug: "github-pr-agent"
      expectedOutcome: "PR opened with valid URL saved in workflow artifacts."
      doneWhen: "validator_pass"
      next: "submitted"
    - id: "submitted"
      label: "Submitted"
      kind: "terminal"
      terminal: true
      doneWhen: "terminal"

playbooks:
  collect_intent:
    replyPolicy:
      tone: "helpful and supportive"
      wordLimit: 100
      instructions:
        - "Ask the user politely for the repository name and branch details if missing."
    constraints:
      - "Never call github-pr-save unless a valid repoName is supplied."
  generate_and_open_pr:
    agentBrief: "Execute the git branch and pull request creation tool. Ensure the resulting PR URL is saved to the prUrl artifact."
```

### 1.4 Layout Detection Rules

The ResMate loader checks folders using the following precedence rules:
1. If `workflow.yaml` or `workflow.json` exists under `workflows/<name>/`, parse as a bundle.
2. Otherwise, if both `meta.yaml` and `flow.yaml` exist, parse as Split YAML and merge `meta.yaml`, `schema.yaml`, `flow.yaml`, `gates.yaml` (optional), and `playbooks.yaml` (optional).
3. If neither condition is met, fail with a validation error.

---

## 2. Component Specifications & Schemas

### 2.1 `meta.yaml`

The `meta.yaml` file defines the identity, stable slug, type classification, and version of the workflow.

#### Schema Fields

| Field | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `id` | `string` | No | MongoDB Object ID. Omitted during local development; assigned by Smriti on push. |
| `name` | `string` | Yes | Human-readable name of the workflow displayed in the ResMate UI. |
| `slug` | `string` | Yes | Stable lookup key used by assistants to bind this workflow. Must be unique. |
| `workflowType` | `string` | Yes | Stable type key for categorizing and filtering workflow instances. Pattern: `^[a-z][a-z0-9_]*$`. |
| `version` | `integer` | Yes | Incremental version integer. Smriti enforces append-only versioning on push. |
| `description` | `string` | No | Detailed description of the workflow's purpose and scope. |

#### Example `meta.yaml`

```yaml
id: ""
name: "Oracle Purchase Requisition"
slug: "oracle-purchase-requisition-v1"
workflowType: "purchase_requisition"
version: 1
description: "Multi-step PR wizard with approval gate for amounts over 10k."
```

---

### 2.2 `schema.yaml`

The `schema.yaml` file defines the data schema of the workflow instance. It specifies every field that can be collected, processed, or generated, organizing them into logical storage "bags" and establishing validation rules.

#### Schema Fields

| Field | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `fields` | `array` | Yes | List of field definitions. Each item must conform to the field schema below. |

#### Field Definition Schema (`$defs/field`)

| Property | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `key` | `string` | Yes | The stable identifier for the field. Used as the key in state patches and lookups. |
| `label` | `string` | Yes | Human-readable label displayed in HITL forms and UI summaries. |
| `type` | `string` | Yes | Data type of the field. Allowed: `string`, `number`, `boolean`, `array`, `object`. |
| `bag` | `string` | Yes | Storage bucket for the field. Allowed:<br>- `inputs`: Collected from human users or external upstream systems.<br>- `artifacts`: Generated by AI agents or downstream automated tools (e.g., object-store refs, URLs). |
| `required` | `boolean` | No | If `true`, the field must be populated before the entire workflow can reach a terminal state. |
| `requiredFromStage` | `string` | No | Stage ID that **owns** this field requirement. Enforced only when that stage is on the instance **active path** (historical visits plus forward projection from the current stage). Stages bypassed by conditional branching never enter the active path, so their `requiredFromStage` fields are not required on those paths. Prefer `requiredFromStage` over blanket `required: true` for fields on skippable stages. Do not set both on the same field — if both are present, `required: true` wins and `requiredFromStage` is ignored. |
| `hitlFieldId` | `string` | No | Maps this field directly to a field ID in a HITL form configuration. |
| `phi` | `boolean` | No | If `true`, marks the field as Protected Health Information (PHI) to trigger platform-level encryption. |
| `redactInPrompt` | `boolean` | No | If `true`, redacts this field's value in LLM prompt contexts to protect sensitive data. |
| `validation` | `object` | No | Custom validation parameters (e.g., regex, min/max values) checked by the state engine. |

#### Example `schema.yaml`

```yaml
fields:
  - key: vendorId
    label: Vendor ID
    type: string
    bag: inputs
    required: true
    requiredFromStage: collect_vendor
    hitlFieldId: vendorId

  - key: lineItems
    label: Line Items
    type: array
    bag: inputs
    required: true
    requiredFromStage: collect_line_items

  - key: totalAmount
    label: Total Amount
    type: number
    bag: inputs
    required: true
    requiredFromStage: collect_line_items

  - key: prUrl
    label: Purchase Requisition URL
    type: string
    bag: artifacts
    requiredFromStage: generate_and_open_pr
    redactInPrompt: false

  - key: reviewConfirmed
    label: Review Confirmed
    type: boolean
    bag: inputs
    required: true
    requiredFromStage: review_summary
    hitlFieldId: reviewConfirmed
```

---

### 2.3 `flow.yaml`

The `flow.yaml` file defines the state machine of the workflow. It declares the initial starting stage and maps out the sequence of execution stages, including transition boundaries, preferred agents, and expected outcomes.

#### Schema Fields

| Field | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `initialStage` | `string` | Yes | The ID of the stage where the workflow begins execution. |
| `stages` | `array` | Yes | List of stage definitions. Each item must conform to the stage schema below. |

#### Stage Definition Schema (`$defs/stage`)

| Property | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `id` | `string` | Yes | Unique identifier for the stage. Keyed by playbooks and transition rules. |
| `label` | `string` | Yes | Human-readable label representing the stage in the UI progress bar. |
| `kind` | `string` | Yes | The execution nature of the stage. Allowed: `collect`, `agent_task`, `review`, `terminal`. |
| `doneWhen` | `array` \| `string` | Yes | The condition that must be met to complete the stage. Can be:<br>- An `array` of field keys (e.g., `[vendorId, totalAmount]`) that must be non-null.<br>- The string `"validator_pass"`, requiring an explicit supervisor/validator assertion.<br>- The string `"terminal"`, used only for terminal stages. |
| `hitlSlug` | `string` | No | The slug of the HITL record associated with this stage (required for `collect` and `review` stages). |
| `agentSlug` | `string` | No | The slug of the preferred AI agent assigned to execute this stage. |
| `expectedOutcome` | `string` | No | High-level description of the task's target outcome. (Required for `agent_task` stages to guide the Agent Planner). |
| `next` | `string` | No | Legacy sequential fallback target. Used when no `transitions` rule matches, or as the default branch when `transitions` is omitted. |
| `back_to` | `string` | No | **Legacy metadata hint only** — not wired into auto-advance routing. For rework loops, use `transitions[].target` pointing at an earlier stage id plus `reset` (see §2.4.3). Do not use `back_to` in new authoring. |
| `transitions` | `array` | No | Ordered list of conditional transition rules. Evaluated after `doneWhen` is satisfied and **before** the legacy `next` fallback. See §2.4. |
| `terminal` | `boolean` | No | If `true`, marks this stage as an end-state for the workflow. |

#### Example `flow.yaml`

```yaml
initialStage: collect_vendor
stages:
  - id: collect_vendor
    label: "Collect Vendor"
    kind: collect
    hitlSlug: "oracle-pr-vendor-form"
    agentSlug: "oracle-pr-agent"
    doneWhen:
      - vendorId
    next: collect_line_items

  - id: collect_line_items
    label: "Collect Line Items"
    kind: collect
    hitlSlug: "oracle-pr-lines-form"
    agentSlug: "oracle-pr-agent"
    doneWhen:
      - lineItems
      - totalAmount
    next: review_summary

  - id: review_summary
    label: "Review Summary"
    kind: review
    hitlSlug: "oracle-pr-review-form"
    agentSlug: "oracle-pr-agent"
    doneWhen:
      - reviewConfirmed
    next: generate_and_open_pr
    back_to: collect_line_items

  - id: generate_and_open_pr
    label: "Generate and Open PR"
    kind: agent_task
    agentSlug: "oracle-pr-agent"
    expectedOutcome: "Purchase requisition successfully created in Oracle and the PR URL is saved to artifacts."
    doneWhen: "validator_pass"
    next: completed

  - id: completed
    label: "Completed"
    kind: terminal
    terminal: true
    doneWhen: "terminal"
```

---

### 2.4 Conditional Transitions

By default, a stage advances to the stage named in `next` once its `doneWhen` condition is satisfied. For branching workflows — fast-track paths, approval/rejection forks, or amount-based routing — add a `transitions` array to the stage.

Each transition rule is evaluated **in array order**. The **first** rule whose `when` condition evaluates to `true` determines the target stage. If no rule matches, the engine falls back to the legacy `next` field. If neither a matching transition nor `next` exists, auto-advance halts (see **Dead-end pitfalls** below).

#### Transition Rule Schema (`$defs/transition`)

| Property | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `target` | `string` | Yes | Stage ID to transition to when `when` evaluates to `true`. Must reference an existing stage. |
| `when` | `object` | Yes | A `Condition` tree (see below). |
| `reset` | `array` | No | Field keys to remove from instance state when this transition is taken. Applies to forward and backward transitions. See §2.4.3. |

#### Condition Primitives (`$defs/condition`)

Conditions are JSON-safe predicate trees evaluated against the live workflow instance (`inputs` and `artifacts` bags).

| Variant | YAML shape | Evaluates to `true` when… |
| :--- | :--- | :--- |
| **`present`** | `present: "<path>"` | The field at `path` exists and is non-null/non-empty. |
| **`absent`** | `absent: "<path>"` | The field is missing, null, or empty. |
| **`eq`** | `eq: { field: "<path>", value: <json> }` | The field value exactly equals `value` (JSON equality). |
| **`all`** | `all: [ <conditions> ]` | Every sub-condition is `true`. An empty list (`all: []`) is vacuously `true` — use as an explicit exhaustive fallback branch. |
| **`any`** | `any: [ <conditions> ]` | At least one sub-condition is `true`. |
| **`not`** | `not: <condition>` | The sub-condition is `false`. |
| **`gate`** | `gate: "<gateId>"` | Resolves and evaluates the named predicate defined in `gates.yaml`. See §2.4.5 for schema, composition, and validation. |

**Field paths:** Use explicit namespaces (`inputs.vendorId`, `artifacts.prUrl`) or bare keys (`vendorId`) — bare keys search `inputs` first, then `artifacts`.

#### Evaluation Order

When a stage's `doneWhen` is satisfied, the engine resolves the next stage in this order:

1. **Conditional transitions** — iterate `transitions` in definition order; take the first rule where `when` is `true`.
2. **Legacy `next`** — if no transition matched, use the stage's `next` field.
3. **Halt** — if neither applies, log `no_matching_transition` and keep the instance on the current stage.

Overlapping conditions are deterministic: put the most specific rule first; broader catch-all rules (or `next`) belong last.

#### YAML Examples

**Fast-track vs standard path** (`eq` + `present` + `next` fallback):

```yaml
- id: collect_vendor
  label: "Collect Vendor"
  kind: collect
  hitlSlug: "oracle-pr-vendor-form"
  doneWhen: [vendorId]
  transitions:
    - target: review_summary
      when:
        eq:
          field: inputs.vendorId
          value: "V-PRE_APPROVED"
    - target: collect_line_items
      when:
        present: inputs.vendorId
  next: collect_line_items
```

**Review approve / reject fork with rework reset** (`eq` branches + back-edge field clearing):

```yaml
- id: review_summary
  label: "Review Summary"
  kind: review
  hitlSlug: "oracle-pr-review-form"
  doneWhen: [reviewConfirmed]
  transitions:
    - target: generate_and_open_pr
      when:
        eq:
          field: inputs.reviewConfirmed
          value: true
    - target: collect_line_items
      when:
        eq:
          field: inputs.reviewConfirmed
          value: false
      reset:
        - reviewConfirmed
        - lineItems
```

The reject branch is a **back-edge** (returns to an earlier stage). Without `reset`, `reviewConfirmed` and `lineItems` would still be populated from the previous pass, causing `collect_line_items` and `review_summary` to immediately re-satisfy `doneWhen` and bounce forward. See §2.4.3 and ADR 015.

**Exhaustive fallback without `next`** (vacuous `all: []` as catch-all):

```yaml
- id: route_by_amount
  label: "Route by Amount"
  kind: collect
  doneWhen: [totalAmount]
  transitions:
    - target: manager_review
      when:
        eq:
          field: inputs.totalAmount
          value: 10000
    - target: standard_submit
      when:
        all: []
```

#### Dead-End Pitfalls

A **dead end** occurs when a non-terminal stage has `transitions` defined but, at runtime, no rule matches and there is no `next` fallback.

| Pitfall | Symptom | Fix |
| :--- | :--- | :--- |
| Missing fallback | `resmate workflow validate` rejects with `WORKFLOW_SEMANTIC_INVALID`; runtime logs `no_matching_transition` | Add a legacy `next` field **or** a final transition with `when: { all: [] }` |
| Dangling target | Validation error: transition target stage does not exist | Ensure every `target` references a stage `id` in the same workflow |
| Over-specific rules | Instance stuck on stage despite "complete" inputs | Reorder rules (specific first) or add a catch-all branch |
| Stale state on back-edges | Stage immediately bounces forward after rework | Use `reset` on back-edge transitions to clear gating fields (see §2.4.3) |

**Static validation:** `resmate workflow validate` checks that any non-terminal stage with `transitions` has an exhaustive fallback — either a legacy `next` field or a transition whose `when` is trivially always-true (`all: []`).

**Runtime behavior (`no_matching_transition`):** If validation was bypassed or instance state diverges from what authors expected, the engine **does not** throw. It halts auto-advance, writes a `no_matching_transition` audit entry, and leaves the instance on the current stage so the user or agent can correct inputs and retry.

#### `next` Fallback

The legacy `next` field remains fully supported. Linear workflows need no `transitions` at all. When both are present, `transitions` are evaluated first; `next` is the deterministic default when no conditional rule matches. This preserves backward compatibility with v1 linear definitions.

#### 2.4.3 `reset` — Clearing Fields on a Transition

Any transition rule — forward **or** backward — may declare a `reset` list of field keys. When that transition is taken, the runtime removes each listed key from its owning bag (`inputs` or `artifacts`) before entering the target stage. Reset uses key removal (not writing `null`), so `field_is_present` correctly treats cleared fields as absent afterward — including for required-array checks.

**Mechanism**

1. A stage satisfies `doneWhen`.
2. The engine evaluates `transitions` in order and selects the first matching rule.
3. **`execute_transition_resets`** runs for the matched rule's `reset` list — each key is removed from the bag declared in `schema.yaml`.
4. The instance advances to `target`.

Reset is **not direction-aware**: authors declare which fields must be cleared; the runtime does not infer whether a transition is forward or backward. This keeps rework loops fully declarative (ADR 015 §1).

**Rework-loop safety**

Without `reset`, a back-edge transition (e.g. `review_summary → collect_line_items` on rejection) leaves gating fields populated from the previous pass. The revisited stage's `doneWhen` is already satisfied, so the engine immediately auto-advances forward again — an infinite **bounce**. Clearing fields that gate `doneWhen` on the target stage (and any intermediate stages skipped on re-entry) forces the user to re-supply corrected data.

At runtime, a separate safety net prevents infinite loops inside a single auto-advance call: `visited_stages` cycle detection halts with a `stage_advance_halted_cycle` audit entry, and a hop bound (`MAX_AUTO_ADVANCE_HOPS = 10`) halts with `stage_advance_halted_max_hops`. These are runtime guards — authors should still declare `reset` on rework transitions (see §9 checklist).

**Validation**

Every key in a `reset` list must exist in `schema.fields`. `resmate workflow validate` rejects unknown keys with `WORKFLOW_SEMANTIC_INVALID` and a path like:

```text
flow.stages[review_summary].transitions[1].reset[0]
```

**`back_to` vs transition back-edges**

| Mechanism | Role | Use in new authoring? |
| :--- | :--- | :--- |
| **`back_to`** (stage field) | Legacy UI/metadata hint only — **not wired** into auto-advance routing (ADR 015 §3) | **No** — do not rely on it for rework |
| **`transitions[].target`** pointing at an earlier stage id | Actual rework routing — fully supported since Phase 1's conditional transition engine | **Yes** — pair with `reset` on reject/rework branches |

Actual backward routing goes through ordinary `transitions` entries whose `target` is an earlier stage id in `flow.yaml` declaration order. The legacy `back_to` field may still appear in older definitions or UI hints but does not drive the state machine.

#### 2.4.5 `gates.yaml` — Reusable Named Predicates

Once the same `Condition` is referenced from more than one place — multiple `transitions[].when` clauses on different stages, or nested inside another gate's `when` tree — extract it into **`gates.yaml`**, an optional sibling split file next to `meta.yaml`, `schema.yaml`, and `flow.yaml`. In bundle YAML/JSON, the equivalent is a top-level `gates:` array.

##### Gate Definition Schema (`WorkflowGateDef`)

| Property | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `id` | `string` | Yes | Stable identifier referenced via `{ gate: "<id>" }` in transition or gate `when` trees. Must be unique within the workflow. |
| `name` | `string` | Yes | Human-readable label for docs, graph visualization, and planner prompts. |
| `when` | `object` | Yes | A `Condition` tree (same seven primitives as §2.4). May nest `{ gate: "<otherId>" }` to compose predicates. |

##### When to Extract vs Inline

| Situation | Authoring choice |
| :--- | :--- |
| Predicate used once on a single transition | Inline in `transitions[].when` — no `gates.yaml` needed |
| Same predicate on multiple stages or transitions | Extract to `gates.yaml`; reference with `{ gate: "<id>" }` |
| Complex rule built from smaller reusable checks | Define atomic gates first; compose with `all`/`any`/`not` + nested `gate:` refs |

##### Composed Gate Example

Mirrors the core engine reference (`is_high_value` → `requires_manager_approval`); see ADR 016 and the core DAG workflow plan §2.2.3.

```yaml
# gates.yaml
- id: is_high_value
  name: High Value Requisition
  when:
    eq:
      field: inputs.totalAmount
      value: 10000

- id: requires_manager_approval
  name: Requires Manager Approval
  when:
    all:
      - gate: is_high_value
      - not:
          present: inputs.managerApprovalCode
```

Reference a gate from any `transitions[].when` (or from another gate's `when` to compose predicates):

```yaml
# flow.yaml excerpt
- id: route_after_review
  label: Route After Review
  kind: review
  hitlSlug: oracle-pr-review-form
  doneWhen: [reviewConfirmed]
  transitions:
    - target: manager_approval
      when:
        gate: requires_manager_approval
    - target: submit
      when:
        all: []
```

##### Semantic Validation

`resmate workflow validate` rejects gate authoring errors at load/push time with actionable paths:

| Error | Cause | Typical error path |
| :--- | :--- | :--- |
| Duplicate gate id | Two `gates.yaml` entries share the same `id` | `gates[n].id` |
| Unknown gate reference | `{ gate: "<id>" }` references an id not defined in `gates.yaml` | `flow.stages[<stageId>].transitions[n].when` or `gates[n].when` |
| Gate reference cycle | Gate A references gate B which references gate A (directly or transitively) | Cycle path in message, e.g. `"is_high_value -> requires_manager_approval -> is_high_value"` |

**Static vs runtime:** Semantic validation catches typos, dangling references, and cycles at author time. A separate **runtime recursion guard** in the advance engine still returns `false` (without panicking) if a cyclic gate is evaluated at runtime — for example when validation was bypassed by a direct API push. Static validation and the runtime guard are complementary (ADR 016 §1).

**Push/pull:** Split YAML pull materializes `gates.yaml` when the definition includes gates; an empty `gates: []` on pull removes a stale local `gates.yaml`. Bundle push includes the `gates` array unconditionally.

Canonical fixture: `named-gates-v1` (ResMate CLI test fixtures).

#### 2.4.6 Path-Aware Completeness

Once a workflow branches, a field tied to a **skipped** stage must not block advancement on the path that legitimately bypasses it. The engine derives "in scope" from the instance **active path** — not from every stage in the definition.

**Active path** (`compute_active_path`) is the union of:

1. **Historical path** — `initialStage` plus every `to` target recorded on `stage_advance` / `stage_back_to` audit entries (stages the instance has actually visited).
2. **Projected path** — starting from the current stage (or an explicit `for_stage` override), walk forward by evaluating each stage's `transitions` in order (falling back to legacy `next`), stopping at a terminal stage or when a cycle is detected.

Terminal stages are excluded from the active path set. Stages on the branch not taken — historically or projected — are never inserted, so their `requiredFromStage` fields are excluded by construction.

**`required` vs `requiredFromStage`**

| Mechanism | Scope | Use when |
| :--- | :--- | :--- |
| `required: true` | Always enforced for the instance (global) | Field must be present before any terminal state, regardless of path |
| `requiredFromStage: <stageId>` | Enforced only when `<stageId>` is on the active path | Field belongs to a stage that may be skipped by conditional branching |

Rules:

- Prefer **`requiredFromStage`** over blanket **`required: true`** for fields owned by skippable stages.
- **Never set both** on the same field. If both are present, **`required: true` wins** and `requiredFromStage` is ignored.
- `requiredFromStage` must reference an existing stage `id` in `flow.yaml` (validated by `resmate workflow validate`).

**Skip-branch example** — VIP fast-track bypasses `collect_line_items`; `expressReason` is required only on the express path:

```yaml
# schema.yaml excerpt
fields:
  - key: expressReason
    label: Express review reason
    type: string
    bag: inputs
    requiredFromStage: express_review   # NOT required: true — stage is skippable

  - key: lineItems
    label: Line items
    type: array
    bag: inputs
    requiredFromStage: collect_line_items   # required on standard path only
```

```yaml
# flow.yaml excerpt — collect_vendor branches on inputs.vip
- id: collect_vendor
  kind: collect
  doneWhen: [vendorId]
  transitions:
    - target: express_review
      when:
        eq:
          field: inputs.vip
          value: true
  next: collect_line_items

- id: express_review
  kind: collect
  doneWhen: [expressReason]
  next: review_summary

- id: collect_line_items
  kind: collect
  doneWhen: [lineItems]
  next: review_summary
```

When `vip == true`, the instance visits `express_review` and `expressReason` is in scope; `lineItems` is not. When `vip` is false or absent, `collect_line_items` is on the path and `lineItems` is required; `expressReason` is not.

Canonical fixture: `conditional-branch-v1` (shipped with ResMate CLI test fixtures and workspace examples).

#### 2.4.7 Runtime Routing Metadata — `allowedNext` / `lastTransition`

Every workflow instance snapshot projection includes two **read-only** fields for planners and UIs. They are **not authored** in YAML — the platform computes them from `flow.yaml` and `gates.yaml` at snapshot time (ADR 016 §2).

| Field | Type | Semantics |
| :--- | :--- | :--- |
| `allowedNext` | `array<string>` | **Static view** of the current stage's declared routing: ordered, deduplicated transition targets from `stage.transitions[].target`, with the legacy `next` fallback appended if not already listed. Independent of whether any `when` condition currently evaluates to `true` — useful for rendering "this stage can lead to A, B, or C" without re-implementing condition evaluation. Empty for a terminal stage or a stage with no transitions/`next`. |
| `lastTransition` | `object` \| `null` | Most recent **real** stage movement as `{ from, to, action, timestamp }` (`WorkflowTransitionSummary`). Only `stage_advance` and `stage_back_to` audit actions count; halt markers (`stage_advance_halted_cycle`, `stage_advance_halted_max_hops`) are skipped because they represent "nothing happened." `null` for a fresh instance with no transition history. |

**Static vs live:** `allowedNext` lists declared routes, not the single branch that would win if conditions were evaluated now. For live auto-advance resolution, the engine still evaluates `transitions` in order at runtime (§2.4). `lastTransition` reflects audit history, not projected future paths.

---

## 3. Stage-Level Playbooks (`playbooks.yaml`)

While `flow.yaml` defines the structural skeleton of the state machine, **`playbooks.yaml`** defines the behavioral, conversational, and transactional instructions for each stage. 

Playbooks are injected dynamically into the **Orchestrator v4 loop** and **Agent Planner v4 ReAct loop** based on the active stage, ensuring the assistant adapts its tone, constraints, and background actions to the current execution phase with zero database round-trip overhead.

### 3.1 `playbooks.yaml` Schema

The `playbooks.yaml` file is a top-level dictionary where each key is a **Stage ID** mapping to a `StagePlaybookDef` object.

#### Playbook Definition Schema (`$defs/playbook`)

| Property | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `replyPolicy` | `object` | No | Governs conversational output parameters during this stage. |
| `constraints` | `array` | No | List of strict execution boundaries and "never-rules" injected into the prompt. |
| `assistantBrief` | `string` | No | Stage-specific instructions injected into the Master Orchestrator/Assistant prompt. |
| `agentBrief` | `string` | No | Stage-specific instructions injected into the Agent Planner prompt. |
| `onEntry` | `array` | No | List of transactional hook actions executed immediately upon entering this stage. |
| `onHitlSubmit` | `array` | No | List of transactional hook actions executed immediately when a HITL form is submitted. |
| `onSaveSuccess` | `array` | No | List of transactional hook actions executed immediately after a state patch is successfully saved. |
| `validatorRules` | `object` | No | Custom assertions evaluated by the platform to determine stage completeness. |

#### Reply Policy Schema (`replyPolicy`)

| Property | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `tone` | `string` | Yes | The conversational tone the assistant must adopt (e.g., `highly_professional`, `concise`). |
| `wordLimit` | `integer` | No | Hard limit on the length of assistant responses during this stage. |
| `instructions` | `array` | No | List of specific guidelines for formulating conversational replies. |

---

### 3.2 Transactional Hook Actions

Playbook hooks automate state changes, system integrations, and background processing during workflow transitions. Hooks are transaction-aware and execute on the mutable `WorkflowInstance` state before database writes are flushed.

Each hook action must specify an `action` type and its corresponding parameters:

#### 1. `mutate` (State Mutation)
- **Purpose**: Programmatically updates a field in the workflow instance's state.
- **Parameters**:
  - `path`: A JSONPath expression targeting a field in `inputs` or `artifacts` (e.g., `$.inputs.procurement_bu`).
  - `value`: The literal value or structured object to write.
- **Example**:
  ```yaml
  - action: "mutate"
    path: "$.inputs.procurement_bu"
    value: "RESMED_US_BU"
  ```

#### 2. `notify` (PubSub Notification)
- **Purpose**: Dispatches real-time events to internal system channels or message brokers.
- **Parameters**:
  - `channel`: The target PubSub channel name (e.g., `workflow_stages`).
  - `message`: The notification message payload.
- **Example**:
  ```yaml
  - action: "notify"
    channel: "workflow_stages"
    message: "Now entering vendor collection phase."
  ```

#### 3. `webhook` (External Integration)
- **Purpose**: Calls external APIs or webhooks asynchronously to prevent blocking database transactions.
- **Parameters**:
  - `url`: The target HTTP POST URL.
  - `payload`: Optional JSON payload. Supports dynamic interpolation (e.g., `{{workflowId}}`).
- **Example**:
  ```yaml
  - action: "webhook"
    url: "https://api.resmed.io/v1/oracle/sync-audit"
    payload:
      workflowId: "{{workflowId}}"
      stage: "collect_vendor"
  ```

#### 4. `triggerJob` (Asynchronous Agent Job)
- **Purpose**: Automatically schedules a background task for a specialized agent.
- **Parameters**:
  - `agent_slug`: The slug of the target agent to invoke.
  - `delegation_brief`: The instructions and context passed to the background agent.
- **Example**:
  ```yaml
  - action: "triggerJob"
    agent_slug: "oracle-line-extractor"
    delegation_brief: "Extract and compile line items from the uploaded invoice PDF."
  ```

---

### 3.3 Complete `playbooks.yaml` Example

```yaml
collect_vendor:
  replyPolicy:
    tone: "professional, concise, and helpful"
    wordLimit: 120
    instructions:
      - "Always greedily offer available vendors from previous search if any."
      - "Prompt the user politely for their Vendor ID if missing."
  constraints:
    - "Never call oracle-pr-vendor-save unless a numeric vendorId is supplied."
    - "If vendor is found in Oracle, never prompt user to create a new one."
  assistantBrief: |
    The user is currently in the vendor selection stage. Direct the conversation
    toward selecting a valid Oracle Vendor ID. Analyze user input to determine if
    a vendor query is implied. If so, execute the search-oracle-vendors tool.
  agentBrief: |
    Perform Oracle Vendor lookups based on the user's query. If matches are found, 
    map the top vendor to the 'vendorId' field. Explain any procurement business unit (BU)
    restrictions to the user if they select a vendor outside their primary business unit.
  onEntry:
    - action: "mutate"
      path: "$.inputs.procurement_bu"
      value: "RESMED_US_BU"
    - action: "notify"
      channel: "workflow_stages"
      message: "Entered Vendor Collection Stage"
  onHitlSubmit:
    - action: "mutate"
      path: "$.artifacts.vendor_verification_status"
      value: "VERIFIED"
  onSaveSuccess:
    - action: "webhook"
      url: "https://api.resmed.io/v1/oracle/sync-audit"
      payload:
        workflowId: "{{workflowId}}"
        stage: "collect_vendor"
  validatorRules:
    requiredVendorKeys: ["vendorId", "vendorName"]
    verifyActiveBU: true

collect_line_items:
  replyPolicy:
    tone: "analytical and precise"
    wordLimit: 150
    instructions:
      - "Present extracted line items in a clean markdown table."
      - "Clearly highlight any missing mandatory fields (e.g., Unit Price, Quantity)."
  constraints:
    - "Never allow totalAmount to exceed the sum of line item prices."
  assistantBrief: |
    Help the user compile line items. If a document is uploaded, route to the oracle-pr-agent
    to perform automated extraction.
  agentBrief: |
    Use the oracle-line-extractor tool to process invoice or quote files. Ensure every extracted
    line item contains a valid description, quantity, unit price, and category code.
  onEntry:
    - action: "notify"
      channel: "workflow_stages"
      message: "Entered Line Items Collection Stage"
```

---

### 3.4 Playbook-Aware Validator Compose Framing

When the macro validator returns `fail_user` because required fields for the remaining workflow are still missing, compose framing depends on the active stage kind and playbook — authors should set `replyPolicy` or `assistantBrief` on mid-workflow stages where conversational tone matters.

| Scenario | Stage kind | Playbook | Compose behavior |
| :--- | :--- | :--- | :--- |
| **Soft failure** | `collect` or `agent_task` | Active stage playbook has `replyPolicy` or `assistantBrief` | Instructions **not** to use failure framing; follow playbook tone and word limit; ask only for the immediate next required input(s). Validator gaps are appended in a delimited details block for the LLM — not dumped verbatim to the user. |
| **Hard failure** | `review` or `terminal` | Any (or none) | Existing behavior unchanged — list missing fields, "could not complete" framing, honest-failure instructions. |
| **Hard failure** | `collect` or `agent_task` | No `replyPolicy` or `assistantBrief` on active playbook | Falls back to hard failure framing (same as review/terminal). |

This is runtime compose behavior only — no CLI or author-file changes are required beyond defining stage playbooks. See ADR 017 §3 for the engine decision record.

### 3.5 Hook Execution Lifecycle Flow

The following diagram illustrates the transaction-aware execution sequence of playbook hooks and state persistence within the Smriti state engine:

```text
               +-------------------------------------------+
               |          Workflow Patch Applied           |
               +---------------------+---------------------+
                                     |
                                     v
               +---------------------+---------------------+
               |      Process 'onHitlSubmit' Hooks         | (If patch source is HITL)
               |      Process 'onSaveSuccess' Hooks        |
               +---------------------+---------------------+
                                     |
                                     v
               +---------------------+---------------------+
               |     Evaluate Stage Transition Conditions  |
               +---------------------+---------------------+
                                    / \
                                   /   \
                    Transition?   /     \   No Transition?
                                 v       v
            +-----------------------+   +-----------------------+
            | Run 'onEntry' hooks   |   | Persist patched state |
            | of the new stage      |   | to Smriti Database    |
            +-----------+-----------+   +-----------------------+
                        |
                        v
            +-----------------------+
            | Write transition event|
            | to execution_ledger   |
            +-----------+-----------+
                        |
                        v
            +-----------------------+
            | Persist final state   |
            | to Smriti Database    |
            +-----------------------+
```

---

## 4. Stage Transitions & Execution States

The ResMate platform coordinates workflows through four specialized stage kinds, each governed by strict transition boundaries.

```text
  [collect] ──(doneWhen: fields)──> [agent_task] ──(doneWhen: validator_pass)──> [review] ──(doneWhen: fields)──> [terminal]
      │                                                                             │
      └─────────────────────────────────<───(back_to)───────────────────────────────┘
```

### 4.1 Stage Kinds

1. **`collect` (Information Gathering)**:
   - **Purpose**: Gathers human-supplied data or external inputs.
   - **Behavior**: Typically binds to a `hitlSlug` representing a structured form. The active agent prompts the user to complete the form, or extracts data from conversational context to pre-fill it.
   - **Transition**: Governed by a list of required field keys in `doneWhen`.

2. **`agent_task` (Automated Execution)**:
   - **Purpose**: Executes background processing, integrations, or complex reasoning.
   - **Behavior**: Assigned to a specific `agentSlug`. The Agent Planner v4 ReAct loop executes tools to achieve the `expectedOutcome`.
   - **Transition**: Typically governed by `"validator_pass"`. The platform evaluates the output and applies a programmatic validator to confirm completeness.

3. **`review` (Human Gate/Approval)**:
   - **Purpose**: Establishes a human-in-the-loop gate before executing critical or irreversible actions.
   - **Behavior**: Displays a summary card to a human reviewer via the `hitlSlug`. The reviewer can approve, reject, or modify the collected data.
   - **Transition**: Governed by approval fields (e.g., `reviewConfirmed`). If rejected, route back via a `transitions` rule whose `target` is an earlier stage id and declare `reset` to clear stale gating fields (see §2.4.3). Legacy `back_to` is a metadata hint only.

4. **`terminal` (End State)**:
   - **Purpose**: Marks the successful or unsuccessful completion of the business process.
   - **Behavior**: Freezes the workflow instance. No further patches or transitions are permitted.
   - **Transition**: Governed by `"terminal"`.

### 4.2 Transition Boundaries (`doneWhen`)

The **Prajna Platform Auto-Advances** stages automatically. The state transition engine evaluates the active stage's `doneWhen` condition immediately after any state modification (such as a tool returning a `workflowPatch` or a HITL form submission):

- **Field Array Evaluation**: If `doneWhen` is an array of field keys (e.g., `[vendorId, totalAmount]`), the engine checks if all listed fields are present and non-null in the workflow instance's `inputs` or `artifacts` bags. If satisfied, the engine automatically transitions the instance to the stage specified in `next`.
- **Validator Pass Evaluation**: If `doneWhen` is `"validator_pass"`, the stage remains active until an authorized supervisor or validation tool asserts a pass condition.
- **Strict State Ownership**: Tools and handlers **must never** return an `advanceStage` instruction or attempt to mutate the stage ID directly. State ownership belongs strictly to the platform. Tools supply state patches, and the platform's state machine handles transitions.

---

## 5. State Save Tools & Workflow Schema Mapping

Tools named `*-save` or `*-confirm` are specialized **workflow patch tools** designed to update the state of a workflow instance. They do not write directly to databases; instead, they return a `workflowPatch` which the platform normalizes and applies to the workflow instance.

### 5.1 Tool Return Conventions

Depending on the operational role, your tools should structure their output according to the following conventions:

| Tool Role | Operational Outcome | Required Return Payload |
| :--- | :--- | :--- |
| **Input Save Tool** | Saves human/user data collected via HITL forms or conversation to the inputs bag. | `{ "success": true, "workflowPatch": { "inputs": { "fieldKey": "value" } } }` |
| **Artifact Produce Tool** | Saves automated/agent outcomes or external document refs to the artifacts bag. | `{ "success": true, "workflowPatch": { "artifacts": { "artifactKey": "value" } } }` |
| **Submit / Side-Effect Tool** | Triggers an external transaction or system commit (e.g., committing to Oracle API). | `{ "success": true, "externalRefs": { "oraclePrNumber": "PR-88219" } }` + status tracking via Prajna. |

### 5.2 The `workflowPatch` Contract

A workflow patch tool must return a structured JSON object containing a `workflowPatch` property. This property maps fields directly to the `inputs` or `artifacts` bags defined in `schema.yaml`.

#### JSON Return Contract

```json
{
  "success": true,
  "workflowPatch": {
    "inputs": {
      "vendorId": "VEND-99281",
      "totalAmount": 12500.00
    },
    "artifacts": {
      "vendor_verification_status": "VERIFIED"
    }
  }
}
```

### 5.3 Field Alignment Checklist

To ensure a field collected via HITL or an agent is correctly persisted and triggers stage advancement, developers must align keys across multiple layers:

1. **`schema.yaml`**: Declare the field `key`, specify its `bag` (`inputs` or `artifacts`), define its `requiredFromStage`, and map it to `hitlFieldId`.
2. **HITL `config.json`**: Ensure the form field ID matches the `hitlFieldId` declared in the schema.
3. **Save Tool**: Ensure the handler script returns a `workflowPatch` targeting the exact schema `key` in the correct bag.
4. **`flow.yaml`**: Ensure the stage's `doneWhen` array includes the schema `key` to trigger automatic stage advancement once populated.

```text
┌────────────────────────┐      ┌────────────────────────┐      ┌────────────────────────┐
│      schema.yaml       │      │    HITL config.json    │      │    Save Tool Handler   │
│                        │      │                        │      │                        │
│ key: vendorId          │ ───> │ "id": "vendorId"       │ ───> │ workflowPatch: {       │
│ hitlFieldId: vendorId  │      │ (Form Input Field)     │      │   inputs: {            │
│ bag: inputs            │      │                        │      │     vendorId: val      │
│ └──────────────────────┘      └────────────────────────┘      │   }                    │
│                                                               │ }                      │
│                                                               └────────────────────────┘
```

### 5.4 Example JS Save Tool Handler

Below is a complete, production-grade JavaScript handler demonstrating how a save tool performs validation, interacts with platform polyfills, and returns a `workflowPatch` to update the workflow instance. It is written as a **top-level V8 sandbox script** (no `require`, no `module.exports` — see [sdk-response-patterns.md §4](sdk-response-patterns.md#4-js-anti-patterns)) and uses the **dual-unwrap pattern** to safely read the `queryRecords` result:

```javascript
// tools/oracle-pr-vendor-save/handler.js
try {
  const input = context?.input || {};
  const vendorId = input.vendorId;

  if (!vendorId) {
    return JSON.stringify({
      success: false,
      error: "Missing mandatory parameter: vendorId"
    });
  }

  // 1. Fetch credentials securely using the JS sandbox polyfill (sync, { success, value, error })
  const secretRes = getSecret("ORACLE_INTEGRATION_KEY");
  if (!secretRes.success) {
    return JSON.stringify({
      success: false,
      error: "Secret not found: " + secretRes.error
    });
  }
  const apiKey = secretRes.value;

  // 2. Query internal records to verify vendor existence — dual-unwrap the single record
  const result = await queryRecords([{ collectionName: "oracle_vendors", query: { id: vendorId } }]);
  const vendor = result?.data?.[0]?.[0] ?? result?.[0]?.[0];

  if (!vendor) {
    return JSON.stringify({
      success: false,
      error: `Vendor ID '${vendorId}' is not registered in Oracle.`
    });
  }

  // 3. Return a workflowPatch to update the inputs and artifacts bags
  return JSON.stringify({
    success: true,
    workflowPatch: {
      inputs: {
        vendorId: vendor.id,
        totalAmount: input.totalAmount || 0
      },
      artifacts: {
        vendor_verification_status: "VERIFIED",
        vendor_name: vendor.name
      }
    }
  });
} catch (error) {
  return JSON.stringify({
    success: false,
    error: `Failed to save vendor state: ${error?.message || String(error)}`
  });
}
```

---

## 6. Assistant Linking & Orchestration Contract

Assistants declare links to workflows under their `systemContext` block using the standard **Orchestration Contract**.

### 6.1 Orchestration Contract Specification

Include a markdown section titled `## Orchestration Contract` inside your assistant's `systemContext`. This instructs the Master Assistant Planner how to detect user intents, instantiate workflow engines, and assign jobs aligned with the active stage.

```yaml
systemContext: |-
  You are the Oracle PR Assistant. Your job is to help users manage purchase requisitions.

  ## Orchestration Contract

  outcome_profile: "narrative_synthesis"
  workflow_type: "purchase_requisition"
  workflow_definition_slug: "oracle-purchase-requisition-v1"

  orchestrator: |
    Ensure an active workflow instance exists (session-bound). If none exists, instantiate one.
    Assign jobs aligned with the active workflow.stage and stage kind.
    Allow the user to submit human-in-the-loop forms.
```

### 6.2 Key Fields

- `workflow_type`: Serves for intent detection and filtering inside Smriti.
- `workflow_definition_slug`: Identifies the exact workflow blueprint slug (`meta.slug`) to instantiate.

---

## 7. End-to-End Workflow Recipes (Golden Paths)

These golden path architectures serve as blueprints for implementing the most common ResMate workflow styles.

### Recipe 1: Standard Workflow Form Wizard (collect → review → terminal)

Used for multi-stage human form collection followed by a review step and terminal submit.

```text
  [collect_vendor] ──> [collect_line_items] ──> [review_summary] ──> [completed]
```

#### Workspace Layout
```text
my-workspace/
├── workflows/purchase-requisition/
│   ├── meta.yaml
│   ├── schema.yaml
│   └── flow.yaml
├── hitl/oracle-pr-vendor-form/           # HITL form config for vendor
├── hitl/oracle-pr-lines-form/            # HITL form config for line items
├── hitl/oracle-pr-review-form/           # HITL form config for review
├── tools/oracle-pr-vendor-save/          # JS save tool (returns workflowPatch.inputs)
├── tools/oracle-pr-lines-save/           # JS save tool (returns workflowPatch.inputs)
├── agents/oracle-pr-agent.yaml           # Runs the HITL forms sequentially
└── assistants/oracle-pr-assistant.yaml   # Contains Orchestration Contract binding
```

#### Runtime Execution Rules
1. User interacts with assistant -> Assistant Planner detects intent and instantiates the workflow.
2. The active stage is `collect_vendor`. The agent loads `oracle-pr-vendor-form` and displays it.
3. User completes the form -> submits payload -> the agent invokes `oracle-pr-vendor-save` tool.
4. The save tool returns `{ success: true, workflowPatch: { inputs: { vendorId: "123" } } }`.
5. The platform applies the patch. Since `doneWhen: [vendorId]` is satisfied, the platform **automatically advances** the stage to `collect_line_items`.
6. This cycle repeats until the `review_summary` stage is confirmed, after which it enters the `completed` terminal stage.

---

### Recipe 2: Autonomous Agent Task (collect → agent_task → terminal) with Validator Pass

Used for autonomous background processing (such as automated PR creation, file parsing, or git commits) where stage advancement must wait for an LLM validator to assert success criteria.

```text
  [collect_intent] ──(doneWhen: inputs)──> [generate_and_open_pr] ──(doneWhen: validator_pass)──> [submitted]
```

#### Workspace Layout
```text
my-workspace/
├── workflows/github-pr-create/
│   ├── meta.yaml
│   ├── schema.yaml
│   └── flow.yaml
├── hitl/github-pr-intent-form/           # Form to gather git repo and branch name
├── tools/github-pr-intent-save/          # Saves collected intent fields
├── tools/git-pr-create-tool/             # Performs PR creation, outputs artifacts (prUrl)
├── agents/github-pr-agent.yaml           # Executes the git PR creation tool
└── assistants/github-pr-assistant.yaml   # Binds workflow and holds validator rules
```

#### Stage Configuration in `flow.yaml`
```yaml
- id: generate_and_open_pr
  label: "Generate and Open PR"
  kind: agent_task
  agentSlug: "github-pr-agent"
  expectedOutcome: "PR opened on Github. Save the PR url to artifacts.prUrl."
  doneWhen: "validator_pass"
  next: submitted
```

#### Runtime Execution Rules
1. Once intent is collected and saved, the workflow advances to `generate_and_open_pr`.
2. Because `kind` is `agent_task`, the system triggers the `github-pr-agent`.
3. The agent runs the `git-pr-create-tool`. The tool outputs the PR link and returns:
   ```json
   {
     "success": true,
     "workflowPatch": {
       "artifacts": {
         "prUrl": "https://github.com/resmed/my-repo/pull/42"
       }
     }
   }
   ```
4. The platform applies the patch. However, because `doneWhen` is `"validator_pass"`, the platform **does not** advance the stage immediately on the patch.
5. Instead, the Master Supervisor executes the **Macro Validator** using the stage's `expectedOutcome` and the assistant's validation rules.
6. When the validator confirms `requirement_met`, the platform auto-advances the workflow to `submitted`.

---

### Recipe 3: Search → Dynamic HITL ChoiceSet → Workflow Save

Used when a user needs to select an option from a list of search results queried from an external database at runtime. The results are loaded dynamically, injected into a HITL ChoiceSet card, and saved to the workflow inputs.

```text
Turn 1: Search Query ──> [Search Tool] ──> Results List
Turn 2: [HITLConfig Tool] Injects Choices ──> Interactive ChoiceSet Shown
Turn 3: User Selection ──> [Save Tool] ──> workflowPatch.inputs ──> Auto-Advance
```

#### Turn-by-Turn Execution Lifecycle
- **Turn 1 (Search)**:
  - User says: "Search for procurement user John Smith".
  - Assistant delegates to `oracle-pr-agent` which runs the search tool.
  - The search tool queries the Oracle DB and returns a structured array of matching users:
    ```json
    { "success": true, "users": [{ "id": 4012, "name": "John Smith (US)" }, { "id": 5119, "name": "John Smith (UK)" }] }
    ```
- **Turn 2 (Dynamic HITL Injection)**:
  - The agent invokes a specialized **HITLConfig** tool.
  - The HITLConfig tool loads the card template, dynamically maps the search results to card choices, and returns the modified card. Written as a top-level V8 sandbox script using the dual-unwrap pattern and the singular `"hitlConfig"` collection name (see [sdk-response-patterns.md](sdk-response-patterns.md)):
    ```javascript
    // tools/select-requester-hitlconfig/handler.js
    try {
      const input = context?.input || {};
      const users = input.users || []; // Array passed from previous tool

      const result = await queryRecords([{ collectionName: "hitlConfig", query: { slug: "select-requester-form" } }]);
      const hitlRecord = result?.data?.[0]?.[0] ?? result?.[0]?.[0];

      if (!hitlRecord) {
        return JSON.stringify({
          success: false,
          error: "HITL record not found for slug: select-requester-form"
        });
      }

      const card = typeof hitlRecord.config === "string" ? JSON.parse(hitlRecord.config) : hitlRecord.config || {};

      // Inject users dynamically into ChoiceSet
      const choiceSet = card.body.find(item => item.type === "Input.ChoiceSet" && item.id === "requesterId");
      if (choiceSet) {
        choiceSet.choices = users.map(u => ({ title: u.name, value: String(u.id) }));
      }

      return JSON.stringify({ success: true, config: JSON.stringify(card) });
    } catch (err) {
      return JSON.stringify({ success: false, error: err?.message || String(err) });
    }
    ```
- **Turn 3 (User Submit & Save)**:
  - The user selects a requester and clicks submit.
  - The agent planner captures the submitted `requesterId` and executes the save tool.
  - The save tool returns the patch:
    ```json
    { "success": true, "workflowPatch": { "inputs": { "requesterId": "4012" } } }
    ```
  - The platform saves `requesterId` to inputs and auto-advances the stage since `doneWhen: [requesterId]` is met.

---

### Recipe 4: Long-Running Tool with Status Updates

Used when a tool performs slow multi-step calculations, external data crawls, or file compilations. It streams real-time progress updates back to the user via WebSocket streams while maintaining active execution context.

#### Progress Streaming Syntax

- **JS Tool (V8 Sandbox)**:
  Use the global `publishStatus` platform helper directly, as a top-level script (no `module.exports` wrapper):
  ```javascript
  try {
    publishStatus("Authenticating with procurement portal...");
    // perform auth...
    publishStatus("Extracting line items and verifying unit prices...");
    // perform processing...
    return JSON.stringify({
      success: true,
      workflowPatch: { inputs: { /* ... */ } },
      agentResponseContext: "List the completed lines and total calculated cost."
    });
  } catch (err) {
    return JSON.stringify({ success: false, error: err?.message || String(err) });
  }
  ```

- **FaaS Tool (Node/Lambda)**:
  Import the Kriya SDK:
  ```javascript
  import { kriya } from "/runtime/runtime-sdks/kriya.js";
  
  export async function handler(event) {
    await kriya.process.publishStatus("Initiating secure tunnel to corporate mainframe...");
    // slow work...
    return { success: true };
  }
  ```

#### Presentation with `agentResponseContext`
Always return `agentResponseContext` on your final tool payload. This provides a clear, high-level hint to the Agent Planner about how to present the result to the user, preventing prompt dilution.

---

## 8. Push Order & CLI Operations

Workflows must be compiled, validated, and uploaded to Smriti using the ResMate CLI.

### 8.1 Chronological Push Order
Because workflow compilation applies structural validation against human forms, tools, and agents, you must push workspace components in this exact order:

```text
1. HITL Forms  ──>  2. Workflows  ──>  3. Tools  ──>  4. Agents  ──>  5. Assistants
```

### 8.2 CLI Commands

- **Local Validation Only**:
  ```bash
  resmate workflow validate purchase-requisition
  ```
- **Push Workflow Definition (Smriti Append)**:
  ```bash
  resmate workflow push purchase-requisition
  ```
- **Pull Workflow Definition**:
  ```bash
  resmate workflow pull purchase-requisition
  ```

---

## 9. Comprehensive Author Checklist

Before deploying or checking in a new workflow definition, ensure all verification checks pass:

- [ ] **Field Alignment**: The schema key, HITL input ID, save tool `workflowPatch` key, and stage `doneWhen` array are perfectly matched.
- [ ] **Stage IDs**: Every key in `playbooks.yaml` matches a stage ID defined in `flow.yaml`.
- [ ] **Agent Task Outcomes**: Every stage with `kind: agent_task` has a clearly declared `expectedOutcome` to instruct the planner and Macro Validator.
- [ ] **Validator Pass Rules**: Any stage with `doneWhen: validator_pass` has a corresponding matching validation hook or assistant rules.
- [ ] **Save Tool Isolation**: Save tools only return `workflowPatch` payloads and **never** attempt to issue `advanceStage` parameters or mutate stage indices.
- [ ] **Security & PHI**: All sensitive medical, personal, or financial parameters are tagged with `phi: true` or `redactInPrompt: true`.
- [ ] **Local CLI Mock Validation**: Run `resmate doctor` and `resmate workflow validate <name>` with zero validation errors.
- [ ] **Transition Exhaustiveness**: Every non-terminal stage with `transitions` has either a legacy `next` fallback or a catch-all rule (`when: { all: [] }`).
- [ ] **Transition Targets**: Every `transitions[].target` references an existing stage `id`; no dangling targets.
- [ ] **Condition Field Paths**: `present`, `absent`, and `eq.field` paths resolve to keys declared in `schema.yaml` (use `inputs.<key>` or `artifacts.<key>` for clarity).
- [ ] **Transition Rule Ordering**: More specific `when` clauses appear before broader ones; catch-all or `next` fallback is last.
- [ ] **Path-Aware `requiredFromStage`**: Fields on skippable stages use `requiredFromStage` (not blanket `required: true`); every `requiredFromStage` value references an existing stage `id` in `flow.yaml`; skipped-branch fields do not block paths that bypass their owning stage.
- [ ] **Reset Completeness on Rework Loops**: Every back-edge `transitions` entry that returns to an earlier stage declares a `reset` list covering every field that the target stage's `doneWhen` (and any skipped intermediate stage's `doneWhen`) depends on, so re-entering the stage does not immediately re-satisfy `doneWhen` from stale data. Do not rely on legacy `back_to` for rework routing — use `transitions[].target` plus `reset` (see §2.4.3).
- [ ] **Gate Reuse over Duplication**: Any `Condition` referenced from more than one `transitions[].when` (or nested in multiple gates) is extracted into `gates.yaml` rather than copy-pasted; every `{ gate: "<id>" }` references an existing gate `id`; no reference cycles among gates (see §2.4.5).
