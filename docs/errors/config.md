# Config & connectivity error codes

| Code | Exit | Severity | Description | Remediation |
|------|------|----------|-------------|-------------|
| `CONFIG_LOAD_FAILED` | 1 | error | Failed to read or parse config file | Check `VGEN_CONFIG` path or `~/.vgen/config.yaml` for valid YAML |
| `CONFIG_MISSING_API_KEY` | 1 | error | `VGEN_API_KEY` not set or empty | Export `VGEN_API_KEY` or add `api_key` to config file |
| `CONNECTIVITY_FAILED` | 1 | error | API unreachable, unauthorized, or non-success HTTP status | Run `vgen doctor`; verify `VGEN_BASE_URL` and API key |

## Workspace

| Code | Exit | Severity | Description | Remediation |
|------|------|----------|-------------|-------------|
| `WORKSPACE_NOT_FOUND` | 1 | error | No artifact directories detected from cwd | Run `vgen init` or `cd` to a workspace with tools/, agents/, etc. |
| `ARTIFACT_DIR_MISSING` | 1 | error | Expected artifact directory does not exist | Create the directory or set the matching `VGEN_*_DIR` env var |

## Graph

| Code | Exit | Severity | Description | Remediation |
|------|------|----------|-------------|-------------|
| `GRAPH_BUILD_FAILED` | 1 | error | Failed to parse artifacts while building link graph | Fix YAML/JSON parse errors in artifact files; run `vgen validate` |

## Usage

| Code | Exit | Severity | Description | Remediation |
|------|------|----------|-------------|-------------|
| `UNKNOWN_ERROR_CODE` | 2 | error | Requested error code not in registry | Run `vgen explain --list` to see available codes |
| `USAGE_CONFIRM_REQUIRED` | 2 | error | Mutating command requires `--yes` in JSON mode | Re-run with `--yes` or set `confirm: true` for MCP tools |
| `NOT_IMPLEMENTED` | 1 | error | Feature not yet available | Check CLI version and docs for supported commands |
