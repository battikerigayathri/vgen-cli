# Phase 3A (P2) — Authoring kit & developer experience

**Status:** In progress (PR12 core)  
**Target repo:** [`resmed_vgen-cli`](../.)  
**Architecture:** [AUTHORING-KIT-ARCHITECTURE.md](./AUTHORING-KIT-ARCHITECTURE.md)  
**Jira epic:** [CGA-1094](https://resmedglobal.atlassian.net/browse/CGA-1094)  
**Prerequisite:** [PHASE2-P1-COMPLETE.md](./PHASE2-P1-COMPLETE.md) (CGA-1101–CGA-1106)

**Related:** [PHASE3B-P2-PLAN.md](./PHASE3B-P2-PLAN.md) (validation hardening, deferred) · [PHASE3-P2-PLAN.md](./PHASE3-P2-PLAN.md) (index)

---

## 1. Goals & success criteria

### What Phase 3A delivers

IDE agents can **bootstrap a full authoring workspace with one command** and scaffold starter recipes — without manually copying `cli-context` from the platform monorepo.

| # | Capability | Command(s) / artifact | Jira |
|---|------------|----------------------|------|
| 1 | **Authoring kit in CLI** | `templates/workspace/` vendored from core `cli-context/` | CGA-1102 ext. |
| 2 | **Init bootstrap** | `vgen init` copies kit + seed files | CGA-1102 ext. |
| 3 | **Recipe scaffold** | `vgen scaffold <recipe>` from `templates/recipes/` | CGA-1102 ext. |
| 4 | **CLAD replacement** | `docs/agent-authoring-guide.md` | [CGA-1110](https://resmedglobal.atlassian.net/browse/CGA-1110) |
| 5 | **Core redirect (partial)** | Core `cli-context/` README stub | CGA-1106 cont. |

### Success criteria

```bash
tmpdir=$(mktemp -d) && cd "$tmpdir"
vgen init --name smoke-test --json | jq '.ok == true'
test -f AGENTS.md && test -f .cursor/skills/vgen-use-case/SKILL.md
test -f platform/execution-model.md && test -f cli/authoring-checklist.md
vgen init --no-examples --force  # skips examples/
vgen scaffold oracle-pr --name smoke-test --json | jq '.ok == true'
cargo test init_authoring_kit
```

**Agent ergonomics:** `init` → `scaffold` → edit → (3B validate/diff/push gates).

---

## 2. PR breakdown (Phase 3A only)

| PR | Jira | Title | SP | Depends on |
|----|------|-------|-----|------------|
| **PR12** | CGA-1102 ext. | Authoring kit + init bootstrap + recipe templates | 8 | P1 PR9 |
| **PR19** | [CGA-1110](https://resmedglobal.atlassian.net/browse/CGA-1110) | CLAD / agent authoring guide | 3 | PR12 |
| **PR20-partial** | CGA-1106 cont. | Core `cli-context/` redirect stub | 2 | PR12 |

**Phase 3A total:** ~13 SP · ~10–12 person-days

---

## 3. PR12 — Authoring kit + init bootstrap (detailed)

### Scope

1. Vendored kit at `templates/workspace/` from `resmedai-core-framework/cli-context/`
2. Kit loader (`src/kit/`) — filesystem via `CARGO_MANIFEST_DIR` / `VGEN_TEMPLATES_DIR`
3. `vgen init` — full kit copy, seed files, empty live dirs
4. `vgen scaffold` — recipes from `templates/recipes/`; requires kit or `--with-kit`
5. Maintainer sync script `scripts/sync-authoring-kit.sh`

### Template layout

```
templates/
├── workspace/          # SoT: AGENTS.md, .cursor/, platform/, cli/, examples/
├── recipes/
│   ├── minimal/
│   ├── form-wizard/
│   └── oracle-pr/
└── seed/
    ├── vgen.yaml.tmpl
    └── gitignore
```

### Template resolution (v1)

| Context | Resolution |
|---------|------------|
| Dev / `cargo test` | `CARGO_MANIFEST_DIR/templates/` |
| Override | `VGEN_TEMPLATES_DIR` |
| `cargo install` | Set `VGEN_TEMPLATES_DIR` **or** follow-up `rust-embed` PR |

**Decision (PR12):** Filesystem-only for v1. `rust-embed` deferred — document in `src/kit/load.rs` and [AUTHORING-KIT-ARCHITECTURE.md](./AUTHORING-KIT-ARCHITECTURE.md) §8.

### `vgen init` flags

| Flag | Behavior |
|------|----------|
| `--name` | `vgen.yaml` `name` field (default: cwd basename) |
| `--force` | Overwrite kit markdown; never delete live artifacts |
| `--no-examples` | Skip `examples/` tree |
| `--json` | JSON envelope (global) |

### Copy semantics

- Recursive copy from `templates/workspace/` preserving paths
- `--force` overwrites kit files only; skips live `tools/<name>/`, `agents/*.yaml`, etc.
- Writes `vgen.yaml` with `kit_version`, `initialized_at`
- Creates empty `tools/`, `agents/`, `assistants/`, `hitl/`, `workflows/`

### Files

| Action | Path |
|--------|------|
| Add | `templates/workspace/**` |
| Add | `templates/recipes/{minimal,form-wizard,oracle-pr}/**` |
| Add | `templates/seed/*` |
| Add | `src/kit/{mod,load,copy}.rs` |
| Modify | `src/commands/init.rs`, `src/commands/scaffold.rs`, `src/scaffold.rs` |
| Modify | `src/cli/mod.rs`, `src/mcp/dispatch.rs` |
| Add | `scripts/sync-authoring-kit.sh` |
| Add | `tests/init_authoring_kit_test.rs` |

### Acceptance criteria

- [x] `vgen init` produces `AGENTS.md`, `.cursor/skills/`, `platform/`, `cli/`
- [x] `vgen init --no-examples` skips `examples/`
- [x] `vgen scaffold oracle-pr` after init writes recipe artifacts
- [x] `scaffold` without kit fails with `KIT_NOT_PRESENT` (or `--with-kit`)
- [x] `vgen.yaml` includes `kit_version`
- [ ] Kit docs fully updated (`STANDALONE.md`, skill step 1 → `vgen init`) — partial in PR12
- [x] `cargo test` passes

### Tests

| Test | Validates |
|------|-----------|
| `init_creates_authoring_kit` | AGENTS.md, skill, platform, cli checklist |
| `init_no_examples_skips_examples_tree` | `--no-examples` |
| `scaffold_oracle_pr_after_init` | Recipe files on disk |
| `scaffold_without_kit_requires_with_kit` | `KIT_NOT_PRESENT` |

---

## 4. PR19 — CLAD documentation replacement

**Jira:** [CGA-1110](https://resmedglobal.atlassian.net/browse/CGA-1110) · **SP:** 3 · **Depends on:** PR12

### Scope

- Add `docs/agent-authoring-guide.md` — canonical short guide in CLI repo
- Stub `docs/CLAD-vgen-use-cases.md` with redirect
- Update `README.md` — `vgen init` quick start
- Link from `templates/workspace/README.md`

### Acceptance criteria

- [ ] Guide covers: `init` → scaffold → validate → push-all, `--json`, MCP
- [ ] Platform detail links to workspace kit (post-init), not duplicated
- [ ] Old CLAD marked deprecated
- [ ] COE Gen AI stakeholder review checkbox

---

## 5. PR20-partial — Core redirect stub

**Jira:** CGA-1106 cont. · **SP:** 2 · **Depends on:** PR12

### Scope (3A only — not full kit doc sync)

| Repo | Action |
|------|--------|
| `resmedai-core-framework` | `cli-context/README.md` redirect to CLI `templates/workspace/` + `vgen init` |
| `resmedai-core-framework` | Add `cli-context/MIGRATION.md` |
| `resmedai-core-framework` | Update `AGENTS.md`, `context/links.md` pointers |

**Deferred to PR20 remainder (3B plan):** Full `templates/workspace/cli/` P2 command sync after validate/diff land.

---

## 6. Timeline

| PR | Story | Person-days | Parallelizable |
|----|-------|-------------|----------------|
| PR12 authoring kit | CGA-1102 ext. | 5–7 | **Start first** |
| PR19 CLAD guide | CGA-1110 | 2 | After PR12 |
| PR20-partial redirect | CGA-1106 | 1–2 | After PR12 |

**Story points:** ~13 SP  
**Calendar (1 developer):** ~2–2.5 weeks after P1 merge

---

## 7. Non-goals (Phase 3A)

| Item | Rationale |
|------|-----------|
| Workflow smriti alignment | Phase 3B PR14 |
| Remote validate / diff | Phase 3B |
| Local smoke + testdata | Phase 3B PR18 |
| GitHub Actions CI | Phase 4 |
| `rust-embed` for `cargo install` | Follow-up after PR12 if needed |
| Deleting core `cli-context/` | Redirect only |

---

## 8. Risks

| Risk | Mitigation |
|------|------------|
| Kit embed / install UX | Document `VGEN_TEMPLATES_DIR`; add `rust-embed` follow-up |
| Kit drift vs `pr-agent-v2` | `sync-authoring-kit.sh`; PR20 remainder |
| `scaffold` without init | `KIT_NOT_PRESENT` + `--with-kit` |
