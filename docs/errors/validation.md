# Validation finding codes

Finding codes appear in `vgen validate` reports. Top-level envelope uses `VALIDATION_FAILED` (exit 3) when any error-severity finding exists.

| Code | Exit | Severity | Description | Remediation |
|------|------|----------|-------------|-------------|
| `VALIDATION_FAILED` | 3 | error | One or more validation findings at error severity | Run `vgen validate --json` and fix each error finding |
| `BROKEN_AGENT_REF` | — | error | Assistant references an agent id not found locally | Add the agent YAML or fix `agents[]` in the assistant file |
| `BROKEN_TOOL_REF` | — | error | Agent references a tool id not found locally | Add the tool folder or fix `skills[]` in the agent file |
| `ORPHAN_AGENT` | — | warning | Agent file not reachable from any assistant | Wire agent into an assistant or remove unused agent |
| `ORPHAN_TOOL` | — | warning | Tool folder not reachable from any agent | Wire tool into an agent or remove unused tool |
| `DUPLICATE_ID` | — | error | Same platform id used by multiple artifacts | Ensure each artifact has a unique `id` |
| `WORKFLOW_SLUG_MISSING` | — | error | Assistant systemContext missing workflow slug | Add `workflow_definition_slug` to systemContext |
| `WORKFLOW_SLUG_MISMATCH` | — | error | Workflow slug in assistant does not match local folder | Align `meta.slug` with systemContext slug |
| `WORKFLOW_FOLDER_ORPHAN` | — | warning | Workflow folder not referenced by any assistant | Reference workflow in assistant or remove folder |
| `HITL_SLUG_UNRESOLVED` | — | error | Workflow stage hitlSlug does not match local HITL | Add `hitl/<slug>/` with matching `meta.yaml` slug |
| `WORKFLOW_AGENT_UNRESOLVED` | — | error | Workflow stage agentSlug does not match local agent | Fix agent slug in workflow stage or agent YAML |
| `MISSING_REQUIRED_FIELD` | — | error | Required YAML/JSON field is missing | Fill required fields per layer reference docs |
| `MISSING_HANDLER` | — | error | FaaS/JS tool missing handler file | Add `handler.js` (or configured handler) to tool folder |
| `MISSING_HITL_CONFIG` | — | error | HITL folder missing config.json | Add `config.json` adaptive card to HITL folder |
| `FAAS_MISSING_PACKAGE` | — | error | FaaS tool missing package.json | Add `package.json` to FaaS tool folder |
| `JS_UNEXPECTED_PACKAGE` | — | warning | JS tool has package.json but type is not FaaS | Remove package.json or change tool type |
| `SECRET_SUSPECTED` | — | warning | Possible secret in artifact file | Move secrets to env vars; never commit API keys |
| `MISSING_ID` | — | info | Artifact has no platform id (create on push) | Push artifact or add id after first create |
| `MISSING_ID_BLOCKING` | — | warning | Missing id blocks push-readiness for referenced resource | Push dependency first or add id manually |
| `MISSING_WORKFLOW_PATCH` | — | warning | Save-tool handler likely missing workflowPatch return | Add workflowPatch to handler per [js-handlers.md](../tools/js-handlers.md) |
| `FAAS_HANDLER_SHAPE` | — | warning | FAAS handler missing expected export shape | Export `handler` async function per FaaS conventions |
| `JS_HANDLER_SHAPE` | — | warning | JS handler uses module.exports without JSON.stringify | Return JSON.stringify({ success, workflowPatch?, ... }) |
| `TOOL_FEEDS_INVALID` | — | error | Tool feeds block contains invalid schema, keys, empty fields, or duplicate entries | Check `feeds` or `output.x-feeds` block in `tool.yaml` |
| `ASSISTANT_TOOL_FEEDS_INVALID` | — | error | Assistant tool_feeds override block contains invalid consumer keys, empty fields, or duplicate entries | Check `tool_feeds` block in assistant YAML |
| `TOOL_FEEDS_MISSING` | — | warning | Tool is missing both feeds and output.x-feeds | Add `feeds` or `output.x-feeds` to tool.yaml to document output boundary |
| `REMOTE_ID_NOT_FOUND` | — | error | Local artifact id not found on platform (`validate --remote`) | Push artifact or fix stale id; run `vgen diff` |
| `REMOTE_LOOKUP_FAILED` | — | warning | Remote ID check failed (auth/network) | Verify VGEN_API_KEY; retry without `--offline` |
| `REMOTE_SKIPPED_OFFLINE` | — | info | `--remote` skipped because `--offline` was set | Remove `--offline` to run remote checks |

