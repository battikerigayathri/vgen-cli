# ResMate Use Case Workspace

Welcome to your ResMate Use Case Workspace! This workspace has been initialized using `resmate init`.

## 🚀 Getting Started

The developer and agent knowledge base for this workspace is fully consolidated under the Cursor Agent Skill directory to keep the workspace root clean and prevent context window exhaustion.

### ➡️ [Open the Master Router & Index (.cursor/skills/resmate-use-case/SKILL.md)](.cursor/skills/resmate-use-case/SKILL.md)

Please open the main skill file above, which serves as the entry point and decision guide for both human developers and AI coding agents.

## 📁 Directory Structure

*   `tools/` — Place your live tool folders here (containing `tool.yaml` and handler scripts).
*   `agents/` — Place your live agent YAML files here (e.g., `agents/my-agent.yaml`).
*   `assistants/` — Place your live assistant YAML files here (e.g., `assistants/my-assistant.yaml`).
*   `hitl/` — Place your live human-in-the-loop configuration folders here.
*   `workflows/` — Place your live workflow folders here (containing `flow.yaml`, `schema.yaml`, `playbooks.yaml`).
*   `examples/` — Reference-only blueprints and annotated samples. Do not develop here.
*   `.cursor/` — Contains Cursor rules and skills for AI-assisted development.

## 🛠️ CLI Commands Quick Reference

Run all commands from this workspace root:

```bash
resmate doctor                    # Diagnose environment and API connectivity
resmate graph                     # Visualize assistant and agent routing topology
resmate validate                  # Validate all local YAML and handler schemas
resmate push-all --dry-run        # Show local vs. remote field delta before pushing
resmate push-all --yes            # Batch push all resources to the platform
```
