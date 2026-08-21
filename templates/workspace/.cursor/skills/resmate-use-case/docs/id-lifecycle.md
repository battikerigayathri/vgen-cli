# ID Lifecycle — Never Invent MongoDB ObjectIds

Every deployable ResMate resource (HITL form, workflow, tool, agent, assistant) is identified on the platform by a MongoDB ObjectId — a 24-character lowercase hex string, e.g. `6a354b8f4fffc3946d12c573`. This ID is **assigned by the platform**, not chosen locally.

This document is the single source of truth for how that ID is created, written back to your workspace, and reused. Read this before your first `push`.

---

## 1. The Lifecycle Flow

There are exactly four stages. Do not skip or reorder them.

1. **Local Creation** — Create `tool.yaml`, `agent.yaml`, `assistant.yaml`, or `hitl/*/meta.yaml` with the `id` field **empty** (`id: ""`) or **omitted entirely**. There is nothing else to fill in for the ID at this stage.
2. **Platform Registration** — Run the resource's push command, e.g.:
   ```bash
   vgen tool push <folder-name>
   vgen agent push <basename>
   vgen assistant push <basename>
   vgen hitl push <folder-name>
   vgen workflow push <folder-name>
   ```
   Because the local `id` is empty, the CLI knows this is a **create**, not an **update**, and asks the platform to register a brand-new resource.
3. **Local Write-Back** — The platform responds with a generated MongoDB ObjectId. The CLI writes that ID directly back into your local YAML file's `id` field, in place, preserving the rest of the file. You do not do this manually — the CLI does it as part of the `push` command.
4. **Subsequent Updates** — Every later push of that same resource now finds a non-empty `id`, so the CLI performs an **in-place update** against that exact platform record instead of registering a duplicate.

```text
   id: ""  ──push──>  platform assigns ObjectId  ──write-back──>  id: "6a354b8f4fffc3946d12c573"
 (local, new)                                                        (local, now update-tracked)
```

Once a resource has been pushed at least once, its `id` field is a **generated artifact** — treat it the same way you'd treat a compiled binary or a lockfile hash: read it, wire it into dependents, but never hand-author it.

For the order resources must be pushed in, and how their written-back IDs get wired into dependent resources, see [push-pull-wire.md](push-pull-wire.md).

---

## 2. Strict Anti-Patterns

Do **not** do any of the following:

- **Do not copy-paste ObjectIds** from `examples/`, other environments (dev/staging/prod), teammates' workspaces, or old chat transcripts. An ID that exists in one environment almost never exists in another.
- **Do not generate random 24-character hex strings** yourself (by hand, with a UUID-to-hex trick, or by asking an LLM to "make up an ID"). These will look plausible but do not correspond to any real platform record.
- **Do not invent IDs for skills or agents before they are pushed.** If `tools/my-tool/tool.yaml` still has `id: ""`, there is no valid ID to put into `agents/my-agent.yaml`'s `skills[]` array yet — push the tool first.
- **Do not hand-edit an already-written-back `id` field** to point at a different resource. If you need to re-target a binding, edit the *dependent's* binding array, not the target's own `id`.
- **Do not delete a written-back `id`** just to force a "clean" re-push unless you intend to create a brand-new duplicate resource on the platform. If you only want to update fields, keep the `id` and push again.

## 3. Why It Matters

Invented or copy-pasted IDs are a top cause of failed deployments:

- **`404 Not Found` on push**: The CLI tries to update a resource at an ID the platform has never seen (invented or from a different environment). There is nothing there to update.
- **`403 Forbidden` on push**: The ID happens to collide with an existing platform resource you don't own or aren't scoped to, and the platform correctly refuses the write.
- **Silent misrouting**: An agent's `skills[]` array pointing at the wrong (but valid) tool ID will bind to someone else's tool instead of yours, producing confusing runtime behavior with no obvious error.
- **Broken `vgen graph` / `vgen validate --remote`**: Both commands cross-reference local IDs against the platform. Invented IDs show up as `broken_ref` edges or remote-drift warnings, blocking a clean push-all plan.

**The fix is always the same**: leave the `id` empty, push, and let the CLI write it back. See [push-pull-wire.md](push-pull-wire.md) for the full push-and-wire loop across HITL, Workflow, Tool, Agent, and Assistant layers.
