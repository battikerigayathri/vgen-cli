# Phase 3 (P2) — ResMate CLI agent-authoring hardening

**Status:** Split into Phase 3A (authoring kit) + Phase 3B (validation hardening)  
**Target repo:** [`resmed_resmate-cli`](../.)  
**Jira epic:** [CGA-1094](https://resmedglobal.atlassian.net/browse/CGA-1094)  
**Prerequisite:** [PHASE2-P1-COMPLETE.md](./PHASE2-P1-COMPLETE.md)

---

## Sub-plans

| Phase | Focus | Plan | SP (approx.) |
|-------|--------|------|--------------|
| **3A** | Authoring kit, `resmate init`, scaffold, CLAD guide, core redirect stub | [PHASE3A-P2-PLAN.md](./PHASE3A-P2-PLAN.md) | ~13 |
| **3B** | Smriti alignment, handler lint, remote validate, diff, local smoke | [PHASE3B-P2-PLAN.md](./PHASE3B-P2-PLAN.md) | ~27 |

**Architecture:** [AUTHORING-KIT-ARCHITECTURE.md](./AUTHORING-KIT-ARCHITECTURE.md)

---

## Epic outcome (3A + 3B)

IDE agents can `init` → `scaffold` → edit → `validate` → `validate --remote` → `diff` → `push-all --dry-run` → `push-all --yes` without manual `cli-context` copy or shell text parsing.

---

## PR index (all phases)

| PR | Phase | Jira | Title |
|----|-------|------|-------|
| PR12 | 3A | CGA-1102 ext. | Authoring kit + init bootstrap |
| PR19 | 3A | CGA-1110 | CLAD / agent authoring guide |
| PR20-partial | 3A | CGA-1106 | Core `cli-context/` redirect stub |
| PR14 | 3B | CGA-1109 | Workflow smriti alignment |
| PR15 | 3B | CGA-1112 | Handler static analysis |
| PR16 | 3B | CGA-1107 | Remote ID checks |
| PR17 | 3B | CGA-1108 | `resmate diff` |
| PR18 | 3B | CGA-1111 | Local smoke + testdata (no GHA) |
| PR20 remainder | 3B | CGA-1106 | Kit doc sync for P2 commands |

**Total:** ~40 SP · GitHub Actions deferred to **Phase 4**.

---

## Implementation order

1. **3A:** PR12 → PR19 → PR20-partial  
2. **3B:** PR14 ‖ PR15 → PR16 → PR17 → PR18 → PR20 remainder

See sub-plans for acceptance criteria, file lists, and smoke scripts.
