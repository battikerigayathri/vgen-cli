# vgen.yaml manifest

Optional workspace manifest at the repository root.

## Precedence

**Environment variables (`VGEN_*_DIR`) override manifest paths.** Manifest overrides default directory names.

## Schema (version 1)

```yaml
version: 1
name: oracle-pr-agent-v2
description: Oracle purchase requisition use case
kit_version: "1.1.0"
initialized_at: "2026-07-17T09:38:00+00:00"

dirs:
  tools: tools
  agents: agents
  assistants: assistants
  hitl: hitl
  workflows: workflows

default_assistant: oracle-pr-assistant
recipe: oracle-pr

platform:
  base_url: https://api-dev.ai.resmed.com
```

| Field | Required | Purpose |
|-------|----------|---------|
| `version` | Yes | Manifest schema version (`1`) |
| `name` | Yes | Project slug for display / MCP |
| `description` | No | Free-text summary; written by `init` (default `ResMate use case workspace`, override with `--description`) |
| `kit_version` | No | Authoring kit version recorded at `init` time |
| `initialized_at` | No | RFC3339 timestamp written by `init` (e.g. `2026-07-17T09:38:00+00:00`) |
| `dirs.*` | No | Artifact directory paths (relative or absolute) |
| `default_assistant` | No | Convenience for `graph` / `push-all` |
| `recipe` | No | Records which scaffold recipe was used |
| `platform.base_url` | No | Documentation only; auth via env |

## Discovery

`workspace info` reports `manifest.present` and `manifest.name` when `vgen.yaml` exists. A directory with only `vgen.yaml` (no artifacts yet) is treated as a workspace root after `vgen init`.

## Commands

```bash
vgen init --name my-project --description "My use case"
vgen scaffold oracle-pr --name oracle-pr
vgen --json workspace info | jq '.data.manifest'
```

`init` targets an **empty or allowlisted** directory (allowlist: `.git`, `.gitignore`, `README.md`, `.DS_Store`); use `--force` to bootstrap-overwrite kit/seed files in a non-init-safe directory (never deletes live artifacts).
