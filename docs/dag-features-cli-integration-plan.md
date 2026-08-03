# DAG Workflow Features — CLI Integration Plan

This document is the CLI-side companion to [`resmedai-core-framework/docs/dag-workflow-implementation-plan.md`](../../resmedai-core-framework/docs/dag-workflow-implementation-plan.md) (referenced below as "the DAG plan"). It audits `resmate`'s workflow authoring, validation, push/pull, scaffolding, and documentation surfaces against the four DAG phases already shipped in `resmedai-core-framework` — Conditional Transitions, Path-Aware Completeness, Back-Edges & State Hygiene, and Named Gates — and lays out the concrete engineering work required to bring the CLI's authoring experience in line with what the runtime engine already supports.

Related core-framework ADRs: [015 — Back-edges](../../resmedai-core-framework/context/decisions/015-dag-workflow-phase3-back-edges.md), [016 — Named Gates](../../resmedai-core-framework/context/decisions/016-dag-workflow-phase4-named-gates.md), [017 — Path-Aware Completeness](../../resmedai-core-framework/context/decisions/017-dag-workflow-phase2-path-aware-completeness.md).

---

## 1. Executive Summary

`resmate` (this repo) depends on `resmedai-core-framework`'s `smriti_client` crate as a **local path dependency** (`Cargo.toml`: `smriti_client = { path = "../resmedai-core-framework/lib/smriti_client" }`). Every CLI code path that parses or semantically validates a workflow directory — `resmate workflow validate`, `resmate validate`, `resmate doctor`, `resmate workflow push`, `resmate graph`, `resmate push-all` — routes through `WorkflowDefinitionLoader::load_from_dir`, which is the *exact same* function the runtime engine uses. This is a deliberate architectural choice (confirmed by `src/workflow_loader.rs` and `src/workflow_validate.rs` delegating directly, with zero duplicated schema/validation logic in this repo) and it means one important thing up front:

> **The CLI does not need new Rust structs to *parse* `transitions`, `Condition`, `reset`, `requiredFromStage`, or `gates.yaml` — it already parses and semantically validates every DAG feature today, for free, because it is compiled against the live core-framework source tree.**

This finding changes the shape of the gap from "the CLI's parser is unaware of the DAG schema" (the assumption this plan started from) to a more precise and more actionable one: **the CLI's parsing/validation *engine* is current, but every human- and agent-facing *authoring surface* around it — documentation, scaffolding templates, and push/pull field-copy logic — still only knows about the pre-DAG linear model.** A developer (or an authoring agent) running `resmate init --recipe oracle-pr` today gets a workflow directory that validates cleanly, but contains zero examples of `transitions`, `gates.yaml`, `reset`, or `requiredFromStage`, and the only doc that describes `flow.yaml` (`spec-workflow.md`) has no section on any of them. The DAG engine phases are functionally invisible to anyone authoring through this CLI, even though the plumbing underneath already supports them completely.

Concretely, this plan identifies five real gaps, in priority order:

1. **Documentation gap (highest impact, lowest risk)** — `spec-workflow.md` documents `requiredFromStage` and mentions `gates.yaml` in the layout table, but has no schema section for `transitions`, `Condition` (all 7 variants), `reset`, `gates.yaml`'s own schema, or the `allowedNext`/`lastTransition` snapshot fields. Authors and coding agents have no in-repo reference for any DAG feature beyond one field name.
2. **Scaffolding gap** — all three shipped workflow templates (`recipes/form-wizard`, `recipes/oracle-pr`, `workspace/examples/purchase-requisition`) are 100% legacy `next`-only `flow.yaml` files with no `gates.yaml` and no commented-out DAG feature examples.
3. **Push/pull robustness gap** — `specs/workflow.rs::write_workflow_from_definition` (the function `resmate workflow pull` calls to materialize files) hand-extracts a fixed list of top-level keys (`fields`, `stages`, `gates`, `initialStage`, ...) from the API response instead of mirroring the full split-file set the loader understands. This is the direct root cause of the `playbooks.yaml` pull bug (§2) and is a latent risk for `gates.yaml` and any future split file.
4. **Validation regression-lock gap** — the CLI inherits full server-grade semantic validation (dead-end detection, transition target existence, `reset` field existence, gate id uniqueness/cycle detection) via the path dependency, but this repo has **zero test fixtures of its own** that exercise DAG YAML through `validate_workflow_dir`. The inherited behavior is currently un-asserted from the CLI's perspective — a future refactor of the path dependency boundary (e.g. pinning a crates.io version, or changes to `WorkflowDefinitionLoader`'s error mapping) could silently regress CLI-side error reporting for these checks with no CLI test catching it.
5. **Developer ergonomics gap** — `resmate graph` renders cross-resource edges (workflow→HITL, workflow→agent) but has no visibility into intra-workflow DAG structure (`transitions`, `gates`); `mcp/tools.rs`'s `workflow_validate` MCP tool description and CLI help text do not mention DAG authoring at all.

Sections 3–8 below detail each gap and the concrete Rust/YAML changes required, organized to mirror the DAG plan's own structure (schema → validation → push/pull → scaffolding → docs → parallel subagent task breakdown).

---

## 2. Known Issue — `playbooks.yaml` Pull Bug (Fix Required, tracked separately)

**Status:** Being fixed by another engineer in parallel. Documented here for completeness and cross-reference only — **do not action this section as part of this plan's task breakdown (§8).**

### Symptom

`resmate workflow push <name>` correctly serializes stage-level playbooks into the pushed `authorBundle` — `src/workflow_loader.rs::definition_to_bundle` explicitly walks `def.stages`, collects each stage's `playbook` field, and assembles a `playbooks: { <stageId>: StagePlaybookDef }` map that is included in the outgoing bundle (this is only possible because `WorkflowDefinitionLoader::load_from_dir`, on the read side, already merges a sibling `playbooks.yaml` split file per `lib/smriti_client/src/workflow/loader.rs`). However, `resmate workflow pull <name>` calls `specs::write_workflow_from_definition(&workflow_dir, data)` (`src/specs/workflow.rs`), and that function's field-extraction block only reads `id`, `slug`, `name`, `workflowType`, `version`, `description`, `fields`, `stages`, `gates`, and `initialStage` off the response object — **it never reads a `playbooks` key and never writes a `playbooks.yaml` file.** The net effect: a workflow authored with stage playbooks, pushed successfully, and then pulled back down (e.g. onto a fresh clone, or after `resmate workflow pull` to sync a cloud-assigned ID) silently loses its `playbooks.yaml` file on disk, even though the remote definition still has the playbook data. Push is unaffected; only the local pull materialization is broken.

### Why this matters beyond `playbooks.yaml`

The root cause is structural, not a one-line typo: `write_workflow_from_definition` mirrors push's split-file set by an explicitly hand-maintained list of `data_obj.get(<key>)` calls rather than deriving the split-file set from the same source of truth the loader (`WorkflowDefinitionLoader`) and push-side bundler (`definition_to_bundle`) use. **This is exactly the same class of bug that will recur for `gates.yaml` if a future field is added to the pulled payload shape without a matching update to this function** — today `gates.yaml` happens to be handled correctly (the function does check `data_obj.get("gates")` and writes/removes `gates.yaml` accordingly, per `src/specs/workflow.rs` lines 199–202 and 275–282), but there is no structural guarantee that stays true as the schema evolves, because the three write paths (bundle-JSON, bundle-YAML, split-YAML) each separately hardcode their own key list.

### Guidance for the colleague's fix (and for this plan's own follow-up work)

When the `playbooks.yaml` fix lands, the CLI-side pull logic (`write_workflow_from_definition`) should be audited to confirm it **mirrors push for every workflow-adjacent split-file artifact** — `flow.yaml`, `schema.yaml`, `gates.yaml`, and `playbooks.yaml` — not just the one that was reported broken. §5 of this plan proposes a structural fix (deriving the split-file set from a single manifest instead of four independent hardcoded key lists) and a regression test that pulls-after-push for a fixture containing all four split files, so this class of bug cannot silently reappear for `gates.yaml` or any future file.

---

## 3. Schema/Spec Updates Needed

### 3.1 Finding: no new CLI-side structs are required for parsing

Investigation into `src/specs/workflow.rs`, `src/workflow_loader.rs`, and `src/workflow_validate.rs` shows the CLI has **no independent Rust type definitions for the workflow schema at all**. Every parse path funnels through `smriti_client::WorkflowDefinitionLoader`:

```180:161:/Users/roshankgujarathi/Workspace/ResMed/resmed_resmate-cli/src/workflow_loader.rs
pub fn load_workflow_from_dir(
    dir: &Path,
) -> Result<(Value, PathBuf, AuthorFormat), Box<dyn std::error::Error + Send + Sync>> {
    let def = WorkflowDefinitionLoader::load_from_dir(dir).map_err(map_smriti_err)?;
    let format = source_format_to_author_format(def.source_format);
    let version_path = version_file_path(dir, format)?;
    let bundle = definition_to_bundle(&def);
    let normalized = normalize::normalize_mongo_oids(bundle);
    Ok((normalized, version_path, format))
}
```

`WorkflowDefinitionLoader::load_from_dir` (in `resmedai-core-framework/lib/smriti_client/src/workflow/loader.rs`) already deserializes into the fully DAG-aware `WorkflowDefinition`/`WorkflowStageDef`/`Condition`/`TransitionRule`/`WorkflowGateDef` types defined in `lib/smriti_client/src/workflow/types.rs`, and already runs `validate_json_schema` + `validate_semantics` (§4) before returning. The CLI's own code operates on the resulting `serde_json::Value` bundle generically (`Value::get("stages")`, etc.), which means DAG fields inside each stage object (`transitions`, `reset` inside each transition, `requiredFromStage` on fields) pass through the CLI's push/pull/graph code paths untouched and unfiltered, purely because the CLI treats them as opaque JSON rather than re-typing them.

**Consequence:** there is no missing `struct` to add to make parsing work. What *is* missing is CLI-side *ergonomic access* to these types for the handful of call sites (§5, §6) that want to reason about DAG structure rather than pass it through blindly, plus the documentation/scaffolding that makes these fields discoverable to authors. §3.2 gives the actual upstream structs for reference (since the plan calls for concrete struct definitions), and §3.3 gives the one small, genuinely new addition recommended on the CLI side.

### 3.2 Reference: the DAG structs the CLI already inherits

These are already defined in `resmedai-core-framework/lib/smriti_client/src/workflow/types.rs` (lines 180–255) and are pulled in transitively via the `smriti_client` path dependency — reproduced here as the authoritative schema reference for anyone doing CLI work in this area:

```rust
/// Represents a transition rule from one stage to another based on a condition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionRule {
    pub target: String,
    pub when: Condition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset: Option<Vec<String>>,
}

/// Rich, JSON-safe conditional logic evaluation tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Condition {
    #[serde(rename = "present")]
    Present(String),
    #[serde(rename = "absent")]
    Absent(String),
    #[serde(rename = "eq")]
    Eq { field: String, value: Value },
    #[serde(rename = "all")]
    All(Vec<Condition>),
    #[serde(rename = "any")]
    Any(Vec<Condition>),
    #[serde(rename = "not")]
    Not(Box<Condition>),
    #[serde(rename = "gate")]
    Gate(String),
}

/// Reusable named predicate defined in `gates.yaml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowGateDef {
    pub id: String,
    pub name: String,
    pub when: Condition,
}
```

`WorkflowFieldDef.required_from_stage: Option<String>` and `WorkflowStageDef.transitions: Vec<TransitionRule>` (also in `types.rs`) round out the schema. All four are already `#[serde(default)]`/`Option`-annotated for backward compatibility, so v1 (pre-DAG) workflow directories continue to parse identically through the same loader.

### 3.3 Recommended new CLI-side addition: typed re-exports for DAG structure

Rather than duplicating these types, add a thin re-export module so future CLI code (graph visualization, scaffolding generators, lints) can reason about DAG structure without hand-parsing `serde_json::Value` paths like `stage.get("transitions").and_then(|t| t.as_array())`. This is the only new Rust code this plan recommends for the "schema" layer:

```rust
// src/specs/workflow_dag.rs (new file)

//! Typed re-exports of the DAG-era workflow schema, sourced from `smriti_client`.
//! Kept as a thin alias module — never redefine these types locally, or the CLI's
//! parsing will silently drift from the runtime engine's schema.

pub use smriti_client::{Condition, TransitionRule, WorkflowGateDef};

/// Extracts the `transitions` array of a single stage `Value` as typed `TransitionRule`s,
/// for CLI code (e.g. `resmate graph`, `resmate explain`) that wants structural access
/// rather than raw JSON traversal. Returns an empty vec for legacy `next`-only stages.
pub fn stage_transitions(stage: &serde_json::Value) -> Vec<TransitionRule> {
    stage
        .get("transitions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| serde_json::from_value::<TransitionRule>(t.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Extracts a workflow bundle's top-level `gates` array as typed `WorkflowGateDef`s.
pub fn bundle_gates(bundle: &serde_json::Value) -> Vec<WorkflowGateDef> {
    bundle
        .get("gates")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|g| serde_json::from_value::<WorkflowGateDef>(g.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
}
```

This module is consumed directly by the `resmate graph` DAG-edge rendering proposed in §6.3 and the scaffolding linter proposed in §6.2.

---

## 4. Validation Updates Needed

### 4.1 Finding: server-grade semantic validation is already inherited

`src/workflow_validate.rs::validate_workflow_dir` — the function backing `resmate workflow validate`, and (via `check_workflow_dirs` in `src/validate/workflow_rules.rs`) also backing `resmate validate` and `resmate doctor` — calls `WorkflowDefinitionLoader::load_from_dir` directly:

```26:44:/Users/roshankgujarathi/Workspace/ResMed/resmed_resmate-cli/src/workflow_validate.rs
pub fn validate_workflow_dir(dir: &Path) -> Result<WorkflowValidateData, WorkflowValidateError> {
    if !dir.is_dir() {
        return Err(WorkflowValidateError { code: "WORKFLOW_LAYOUT_INVALID", ... });
    }

    match WorkflowDefinitionLoader::load_from_dir(dir) {
        Ok(def) => Ok(WorkflowValidateData { ... }),
        Err(err) => Err(map_smriti_error(err)),
    }
}
```

And `WorkflowDefinitionLoader::load_from_dir` (`resmedai-core-framework/lib/smriti_client/src/workflow/loader.rs`, line 131) unconditionally runs `validate_semantics(&bundle)?` before constructing the definition. `validate_semantics` (`lib/smriti_client/src/workflow/validate.rs`) already implements every check this plan would otherwise need to add:

| Check | Implemented in core framework at | Mirrored to CLI via |
| :--- | :--- | :--- |
| Transition target stage exists | `validate.rs` line ~92 (`path: "flow.stages[id].transitions[idx].target"`) | `load_from_dir` → `WorkflowValidateError::WORKFLOW_SEMANTIC_INVALID` |
| `reset` field key exists in schema | `validate.rs` line ~101 (`path: "...transitions[idx].reset[n]"`) | same |
| Condition tree structurally valid (`when`) | `validate.rs` line ~113 | same |
| Non-terminal dead-end (transitions present, no exhaustive fallback, no `next`) | `validate.rs` line ~120–133, plan §4.2 | same |
| `requiredFromStage` references a real stage id | Phase 2 (ADR 017) — enforced alongside field/stage cross-checks | same |
| Gate id uniqueness | ADR 016 §1 — `HashSet<&str>` dedup over `gates[].id` | same |
| Gate reference resolution (`Condition::Gate` inside stage transitions *and* other gates) | ADR 016 §1 — `collect_gate_refs` helper, `validate.rs` line 222 | same |
| Gate reference cycle detection | ADR 016 §1 — `detect_gate_cycle`, `validate.rs` line 269, DFS with explicit recursion stack | same |

`map_smriti_error` in `src/workflow_validate.rs` already has a catch-all branch (`SmritiError::SemanticValidation { path, message }` → `WORKFLOW_SEMANTIC_INVALID`) that surfaces *any* semantic error the core framework raises, including all of the above, with the exact `path` string (e.g. `flow.stages[review_summary].transitions[1].reset[0]`) preserved for CLI users. **No new match arms, no new error codes, and no new validation logic are required in this repo for the CLI to correctly reject invalid DAG YAML.**

### 4.2 The actual gap: zero CLI-repo test coverage proving this

Because all of the above is inherited transitively, this repo's own test suite (`src/specs/workflow.rs`'s `#[cfg(test)]` module, `src/workflow_loader.rs`'s tests) only exercises the legacy fields (`meta.yaml`/`flow.yaml` round-trips with `next`, no `transitions`). There is no fixture in this repo that proves, from the CLI's own test suite, that:

- `resmate workflow validate` rejects a dangling `transitions[].target`.
- `resmate workflow validate` rejects a `reset` entry referencing an unknown field key.
- `resmate workflow validate` rejects a two-gate reference cycle in `gates.yaml`.
- `resmate workflow validate` rejects a non-terminal dead-end stage.
- `resmate workflow validate` **accepts** a well-formed DAG workflow (conditional branch + `gates.yaml` + `reset`) end-to-end through `validate_workflow_dir`.

This is a regression-lock gap, not a missing-feature gap: if the path dependency boundary ever changes (e.g. `smriti_client` is vendored, version-pinned from crates.io, or its error-mapping shape changes), these behaviors could silently stop working for CLI users with no CLI test failing. Add the following fixture-driven test module:

```rust
// src/workflow_validate.rs — new #[cfg(test)] additions

#[cfg(test)]
mod dag_semantic_tests {
    use super::*;
    use std::path::PathBuf;

    fn write_dag_fixture(dir: &std::path::Path, flow_yaml: &str, gates_yaml: Option<&str>) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("meta.yaml"), "name: DAG Fixture\nslug: dag-fixture-v1\nworkflowType: dag_fixture\nversion: 1\n").unwrap();
        std::fs::write(dir.join("schema.yaml"), "fields:\n  - key: vip\n    label: VIP\n    type: boolean\n    bag: inputs\n  - key: lineItems\n    label: Line Items\n    type: array\n    bag: inputs\n").unwrap();
        std::fs::write(dir.join("flow.yaml"), flow_yaml).unwrap();
        if let Some(gates) = gates_yaml {
            std::fs::write(dir.join("gates.yaml"), gates).unwrap();
        }
    }

    #[test]
    fn rejects_dangling_transition_target() {
        let dir = std::env::temp_dir().join(format!("resmate-dag-dangling-{}", uuid::Uuid::new_v4()));
        write_dag_fixture(&dir, r#"
initialStage: collect
stages:
  - id: collect
    label: Collect
    kind: collect
    doneWhen: [vip]
    transitions:
      - target: nonexistent_stage
        when: { present: vip }
  - id: done
    label: Done
    kind: terminal
    doneWhen: terminal
"#, None);

        let err = validate_workflow_dir(&dir).expect_err("dangling transition target must fail");
        assert_eq!(err.code, "WORKFLOW_SEMANTIC_INVALID");
        assert!(err.message.contains("nonexistent_stage") || err.message.contains("target"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_gate_reference_cycle() {
        let dir = std::env::temp_dir().join(format!("resmate-dag-gatecycle-{}", uuid::Uuid::new_v4()));
        write_dag_fixture(&dir, r#"
initialStage: collect
stages:
  - id: collect
    label: Collect
    kind: collect
    doneWhen: [vip]
    transitions:
      - target: done
        when: { gate: gate_a }
  - id: done
    label: Done
    kind: terminal
    doneWhen: terminal
"#, Some(r#"
- id: gate_a
  name: Gate A
  when: { gate: gate_b }
- id: gate_b
  name: Gate B
  when: { gate: gate_a }
"#));

        let err = validate_workflow_dir(&dir).expect_err("gate cycle must fail");
        assert_eq!(err.code, "WORKFLOW_SEMANTIC_INVALID");
        assert!(err.message.to_lowercase().contains("cycle"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn accepts_well_formed_conditional_branch_with_gates_and_reset() {
        let dir = std::env::temp_dir().join(format!("resmate-dag-accept-{}", uuid::Uuid::new_v4()));
        write_dag_fixture(&dir, r#"
initialStage: collect
stages:
  - id: collect
    label: Collect
    kind: collect
    doneWhen: [vip]
    transitions:
      - target: express
        when: { gate: is_vip }
      - target: collect_line_items
        when: { present: vip }
  - id: collect_line_items
    label: Collect Line Items
    kind: collect
    doneWhen: [lineItems]
    transitions:
      - target: express
        when: { eq: { field: lineItems, value: [] } }
        reset: [lineItems]
      - target: done
        when: { present: lineItems }
  - id: express
    label: Express
    kind: review
    doneWhen: terminal
  - id: done
    label: Done
    kind: terminal
    doneWhen: terminal
"#, Some(r#"
- id: is_vip
  name: Is VIP
  when: { eq: { field: vip, value: true } }
"#));

        validate_workflow_dir(&dir).expect("well-formed DAG workflow must validate");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
```

---

## 5. Push/Pull Updates Needed

### 5.1 `gates.yaml` — already correctly handled, confirm and lock in with tests

`src/specs/workflow.rs::write_workflow_from_definition` already round-trips `gates.yaml` correctly on pull:

```199:283:/Users/roshankgujarathi/Workspace/ResMed/resmed_resmate-cli/src/specs/workflow.rs
    let gates = data_obj
        .get("gates")
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    // ...
        AuthorFormat::SplitYaml => {
            // ...
            if !gates.as_array().map_or(true, |a| a.is_empty()) {
                std::fs::write(&dir.join("gates.yaml"), serde_yaml::to_string(&gates)?)?;
            } else {
                let gates_path = dir.join("gates.yaml");
                if gates_path.exists() {
                    let _ = std::fs::remove_file(gates_path);
                }
            }
        }
```

This is correct behavior (it even correctly *removes* a stale `gates.yaml` if the pulled definition has no gates, avoiding orphaned local files) and needs no functional change. What it needs is a **push→pull round-trip test**, since none currently exists for any file beyond `meta.yaml`/`schema.yaml`/`flow.yaml`:

```rust
// src/specs/workflow.rs — new test in the existing #[cfg(test)] mod tests block

#[test]
fn test_gates_yaml_round_trips_through_pull_write() {
    let dir = setup_temp_dir();
    let def = json!({
        "id": "wf_gates_1",
        "slug": "gates-round-trip",
        "name": "Gates Round Trip",
        "workflowType": "test_type",
        "version": 1,
        "initialStage": "collect",
        "fields": [{ "key": "vip", "label": "VIP", "type": "boolean", "bag": "inputs" }],
        "stages": [
            {
                "id": "collect", "label": "Collect", "kind": "collect",
                "doneWhen": ["vip"],
                "transitions": [{ "target": "done", "when": { "gate": "is_vip" } }]
            },
            { "id": "done", "label": "Done", "kind": "terminal", "doneWhen": "terminal" }
        ],
        "gates": [
            { "id": "is_vip", "name": "Is VIP", "when": { "eq": { "field": "vip", "value": true } } }
        ]
    });

    write_workflow_from_definition(&dir, &def).unwrap();
    assert!(dir.join("gates.yaml").is_file(), "gates.yaml must be written when gates are present");

    let (bundle, _, _) = load_workflow_from_dir(&dir).unwrap();
    assert_eq!(
        bundle.get("flow").unwrap().get("stages").unwrap()[0]
            .get("transitions").unwrap()[0]
            .get("when").unwrap().get("gate").unwrap().as_str().unwrap(),
        "is_vip",
        "transitions[].when.gate must survive the pull-write + reload round trip"
    );

    std::fs::remove_dir_all(dir).unwrap();
}
```

### 5.2 `playbooks.yaml` — confirm the colleague's fix against the same pattern

As documented in §2, this is being fixed elsewhere. Once fixed, the fix should be validated against a test structurally identical to §5.1's `gates.yaml` test (push a definition with a stage `playbook`, pull it back, assert `playbooks.yaml` exists and round-trips), since the current absence of any such test in this repo is exactly why the bug went unnoticed.

### 5.3 Structural recommendation: stop hand-listing split-file keys in four places

The deeper fix — recommended as follow-up hardening, not required to close the immediate `playbooks.yaml`/`gates.yaml` gaps — is that `write_workflow_from_definition`'s three format branches (`BundleJson`, `BundleYaml`, `SplitYaml`) each separately hardcode which top-level keys get written where. A single manifest-driven approach would make "did we forget a split file" structurally impossible to reintroduce:

```rust
// src/specs/workflow.rs — proposed refactor sketch

/// Declarative map of split-directory file name -> top-level bundle key(s) it is
/// materialized from. Adding a new split file (e.g. a future `hooks.yaml`) means
/// adding one entry here, not editing three format-specific write blocks.
const SPLIT_FILE_MANIFEST: &[(&str, &[&str])] = &[
    ("meta.yaml", &["id", "slug", "name", "workflowType", "version", "description"]),
    ("schema.yaml", &["fields"]),
    ("flow.yaml", &["initialStage", "stages"]),
    ("gates.yaml", &["gates"]),
    ("playbooks.yaml", &["playbooks"]),
];
```

This is a larger refactor than this plan's scope requires immediately (§8 schedules it as a Phase 2 stretch item), but it should be adopted as the long-term pattern precisely because it is the structural fix for the bug class described in §2, not just the one instance of it.

### 5.4 Push side: no changes needed

`src/workflow_loader.rs::definition_to_bundle` (used by `resmate workflow push`, `resmate validate`, `resmate graph`, `resmate diff`) already includes `gates: def.gates` and a `playbooks` map built from `def.stages[].playbook` in the outgoing bundle unconditionally — push has never dropped any DAG field, since `WorkflowDefinition` (the typed struct populated by the loader) already carries `transitions`, `gates`, and `required_from_stage` end-to-end. The bug is exclusively in the pull-side *write* function (§2, §5.1–5.3).

---

## 6. CLI Scaffolding/Templates Updates

### 6.1 Current state: three fully legacy templates

All three shipped workflow scaffolds are pure `next`-only, no DAG features, no `gates.yaml`:

| Template | Path |
| :--- | :--- |
| Minimal form-wizard recipe | `templates/recipes/form-wizard/workflows/sample-workflow/flow.yaml.tmpl` |
| Oracle PR recipe | `templates/recipes/oracle-pr/workflows/oracle-purchase-requisition/flow.yaml` |
| Purchase-requisition example | `templates/workspace/examples/purchase-requisition/workflows/purchase-requisition/flow.yaml` |

### 6.2 Update the Oracle PR recipe's `flow.yaml` with a real (default-off) conditional branch

The Oracle PR recipe is the best candidate to carry a *working* DAG example (not just a comment), since it already models a plausible "VIP fast-track" business case that mirrors the DAG plan's own `flow.yaml` example (§2.2.3 of the DAG plan). Update `templates/recipes/oracle-pr/workflows/oracle-purchase-requisition/flow.yaml`:

```yaml
initialStage: collect_requester
stages:
  - id: collect_requester
    label: Collect Requester
    kind: collect
    hitlSlug: roc-select-requester-form
    agentSlug: oracle-pr-agent
    doneWhen: [requesterId]
    # DAG feature (optional, disabled by default — this workflow ships with the
    # legacy `next` fallback below active). Uncomment to route pre-approved
    # requesters straight to submission, skipping any downstream review stage:
    #
    # transitions:
    #   - target: submit
    #     when:
    #       gate: is_pre_approved_requester
    next: submit
  - id: submit
    label: Submitted
    kind: terminal
    terminal: true
    doneWhen: terminal
```

And add a new, disabled-by-default `templates/recipes/oracle-pr/workflows/oracle-purchase-requisition/gates.yaml.example` (note the `.example` suffix — see §6.4 for why it should *not* ship as a live `gates.yaml`):

```yaml
# Rename to `gates.yaml` (drop the `.example` suffix) to activate.
# Referenced by the commented-out `transitions` block in flow.yaml.
- id: is_pre_approved_requester
  name: Requester Is Pre-Approved
  when:
    eq:
      field: inputs.requesterId
      value: "PRE_APPROVED"
```

### 6.3 Update the purchase-requisition example with `requiredFromStage` + a rework back-edge

`templates/workspace/examples/purchase-requisition/workflows/purchase-requisition/flow.yaml` already has a natural `review_summary → collect_line_items` rework case (a rejected review should send the requisition back for more line items). Today it has no `back_to`/rework path at all. Update it to demonstrate a real Phase 3 back-edge with `reset`:

```yaml
initialStage: collect_vendor
stages:
  - id: collect_vendor
    label: Collect vendor
    kind: collect
    hitlSlug: purchase-req-vendor-form
    agentSlug: purchase-req-agent
    doneWhen: [vendorId]
    next: collect_line_items

  - id: collect_line_items
    label: Collect line items
    kind: collect
    hitlSlug: purchase-req-lines-form
    agentSlug: purchase-req-agent
    doneWhen: [lineItems]
    next: review_summary

  - id: review_summary
    label: Review summary
    kind: review
    hitlSlug: purchase-req-review
    agentSlug: purchase-req-agent
    doneWhen: [reviewConfirmed]
    # DAG feature: conditional branching replaces the single unconditional `next`.
    # A rejected review (reviewConfirmed == false) routes back to line-item
    # collection instead of forward to submission, and clears the stale
    # reviewConfirmed/lineItems fields so the reviewer must re-confirm the
    # corrected data (Phase 3 rework loop — see the DAG plan §4.3).
    transitions:
      - target: submit
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

  - id: submit
    label: Submitted
    kind: terminal
    terminal: true
    doneWhen: terminal
```

And update the sibling `schema.yaml` in the same example directory to demonstrate `requiredFromStage` (today `required: true` is used unconditionally, which is exactly the pre-DAG pattern the DAG plan's Phase 2 (ADR 017) was built to replace):

```yaml
fields:
  - key: vendorId
    label: Vendor ID
    type: string
    bag: inputs
    required: true
    requiredFromStage: collect_vendor
  - key: lineItems
    label: Line Items
    type: array
    bag: inputs
    requiredFromStage: collect_line_items
  - key: reviewConfirmed
    label: Review Confirmed
    type: boolean
    bag: inputs
    requiredFromStage: review_summary
```

### 6.4 Keep the minimal `form-wizard` recipe DAG-feature-free, but comment-teach it

The `form-wizard` recipe (`templates/recipes/form-wizard/workflows/sample-workflow/flow.yaml.tmpl`) is intentionally the *minimal* starting point (per `src/scaffold.rs::known_recipes()` — `"minimal"` is a distinct, even smaller recipe). It should stay `next`-only by default so first-time authors aren't confronted with DAG concepts immediately, but should carry a teaching comment so the feature is discoverable without adding any live complexity:

```yaml
initialStage: collect
stages:
  - id: collect
    label: Collect
    kind: collect
    hitlSlug: sample-form
    doneWhen: [submitted]
    # This workflow uses the simple linear `next` field. For conditional branching,
    # rework loops, or reusable named predicates, replace `next` with a `transitions`
    # array — see docs/spec-workflow.md §2.4 "Conditional Transitions & gates.yaml".
    next: done
  - id: done
    label: Done
    kind: terminal
    terminal: true
    doneWhen: terminal
```

**Decision: omit a starter `gates.yaml` from the minimal and form-wizard recipes by default.** `gates.yaml` is optional at the loader level (absence is not an error) and is only useful once there is more than one place referencing the same predicate — introducing an empty or single-entry `gates.yaml` into the minimal scaffold would create dead weight for the common case (a two-stage linear form). The Oracle PR recipe (§6.2) is the correct place for a *reachable, working* `gates.yaml` example precisely because it is scaffolded to be extended, not because it is minimal.

### 6.5 Scaffolding linter (optional stretch item)

Using the `stage_transitions`/`bundle_gates` helpers from §3.3, add a `resmate doctor` info-level (non-blocking) hint — not an error — when a workflow directory uses only `next`/`back_to` with no `transitions` at all, pointing authors at the new docs section. This is explicitly informational (`Severity::Info` if the `Finding` enum supports it, otherwise a human-mode-only printed tip) since legacy linear workflows remain fully valid and are not being deprecated.

---

## 7. Documentation Updates Needed

### 7.1 Scope

The only workflow-authoring doc that ships inside this CLI's own template tree is `templates/workspace/.cursor/skills/resmate-use-case/docs/spec-workflow.md` (verified via directory listing — no `workflow-authoring.md` or `authoring-formats.md` exists under this repo's `templates/`). Those two filenames do exist as separate, already-DAG-plan-adjacent documents in the **`pr-agent-v2`** workspace (`workflows/workflow-authoring.md`, `workflows/authoring-formats.md`), which is a downstream *consumer* of this CLI's authoring conventions, not part of this repo. Updating `pr-agent-v2`'s copies is out of scope for this plan (different repo, different owner) but is flagged here explicitly as required follow-up once this plan's `spec-workflow.md` changes land, so the two stay in sync — `pr-agent-v2/workflows/authoring-formats.md` line 19 already independently mentions `gates.yaml` as "optional" in its layout table, confirming the drift is already starting in both directions.

### 7.2 `spec-workflow.md` — new §2.4 "Conditional Transitions, `gates.yaml`, and Field Resets"

Insert a new subsection immediately after the existing §2.3 (`flow.yaml`) and before §3 (`playbooks.yaml`), extending the existing Stage Definition Schema table (§2.3) with the missing `transitions` row:

```markdown
| `transitions` | `array` | No | Ordered list of conditional transition rules, evaluated top-to-bottom; the first rule whose `when` condition evaluates to `true` is taken. Falls back to `next` if no rule matches (or halts with a `no_matching_transition` audit entry and no fallback exists — see §2.4.4). |
```

Then the new section body:

```markdown
### 2.4 Conditional Transitions, `gates.yaml`, and Field Resets

While `next` and `back_to` express a single unconditional edge per stage, `transitions`
lets a stage route to *different* target stages depending on the current workflow
instance's field values — powering fast-track branches, conditional approval gates,
and rework loops without any code changes.

#### 2.4.1 The `Condition` Schema

Every `transitions[].when` (and every `gates.yaml[].when`) is a `Condition` — a small,
JSON-safe boolean expression tree. Exactly one of the following seven shapes is valid
per node:

| Variant | YAML Shape | Evaluates To |
| :--- | :--- | :--- |
| Present | `present: <field path>` | `true` if the field is present and non-empty. |
| Absent | `absent: <field path>` | `true` if the field is missing, null, or empty. |
| Equals | `eq: { field: <field path>, value: <any> }` | `true` if the field's value exactly equals `value`. |
| All (AND) | `all: [<condition>, ...]` | `true` if every sub-condition is `true`. |
| Any (OR) | `any: [<condition>, ...]` | `true` if any sub-condition is `true`. |
| Not | `not: <condition>` | Negates the sub-condition. |
| Gate | `gate: <gate id>` | Resolves and evaluates the named predicate from `gates.yaml`. |

**Field paths** may be namespaced (`inputs.vendorId`, `artifacts.prUrl`) or bare
(`vendorId`) — a bare key is resolved by looking up which bag (`inputs`/`artifacts`)
owns that key in `schema.yaml`.

```yaml
# All three of these are equivalent ways to express "vendorId is present":
transitions:
  - target: collect_line_items
    when:
      present: inputs.vendorId
  - target: collect_line_items
    when:
      present: vendorId
```

#### 2.4.2 `transitions` on a Stage

```yaml
- id: review_summary
  label: Review Requisition
  kind: review
  hitlSlug: review-page
  doneWhen:
    - reviewConfirmed
  transitions:
    - target: submit
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

Transitions are evaluated **in the order they are declared**; the first matching
condition wins. If none match and no legacy `next` is present, the stage cannot
advance (§2.4.4).

#### 2.4.3 `reset` — Clearing Fields on a Transition (Rework Loops)

Any transition — forward or backward — may declare a `reset: [<field key>, ...]`
list. When that transition is taken, every listed field is removed from its owning
bag (`inputs`/`artifacts`) before the target stage is entered. This is the mechanism
that makes rework loops safe: without it, a stage that routes back to an earlier
stage would find its own `doneWhen` fields still populated from the previous pass
and immediately bounce forward again. Every key in a `reset` list must be a real
field key declared in `schema.yaml` — `resmate workflow validate` rejects unknown
keys with an error path like `flow.stages[review_summary].transitions[1].reset[0]`.

#### 2.4.4 Dead Ends and the `next` Fallback

A non-terminal stage with `transitions` but no exhaustive coverage and no `next`
fallback is a **dead end**: if the instance's state doesn't match any declared
condition, the stage can never advance. `resmate workflow validate` / `resmate
doctor` catch this statically at author time. Always give a stage with conditional
transitions **either** an unconditional catch-all rule (e.g. `when: { all: [] }`)
**or** a legacy `next` fallback:

```yaml
- id: collect_vendor
  kind: collect
  doneWhen: [vendorId]
  transitions:
    - target: express_review
      when: { eq: { field: inputs.vendorId, value: "V-PRE_APPROVED" } }
  next: collect_line_items   # fallback: every other vendorId value takes this path
```

If a dead end is somehow reached at runtime anyway (e.g. a hand-pushed definition
that bypassed local validation), the platform halts auto-advance, logs a
`no_matching_transition` audit entry, and leaves the instance on the current stage
for a human or agent to correct — it never silently drops state.

#### 2.4.5 `gates.yaml` — Reusable Named Predicates

Once the same `Condition` is referenced from more than one place (multiple stages,
or nested inside another gate), extract it into `gates.yaml` — a sibling split file
next to `meta.yaml`/`schema.yaml`/`flow.yaml`, optional in the split-YAML layout:

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

Reference a gate from any `transitions[].when` (or from another gate, to compose
predicates) using `{ gate: <id> }`:

```yaml
transitions:
  - target: manager_approval
    when:
      gate: requires_manager_approval
  - target: submit
    when:
      all: []   # unconditional fallback
```

`resmate workflow validate` rejects duplicate gate ids, references to unknown gate
ids (from a stage transition *or* from inside another gate), and reference cycles
(e.g. gate A → gate B → gate A), with an actionable path pointing at the offending
`when` clause.

#### 2.4.6 `requiredFromStage` and Path-Aware Completeness

A field's `required: true` (§2.2) enforces it unconditionally, everywhere. When a
field should only be mandatory once the workflow has actually reached (or is
projected to reach) a particular stage — because a conditional branch might skip
that stage entirely — use `requiredFromStage` instead of `required`:

```yaml
fields:
  - key: expressReason
    label: Express Reason
    type: string
    bag: inputs
    requiredFromStage: express_review   # only required if express_review is on the active path
```

The platform computes each instance's **active path** — the stages actually
visited plus the stages a forward projection of the current `transitions` would
reach — and only enforces `requiredFromStage` fields whose stage is on that path.
A field owned by a stage that a given instance's branch skips entirely is never
flagged as missing, and never blocks that instance from reaching a terminal or
review stage. Never set both `required: true` and `requiredFromStage` on the same
field — `required: true` always wins and makes `requiredFromStage` a no-op.

#### 2.4.7 Runtime Routing Metadata (`allowedNext` / `lastTransition`)

Every workflow instance's snapshot projection (surfaced to planners/UIs, not
authored directly) now includes two read-only fields reflecting the DAG structure:

| Field | Type | Description |
| :--- | :--- | :--- |
| `allowedNext` | `array<string>` | The current stage's declared transition targets (plus the legacy `next` fallback if not already listed), independent of whether any condition currently evaluates `true`. Empty for a terminal stage. |
| `lastTransition` | `object \| null` | `{ from, to, action, timestamp }` summary of the most recent real stage movement (ignores internal safety-halt audit markers). `null` for a fresh instance. |

These are computed automatically from `flow.yaml`/`gates.yaml` — there is nothing
to author for them, but they are useful when building custom UI progress
indicators or planner prompts that want to describe "where can this instance go
next" without re-implementing condition evaluation.
```

### 7.3 `spec-workflow.md` §1.2 layout diagram — add `gates.yaml`

The existing directory tree in §1.2 omits `gates.yaml` even though the format comparison table in §1.1 already lists it as optional. Update the tree for consistency:

```text
workflows/purchase-requisition/
├── meta.yaml         # Workflow identity, slug, and versioning
├── schema.yaml       # Data fields, bags, and validation rules
├── flow.yaml         # Stage definitions, kinds, and transitions
├── gates.yaml        # Optional: reusable named predicates referenced from flow.yaml
└── playbooks.yaml    # Stage-specific briefs, constraints, and hooks
```

### 7.4 §9 Author Checklist — add DAG-specific items

Append to the existing checklist in `spec-workflow.md` §9:

```markdown
- [ ] **Transition Exhaustiveness**: Every non-terminal stage using `transitions` has either an unconditional fallback rule or a legacy `next`, so no reachable field-value combination results in a dead end.
- [ ] **Reset Completeness on Rework Loops**: Every back-edge `transitions` entry that returns to an earlier stage declares a `reset` list covering every field that stage's (and any skipped intermediate stage's) `doneWhen` depends on, so re-entering the stage doesn't immediately re-satisfy `doneWhen` from stale data.
- [ ] **Gate Reuse over Duplication**: Any `Condition` referenced from more than one `transitions[].when` is extracted into `gates.yaml` rather than copy-pasted.
- [ ] **`requiredFromStage` over blanket `required`**: Fields owned by a stage that can be legitimately skipped on some branch use `requiredFromStage`, not `required: true`, so skip-branch instances aren't blocked from completing.
```

---

## 8. Parallel Subagent Task Breakdown

Following the DAG plan's own §5 format, this work is divided into three phases, each executable by a two-agent sibling team.

### 8.1 Phase 1: Documentation & Scaffolding (highest priority — closes the visibility gap)

```
+-----------------------------------------------------------------------------------+
| Sibling A: Documentation (spec-workflow.md)| Sibling B: Scaffolding & Templates   |
+---------------------------------------------+-------------------------------------+
| 1. Add §2.4 "Conditional Transitions,       | 1. Update `recipes/oracle-pr/.../   |
|    gates.yaml, and Field Resets" (7 sub-    |    flow.yaml` with commented-out    |
|    sections: Condition schema, transitions, |    `transitions`/gate example (§6.2)|
|    reset, dead ends, gates.yaml, requiredFr-| 2. Add `gates.yaml.example` sibling |
|    omStage, allowedNext/lastTransition).    |    file to the oracle-pr recipe.    |
| 2. Add `transitions` row to the existing    | 3. Update `workspace/examples/      |
|    Stage Definition Schema table (§2.3).    |    purchase-requisition/.../        |
| 3. Update the §1.2 directory tree to        |    flow.yaml` with a real Phase 3   |
|    include `gates.yaml`.                    |    back-edge + reset (§6.3).        |
| 4. Append 4 new checklist items to §9.       | 4. Update the same example's        |
|                                              |    `schema.yaml` to use             |
|                                              |    `requiredFromStage` (§6.3).      |
|                                              | 5. Add a teaching comment (no live  |
|                                              |    feature) to the minimal          |
|                                              |    `form-wizard` recipe (§6.4).     |
+-----------------------------------------------------------------------------------+
```

### 8.2 Phase 2: Push/Pull Hardening & Regression Test Coverage

```
+-----------------------------------------------------------------------------------+
| Sibling A: Round-Trip Test Coverage        | Sibling B: Structural Refactor       |
+---------------------------------------------+-------------------------------------+
| 1. Add `test_gates_yaml_round_trips_        | 1. Coordinate with the colleague     |
|    through_pull_write` to                    |    fixing the playbooks.yaml bug     |
|    `src/specs/workflow.rs` (§5.1).           |    (§2) — confirm their fix follows |
| 2. Once the playbooks.yaml fix lands,        |    the same shape as the gates.yaml |
|    add the mirrored playbooks.yaml           |    handling, not a one-off patch.   |
|    round-trip test (§5.2).                   | 2. Implement the `SPLIT_FILE_       |
| 3. Add a bundle-format (workflow.yaml/       |    MANIFEST` refactor of             |
|    workflow.json) round-trip test for        |    `write_workflow_from_definition`  |
|    `transitions`/`gates`/`reset` fields,      |    (§5.3) so future split files      |
|    since §5.1's test only covers split       |    can't be forgotten the same way.  |
|    YAML.                                     | 3. Re-run all existing               |
| 4. Add the three DAG semantic-validation     |    `specs/workflow.rs` and           |
|    fixture tests to `src/workflow_           |    `workflow_loader.rs` tests to     |
|    validate.rs` (§4.2: dangling target,      |    confirm the refactor is behavior- |
|    gate cycle, well-formed accept).          |    preserving before landing.       |
+-----------------------------------------------------------------------------------+
```

### 8.3 Phase 3: CLI Ergonomics & Type Re-Exports

```
+-----------------------------------------------------------------------------------+
| Sibling A: Typed DAG Access Layer           | Sibling B: Graph/Doctor Enhancements|
+---------------------------------------------+-------------------------------------+
| 1. Add `src/specs/workflow_dag.rs` with     | 1. Extend `resmate graph` to render |
|    `stage_transitions`/`bundle_gates`        |    intra-workflow `transitions`     |
|    typed re-export helpers (§3.3).           |    edges (stage -> stage) and       |
| 2. Wire the new module into `src/specs/      |    `gates.yaml` references as a     |
|    mod.rs` re-exports so downstream call     |    distinct `EdgeKind` variant,     |
|    sites (`resmate graph`, `resmate          |    using the Sibling A helpers.     |
|    explain`) can adopt it incrementally.     | 2. Add the optional `resmate        |
| 3. Update `mcp/tools.rs`'s `workflow_        |    doctor` info-level hint (§6.5)   |
|    validate` MCP tool description to         |    surfacing when a workflow only   |
|    mention DAG authoring is supported,        |    uses legacy `next`/`back_to`,    |
|    cross-referencing spec-workflow.md §2.4.   |    pointing at the new docs.        |
+-----------------------------------------------------------------------------------+
```
