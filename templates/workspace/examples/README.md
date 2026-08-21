# Examples (reference only)

**Do not develop here.** These are annotated reference samples inside the KB.

Develop live artifacts at **workspace root** — the same level as this `examples/` folder:

```
my-workspace/
├── tools/          ← your live tools
├── agents/
├── assistants/
├── hitl/           ← optional
└── examples/       ← this folder (reference only)
```

Copy layout and patterns from the samples below into workspace-root `tools/`, `agents/`, etc. See [cli/setup.md](../cli/setup.md) and [cli/authoring-checklist.md](../cli/authoring-checklist.md).

## Reference samples

| Sample | Path | Demonstrates |
|--------|------|--------------|
| Minimal greet | [minimal/](minimal/) | Smallest JS tool + agent + assistant |
| Jira read flow | [jira/](jira/) | HITL two-step, JS HITLConfig + FAAS read |
| Purchase requisition | [purchase-requisition/](purchase-requisition/) | Split YAML workflow form wizard + review + `workflowPatch.inputs` |
| GitHub PR create | [github-pr-create/](github-pr-create/) | Bundle JSON workflow + `agent_task` + `workflowPatch.artifacts` |
| Oracle purchase requisition | [oracle-purchase-requisition/](oracle-purchase-requisition/) | Search → dynamic HITL → `workflowPatch.inputs` |

## Read order

1. [minimal/README.md](minimal/README.md) — structure overview
2. [jira/walkthrough.md](jira/walkthrough.md) — annotated HITL + FAAS read flow
3. [purchase-requisition/walkthrough.md](purchase-requisition/walkthrough.md) — workflow form wizard (split YAML)
4. [github-pr-create/walkthrough.md](github-pr-create/walkthrough.md) — workflow agent_task (bundle JSON)
5. [oracle-purchase-requisition/walkthrough.md](oracle-purchase-requisition/walkthrough.md) — search → dynamic HITL → workflow save
6. [jira/README.md](jira/README.md) — optional smoke-test of Jira reference sample

## Try a reference sample (optional)

To push the bundled Jira or minimal sample as-is:

```bash
cd examples/jira    # from workspace root
vgen config validate
```

For day-to-day work, author under workspace-root `tools/`, `agents/`, etc.
