# Push Order & ID Wiring

This document explains **what order to push resources in**, **how bindings between layers work** (slug vs. Mongo ObjectId), and **why `resmate push-all` does not fully automate wiring for you**.

Read [id-lifecycle.md](id-lifecycle.md) first if you haven't — this doc assumes you already understand the omit -> push -> write-back lifecycle.

---

## 1. Canonical Push Order

Resources must be pushed in strict dependency order. This mirrors the `phase_order` the CLI itself uses internally (`src/push_plan.rs`) to sequence `push-all`:

$$\text{HITL Form} \longrightarrow \text{Workflow} \longrightarrow \text{Tool} \longrightarrow \text{Agent} \longrightarrow \text{Assistant}$$

Pushing out of order causes dangling-reference failures: an Agent that references a Tool ID that doesn't exist yet, or an Assistant that references an Agent ID that doesn't exist yet, will fail validation or push with a broken reference.

---

## 2. Binding Rules — Slug vs. Mongo ObjectId

Not every binding uses the same kind of reference. There are two distinct binding mechanisms on the platform:

| Binding | Mechanism | Example |
| :--- | :--- | :--- |
| **Workflow -> HITL** | **Slug** (human-readable string) | `flow.<stage>.hitlSlug: "purchase-req-vendor-form"` |
| **Agent -> Workflow** (workflow-bound agents) | **Slug** | `workflow_definition_slug: "purchase-requisition-v1"` inside the assistant's `systemContext` orchestration contract |
| **Agent -> Tool** | **MongoDB ObjectId** | `agents/*.yaml` `skills: ["6a354b754fffc3946d12c562", ...]` |
| **Assistant -> Agent** | **MongoDB ObjectId** | `assistants/*.yaml` `agents: ["6a354b8f4fffc3946d12c573", ...]` |

**Rule of thumb**: slugs are used wherever the reference is resolved by *name* at runtime (HITL forms and workflow definitions are looked up by slug so they're stable across environments); Mongo ObjectIds are used for direct object references between deployed platform records (`skills[]`, `agents[]`) because those bindings must point at one specific pushed record.

This is exactly why HITL Forms and Workflows can be pushed with zero advance wiring (slugs are known before push — you choose them), while Tool -> Agent and Agent -> Assistant bindings **cannot** be filled in until the target has actually been pushed and has a real ID.

---

## 3. The Push-Wire-Re-Push Loop

`resmate push-all` walks every reachable resource in `phase_order` (HITL, Workflow, Tool, Agent, Assistant) and pushes each one, performing create-or-update per resource based on whether its local `id` is empty (see [id-lifecycle.md](id-lifecycle.md)). Critically:

> ⚠️ **`push-all` does not auto-wire IDs into dependents.** It pushes each resource in the correct order and writes back each resource's *own* `id`, but it does **not** reach into `agents/*.yaml` to fill `skills[]` with the Tool IDs it just created, and it does **not** reach into `assistants/*.yaml` to fill `agents[]` with the Agent IDs it just created. That wiring step is manual (by a developer or an agent), every time new Tool/Agent records are created.

### Step-by-step loop

1. Run `resmate push-all --dry-run` to preview the chronological plan, then `resmate push-all --yes` to push all initial layers (HITL, Workflows, Tools — and Agents/Assistants too, but their `skills[]`/`agents[]` arrays are still empty or stale on this first pass if those IDs didn't exist locally yet).
2. Inspect the written-back Tool IDs in `tools/*/tool.yaml` (`id` field, now populated).
3. Manually copy those Tool IDs into the relevant `agents/*.yaml` file(s) under the `skills` array.
4. Run `resmate agent push <name>` to redeploy the agent with its now-correct `skills[]`.
5. Inspect the written-back Agent ID in `agents/*.yaml` (`id` field, now populated).
6. Manually copy that Agent ID into the relevant `assistants/*.yaml` file's `agents` array.
7. Run `resmate assistant push <name>` to redeploy the assistant with its now-correct `agents[]`.

On a **fresh bootstrap** (everything starting from empty IDs), expect to run this loop once per use case: `push-all` for the first pass, then two small targeted re-pushes (`agent push`, `assistant push`) after wiring.

On **subsequent iterations** (e.g. you added a new tool to an existing agent), you only need steps 2-4: push the new tool, copy its ID into `skills[]`, re-push the agent. The assistant's `agents[]` binding is unaffected unless you also added a brand-new agent.

### Quick reference

| Scenario | What to push, in order |
| :--- | :--- |
| New use case, first push | `push-all` -> copy Tool IDs into Agent `skills[]` -> `agent push` -> copy Agent ID into Assistant `agents[]` -> `assistant push` |
| Added a new Tool to an existing Agent | `tool push <new-tool>` -> copy its ID into `skills[]` -> `agent push <agent>` |
| Added a new Agent to an existing Assistant | (ensure the agent's own `skills[]` is already wired) `agent push <new-agent>` -> copy its ID into `agents[]` -> `assistant push <assistant>` |
| Edited only a Tool's `handler.js` | `tool push <tool>` — no downstream re-push needed, IDs are unchanged |
| Edited only Agent `systemInstructions` | `agent push <agent>` — no downstream re-push needed unless the Assistant caches agent metadata |

For the full CLI syntax of each push/pull command, see [cli-commands.md](cli-commands.md).
