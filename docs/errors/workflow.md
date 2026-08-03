# Workflow validation codes

| Code | Exit | Severity | Description | Remediation |
|------|------|----------|-------------|-------------|
| `WORKFLOW_VALIDATION` | — | error | Workflow failed schema or semantic validation | Run `resmate workflow validate <name> --json` for details |
| `WORKFLOW_SCHEMA_INVALID` | — | error | Workflow schema validation failed | Fix `schema.yaml` and stage fields per workflow authoring docs |
| `WORKFLOW_SEMANTIC_INVALID` | — | error | Workflow semantic validation failed | Fix stage `next` references, `doneWhen`, and terminal stages |
| `WORKFLOW_LAYOUT_INVALID` | — | error | Workflow layout validation failed | Fix layout/position fields in workflow definition |
