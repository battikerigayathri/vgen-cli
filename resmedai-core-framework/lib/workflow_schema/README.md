# workflow_schema

Published JSON Schema for the **workflow definition bundle** shape (`meta`, `schema`, `flow`, optional `gates`).

- **Schema file:** [`workflow-definition.schema.json`](workflow-definition.schema.json)
- **Rust embed:** `WORKFLOW_DEFINITION_SCHEMA` in [`src/lib.rs`](src/lib.rs)

Used by `smriti_client` validation at push time and consumable by CI (`ajv`) or future ResMate CLI.
