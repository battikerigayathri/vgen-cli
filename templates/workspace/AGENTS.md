# Agent Instructions & Knowledge Base Redirection

The ResMate developer and agent knowledge base has been restructured to prevent context window exhaustion and attention dilution.

All instructions, architectural decision guides, schemas, and CLI references have been consolidated under the Cursor Agent Skill directory.

## ⚠️ Critical Rules for Agents & Developers

- **Never Invent MongoDB ObjectIds**: Leave `id` fields empty or omitted. Let the CLI push and write-back assign them. Full detail: [.cursor/skills/resmate-use-case/docs/id-lifecycle.md](.cursor/skills/resmate-use-case/docs/id-lifecycle.md).
- **Follow Push Order**: HITL -> Workflow -> Tool -> Agent -> Assistant.
- **Wire After Push**: Copy written-back IDs into dependent arrays (`skills`, `agents`) and re-push. Full detail: [.cursor/skills/resmate-use-case/docs/push-pull-wire.md](.cursor/skills/resmate-use-case/docs/push-pull-wire.md).

## ➡️ [Go to the Master Router & Index (.cursor/skills/resmate-use-case/SKILL.md)](.cursor/skills/resmate-use-case/SKILL.md)

Please open and refer to the main skill file above, which acts as the entry point and decision guide for both human developers and AI coding agents.
