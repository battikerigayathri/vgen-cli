---
name: Init Phase 2 — Agent P0 Context (ID Lifecycle + Push/Wire)
overview: Thicken the authoring kit's skill documentation and templates to ensure developers and Cursor agents understand the ID lifecycle (omit -> push -> write-back), never invent Mongo ObjectIds locally, and follow the correct chronological push/wire order.
todos:
  - id: skill-index-and-router
    content: "resmed_vgen-cli — Update templates/workspace/.cursor/skills/vgen-use-case/SKILL.md with new router rows, short ID lifecycle section, and push-wire-re-push checklist"
    status: pending
  - id: id-lifecycle-doc
    content: "resmed_vgen-cli — Create templates/workspace/.cursor/skills/vgen-use-case/docs/id-lifecycle.md documenting omit -> push -> write-back lifecycle and anti-patterns"
    status: pending
  - id: push-pull-wire-doc
    content: "resmed_vgen-cli — Create templates/workspace/.cursor/skills/vgen-use-case/docs/push-pull-wire.md documenting phase_order, binding rules (slug vs Mongo id), and push-all wiring limits"
    status: pending
  - id: cli-commands-doc-update
    content: "resmed_vgen-cli — Expand templates/workspace/.cursor/skills/vgen-use-case/docs/cli-commands.md with push/pull write-back and wiring details"
    status: pending
  - id: rule-links-retarget
    content: "resmed_vgen-cli — Retarget broken platform/ and cli/ relative links in templates/workspace/.cursor/rules/vgen-use-cases.mdc to skill docs"
    status: pending
  - id: agents-md-update
    content: "resmed_vgen-cli — Update templates/workspace/AGENTS.md to point push order and ID lifecycle instructions directly to the skill docs"
    status: pending
  - id: examples-id-cleanup
    content: "resmed_vgen-cli — Strip or annotate invented/hardcoded Mongo ObjectIds from templates/workspace/examples/** YAML files"
    status: pending
isProject: false
---

# Phase 2 — Agent P0 Context (ID Lifecycle + Push/Wire)

**Status:** planned  
**Repo (only):** `resmed_vgen-cli`  
**Parent plan:** `docs/INIT-AUTHORING-KIT-GAPS-PLAN.md` — Phase 2 / Gaps #4 & #5  
**Do not implement:** Phase 3+ (SDK response patterns, HITL resume format, other repos, CLI binary changes)

---

## Goal

Ensure that developers and AI coding agents bootstrapping a new use case with `vgen init` (or refreshing an existing one via `vgen kit update`) have immediate, unambiguous context on the platform's ID lifecycle and deployment wiring rules. This prevents the severe anti-pattern of agents inventing MongoDB ObjectIds locally, which breaks platform synchronization and deployment.

---

## Scope

| In scope (under `templates/workspace/`) | Out of scope |
|-----------------------------------------|--------------|
| `templates/workspace/.cursor/skills/vgen-use-case/SKILL.md` | Changes in `teemo`, `pr-agent-v2`, `resmedai-core-framework` |
| `templates/workspace/.cursor/skills/vgen-use-case/docs/id-lifecycle.md` (NEW) | SDK response pattern cookbooks (Phase 3) |
| `templates/workspace/.cursor/skills/vgen-use-case/docs/push-pull-wire.md` (NEW) | HITL resume message contracts (Phase 3) |
| `templates/workspace/.cursor/skills/vgen-use-case/docs/cli-commands.md` | Any CLI binary or Rust code changes (unless needed for template packing) |
| `templates/workspace/.cursor/rules/vgen-use-cases.mdc` | CLI-side push-validation warnings for invented IDs (Phase 5) |
| `templates/workspace/AGENTS.md` | |
| `templates/workspace/examples/**` (YAML cleanup) | |

---

## Product decisions (locked for this plan)

Do not re-open these unless implementation reveals a hard conflict.

| # | Decision | Choice for Phase 2 |
|---|----------|--------------------|
| 1 | **ID Lifecycle Policy** | **Omit -> Push -> Write-Back -> Never Invent.** Local resource files must start with empty/omitted `id` fields. Pushing them registers them on the platform, which writes the assigned MongoDB ObjectId back to the local YAML. |
| 2 | **Chronological Push Order** | **HITL → Workflow → Tool → Agent → Assistant** (as defined in `src/push_plan.rs` `phase_order`). |
| 3 | **Binding Rules** | **Slug** for HITL/Workflow bindings; **Mongo ObjectId** for Tool -> Agent (`skills[]`) and Agent -> Assistant (`agents[]`) bindings. |
| 4 | **Push-All Wiring Limit** | `vgen push-all` orders by phase but **does not** auto-wire IDs into dependents. Authors/agents must manually wire written-back IDs after pushing each layer, then re-push the dependent layers. |
| 5 | **Example ID Cleanup** | **Strip or comment-out** all hardcoded MongoDB ObjectIds from the teaching examples under `examples/` to prevent copy-paste pollution. |

---

## Current baseline (verified)

- `SKILL.md` does not mention the ID lifecycle or push-wire-re-push checklist.
- There are no dedicated `id-lifecycle.md` or `push-pull-wire.md` documents in the kit.
- `vgen-use-cases.mdc` contains numerous broken relative links pointing to non-existent `../../platform/` and `../../tools/` directories.
- `AGENTS.md` does not highlight the push order or ID lifecycle.
- Multiple files under `templates/workspace/examples/` (e.g. Jira and GitHub PR examples) contain hardcoded MongoDB ObjectIds in both `id` fields and binding arrays (`skills`, `agents`), leading agents to copy-paste or invent them.

---

## Implementation steps

### 1. Update Skill Router — `templates/workspace/.cursor/skills/vgen-use-case/SKILL.md`

1. Add two new rows to the Quick-Routing Index table:
   - `docs/id-lifecycle.md`: "ID Lifecycle (Never Invent)" — Omit -> push -> write-back lifecycle and anti-patterns.
   - `docs/push-pull-wire.md`: "Push Order & ID Wiring" — Chronological push order, binding rules (slug vs Mongo id), and push-all limits.
2. Add a new section:
   ```markdown
   ## 4. ID Lifecycle & Deployment Wiring (CRITICAL)

   To prevent deployment failures and broken platform references, you must strictly follow these rules:
   
   1. **Never Invent MongoDB ObjectIds**: Never write or guess a 24-character hex ID locally.
   2. **Omit -> Push -> Write-Back**:
      - Leave the `id` field empty or omitted in your local YAML.
      - Push the resource to the platform (`vgen <type> push <name>`).
      - The platform generates the ID and the CLI writes it back to your local YAML.
   3. **Chronological Push & Wire Checklist**:
      - Push **HITL Forms** and **Workflows** first (bound via slugs).
      - Push **Tools** next -> copy written-back Tool IDs into Agent `skills[]`.
      - Push **Agents** next -> copy written-back Agent IDs into Assistant `agents[]`.
      - Push **Assistants** last.
   ```

### 2. Create ID Lifecycle Document — `templates/workspace/.cursor/skills/vgen-use-case/docs/id-lifecycle.md` (NEW)

Create a high-fidelity guide detailing:
- **The Lifecycle Flow**:
  1. **Local Creation**: Create `tool.yaml` or `agent.yaml` with `id: ""` or omit the `id` field entirely.
  2. **Platform Registration**: Run `vgen tool push <name>`.
  3. **Local Write-Back**: The CLI automatically writes the assigned Mongo ObjectId back to the local file.
  4. **Subsequent Updates**: Future pushes use this written-back ID to update the resource in place.
- **Strict Anti-Patterns**:
  - *Do not* copy-paste ObjectIds from examples or other environments.
  - *Do not* generate random 24-character hex strings.
  - *Do not* invent IDs for skills or agents before they are pushed.
- **Why It Matters**: Invented IDs cause `404 Not Found` or `403 Forbidden` errors on push because the platform does not recognize them, or they collide with existing platform resources.

### 3. Create Push/Wire Document — `templates/workspace/.cursor/skills/vgen-use-case/docs/push-pull-wire.md` (NEW)

Create a comprehensive guide detailing:
- **Canonical Push Order**:
  $$\text{HITL Form} \longrightarrow \text{Workflow} \longrightarrow \text{Tool} \longrightarrow \text{Agent} \longrightarrow \text{Assistant}$$
- **Binding Rules**:
  - **Slug-based**: Workflows and HITL forms are bound to tools using their human-readable slugs (e.g., `workflowSlug: "my-workflow"` or `hitlSlug: "my-form"`).
  - **ID-based**: Tools are bound to agents via their MongoDB ObjectIds in the `skills` array. Agents are bound to assistants via their MongoDB ObjectIds in the `agents` array.
- **The Push-Wire-Re-Push Loop**:
  - Explain that `vgen push-all` respects the phase order but **does not** automatically inject generated IDs into dependent files.
  - Detail the step-by-step loop:
    1. Run `vgen push-all` to push all initial layers (HITL, Workflows, Tools).
    2. Inspect the written-back Tool IDs in `tools/*/tool.yaml`.
    3. Manually copy those Tool IDs into `agents/*.yaml` under the `skills` array.
    4. Run `vgen agent push <name>` to deploy the agents.
    5. Inspect the written-back Agent IDs in `agents/*.yaml`.
    6. Manually copy those Agent IDs into `assistants/*.yaml` under the `agents` array.
    7. Run `vgen assistant push <name>` to deploy the assistant.

### 4. Expand CLI Command Reference — `templates/workspace/.cursor/skills/vgen-use-case/docs/cli-commands.md`

1. In Section 4 (Resource-Specific Push & Pull Commands), expand the **ID Write-Back** subsection to explicitly reference `id-lifecycle.md` and `push-pull-wire.md`.
2. Add a prominent warning block explaining that `push-all` does not auto-wire IDs and detailing the manual wiring steps required after a bulk push.

### 5. Retarget Rules Links — `templates/workspace/.cursor/rules/vgen-use-cases.mdc`

Identify and replace all broken relative links pointing to non-existent top-level directories with valid links to the skill-centric documentation:
- `[README.md](../../README.md)` -> `[README.md](../../README.md)` (keep if README exists at workspace root)
- `[platform/execution-model.md](../../platform/execution-model.md)` -> `[decision-matrix.md](../skills/vgen-use-case/docs/decision-matrix.md)`
- `[react-patterns.md](../../platform/react-patterns.md)` -> `[decision-matrix.md](../skills/vgen-use-case/docs/decision-matrix.md)`
- `[tools/tool-yaml-reference.md](../../tools/tool-yaml-reference.md)` -> `[spec-tool.md](../skills/vgen-use-case/docs/spec-tool.md)`
- `[agents/agent-yaml-reference.md](../../agents/agent-yaml-reference.md)` -> `[spec-agent.md](../skills/vgen-use-case/docs/spec-agent.md)`
- `[assistants/assistant-yaml-reference.md](../../assistants/assistant-yaml-reference.md)` -> `[spec-assistant.md](../skills/vgen-use-case/docs/spec-assistant.md)`
- `[hitl/README.md](../../hitl/README.md)` -> `[spec-tool.md](../skills/vgen-use-case/docs/spec-tool.md)`
- `[workflows/README.md](../../workflows/README.md)` -> `[spec-workflow.md](../skills/vgen-use-case/docs/spec-workflow.md)`
- `[state-and-workflows.md](../../platform/state-and-workflows.md)` -> `[spec-workflow.md](../skills/vgen-use-case/docs/spec-workflow.md)`

### 6. Update AGENTS.md — `templates/workspace/AGENTS.md`

Update the file to include a clear warning about the ID lifecycle and push order, pointing developers and agents directly to the new skill docs:
```markdown
## ⚠️ Critical Rules for Agents & Developers
- **Never Invent MongoDB ObjectIds**: Leave `id` fields empty or omitted. Let the CLI push and write-back assign them.
- **Follow Push Order**: HITL -> Workflow -> Tool -> Agent -> Assistant.
- **Wire After Push**: Copy written-back IDs into dependent arrays (`skills`, `agents`) and re-push.

Refer to the master router for complete details:
➡️ [Go to the Master Router & Index (.cursor/skills/vgen-use-case/SKILL.md)](.cursor/skills/vgen-use-case/SKILL.md)
```

### 7. Clean Up Example IDs — `templates/workspace/examples/**`

Scan and edit all example YAML files (Jira, GitHub PR, Purchase Requisition) to strip or annotate hardcoded MongoDB ObjectIds:
1. **`id` fields**:
   - Change `id: 6a354b8f...` to `id: ""` or omit the line entirely.
   - Add a comment above the field explaining that it should be left empty for initial push:
     ```yaml
     # Leave id empty; the CLI will write back the assigned ID on push
     id: ""
     ```
2. **Binding arrays (`skills`, `agents`)**:
   - Replace hardcoded IDs in arrays with descriptive placeholder comments:
     ```yaml
     skills:
       # - <tool-id-after-push> (e.g. copy the written-back ID from tools/jira-read-issue/tool.yaml here)
     ```
     ```yaml
     agents:
       # - <agent-id-after-push> (e.g. copy the written-back ID from agents/jira-agent.yaml here)
     ```

---

## Acceptance / exit criteria

- [ ] A freshly initialized workspace contains the updated `SKILL.md`, `cli-commands.md`, `AGENTS.md`, and `vgen-use-cases.mdc`.
- [ ] The new files `id-lifecycle.md` and `push-pull-wire.md` are present in the workspace's skill docs folder.
- [ ] `vgen-use-cases.mdc` contains zero broken relative links; all links resolve to valid files in the workspace.
- [ ] All example YAML files under `examples/` have their hardcoded MongoDB ObjectIds stripped or annotated with clear instructions.
- [ ] Running `vgen kit update` on an existing workspace successfully delivers all of these updated and new files.

---

## Test plan

1. **Verify Template Generation**:
   - Run `vgen init` in a temporary directory.
   - Verify that all new and modified files are generated correctly.
   - Check that `id-lifecycle.md` and `push-pull-wire.md` exist and contain the correct content.
2. **Verify Link Integrity**:
   - Open the temporary workspace in Cursor.
   - Verify that all relative links in `vgen-use-cases.mdc` and `SKILL.md` resolve correctly to existing files.
3. **Verify Example Cleanliness**:
   - Inspect the generated `examples/` directory.
   - Ensure no YAML files contain raw, unannotated MongoDB ObjectIds in `id` fields or binding arrays.
4. **Verify Kit Update Delivery**:
   - Create a mock ResMate workspace with an older kit version.
   - Run `vgen kit update`.
   - Verify that the new skill docs and updated rules/AGENTS.md are successfully written to the workspace.
