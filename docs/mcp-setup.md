# MCP server setup

ResMate ships a stdio MCP server (`resmate-mcp`) that wraps inspect, validate, plan, and push tools for IDE agents.

## Build

```bash
cargo build --release --bin resmate-mcp
```

Binary: `target/release/resmate-mcp`

## Cursor configuration

Add to `.cursor/mcp.json` (or global MCP settings):

```json
{
  "mcpServers": {
    "resmate": {
      "command": "/absolute/path/to/resmate-mcp",
      "args": [],
      "env": {
        "RESMATE_API_KEY": "${env:RESMATE_API_KEY}",
        "RESMATE_BASE_URL": "${env:RESMATE_BASE_URL}"
      }
    }
  }
}
```

Set `cwd` or pass `workspace_root` in tool arguments to target a use-case folder (e.g. `pr-agent-v2`).

## Tools

| Tool | CLI equivalent | Mutating | `confirm` required |
|------|----------------|----------|-------------------|
| `workspace_info` | `workspace info --json` | No | — |
| `doctor` | `doctor --json` | No | — |
| `graph` | `graph --json` | No | — |
| `validate` | `validate --json` | No | — |
| `workflow_validate` | `workflow validate <name> --json` | No | — |
| `push_all_dry_run` | `push-all --dry-run --json` | No | — |
| `push_all_execute` | `push-all --yes --json` | Yes | **Yes** |
| `explain` | `explain <code> --json` | No | — |
| `explain_list` | `explain --list --json` | No | — |
| `init_workspace` | `init --json` | Yes | **Yes** |
| `scaffold_recipe` | `scaffold <recipe> --json` | Yes | **Yes** |

Tool results return a **parsed JSON envelope** in `content[].text` (same shape as `resmate --json`).

## Smoke test

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | resmate-mcp | jq '.result.tools | length'
```

From a workspace root:

```bash
cd /path/to/pr-agent-v2
echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"validate","arguments":{}}}' | resmate-mcp | jq '.result.content[0].text | fromjson | .data.summary.error_count'
```

## Safety

- Mutating tools require `confirm: true` in arguments.
- API keys are inherited from the MCP process environment; never returned in tool output.
