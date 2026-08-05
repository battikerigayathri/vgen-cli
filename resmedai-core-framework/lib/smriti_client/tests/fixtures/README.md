# Workflow test fixtures

**Single source of truth:** `cli-context/examples/*/workflows/`

These copies exist for `cargo test -p smriti_client` and `push_workflow_definition --validate-only` in CI. Re-sync after editing gold samples:

```bash
cp -r cli-context/examples/purchase-requisition/workflows/purchase-requisition/* \
  lib/smriti_client/tests/fixtures/purchase-requisition-split/

cp cli-context/examples/github-pr-create/workflows/github-pr-create/workflow.json \
  lib/smriti_client/tests/fixtures/github-pr-create-bundle/workflow.json
```
