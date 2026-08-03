# ResMate Environment Variables and Secrets Management

Developing ResMate use cases that integrate with external services (such as JIRA API, AWS S3, Oracle ERP, or Slack Webhooks) requires secure handling of API credentials and operational configurations. To prevent hardcoding credentials inside codebase configurations or JS handlers, the platform leverages secure environment mappings, dynamic agent-level interpolation, and a unidirectional write-only secret policy.

---

## 1. Local Environment Configuration

The `resmate` CLI utilizes local environment variables to resolve its active configuration and authenticate requests against platform APIs.

### `.env` File Usage
At startup, the CLI automatically loads variables from a `.env` file located at the **workspace root** (using the `dotenvy` library). Because this file contains sensitive local credentials, it must be explicitly added to the project's `.gitignore` file and never committed to source control:

```text
# .gitignore
.env
.resmate/
node_modules/
```

### CLI System Environment Variables
The CLI reads the following environment variables. If omitted, the CLI utilizes its standard default values:

| Variable Name | Purpose | Default Value (if omitted) |
| :--- | :--- | :--- |
| `RESMATE_BASE_URL` | Specifies the base API URL of the Tantra/Prajna platform. | `https://api-dev.ai.resmed.com` |
| `RESMATE_API_KEY` | Platform API key used for JWT signing and resource operations. | *Required for sync, push, or pull* |
| `RESMATE_SECRET` | Client secret used to sign secure JWT authentication headers. | *Required if utilizing JWT auth* |
| `RESMATE_ROC_SESSION` | Overrides the default session value for API handshakes. | *Omitted* |
| `RESMATE_CONFIG` | Overrides the global configuration path on the filesystem. | `~/.resmate/config.yaml` |
| `RESMATE_IDS_FILE` | Specifies the path to the local Slug-to-ID mapping store. | `~/.resmate/ids.yaml` |
| `RESMATE_TOOLS_DIR` | Absolute or relative path to the tools directory. | `tools/` under workspace root |
| `RESMATE_AGENTS_DIR` | Absolute or relative path to the agents directory. | `agents/` under workspace root |
| `RESMATE_ASSISTANTS_DIR` | Absolute or relative path to the assistants directory. | `assistants/` under workspace root |
| `RESMATE_HITL_DIR` | Absolute or relative path to the HITL forms directory. | `hitl/` under workspace root |
| `RESMATE_WORKFLOWS_DIR` | Absolute or relative path to the workflow definitions directory. | `workflows/` under workspace root |

---

## 2. Platform Secret Mapping: `resmate.yaml`

To bridge the gap between local environment values and remote orchestrator access, developers define mapping configurations inside the `env_mappings` array block of the workspace manifest (`resmate.yaml`).

### Schema Definition
The `env_mappings` block is defined as a list of YAML objects with the following schema fields:

*   **`local_key`**: (String, Required) The exact name of the environment variable defined in your local `.env` or system environment.
*   **`remote_key`**: (String, Required) The target secret key identifier that will be compiled and stored in Smriti's secure credentials vault. Can include the dynamic placeholder `${assigned_agent}`.
*   **`scope`**: (String, Required) Specifies the target orchestration layer. Typical values include `agents` or `assistants`.
*   **`agent_interpolation`**: (Boolean, Required) Controls whether the compiler duplicates and interpolates the mapping across all discovered active agents.

### Example `env_mappings` Configuration
```yaml
# resmate.yaml
version: 1
name: purchase-requisition-workspace
env_mappings:
  - local_key: LOCAL_JIRA_PASSWORD
    remote_key: JIRA_API_TOKEN
    scope: agents
    agent_interpolation: false
  - local_key: LOCAL_ROC_AUTH_TOKEN
    remote_key: ROC_AUTH_${assigned_agent}
    scope: agents
    agent_interpolation: true
  - local_key: LOCAL_AWS_SECRET
    remote_key: AWS_SECRET_KEY_${assigned_agent}
    scope: agents
    agent_interpolation: true
```

---

## 3. Dynamic Interpolation & Compiler Processing

To support multi-agent workspaces without repetitive copy-pasting, the CLI supports dynamic agent-level placeholder interpolation when compile-deploying secrets.

### Compiler Processing Steps
When `resmate env push` is executed, the compiler performs the following sequence:

1.  **Workspace Detection**: Locates the active workspace root and loads the local `.env` file via `dotenvy`.
2.  **Agent Discovery**: Scans the workspace's local `agents/` directory (resolving its path via `RESMATE_AGENTS_DIR` or defaulting to `agents/` relative to workspace root) for all active YAML or YML agent definitions. It extracts the file stems to compile a list of active agent slugs (e.g., `agents/oracle-pr-agent.yaml` resolves to slug `oracle-pr-agent`).
3.  **Key Compilation**: The compiler loops through the declared `env_mappings` block:
    *   If `agent_interpolation` is `true` and the `remote_key` contains the dynamic `${assigned_agent}` placeholder, the compiler duplicates the mapping once for every active agent slug discovered in Step 2.
    *   For each active agent slug, it replaces the `${assigned_agent}` placeholder with the corresponding active agent slug.
    *   If `agent_interpolation` is `false`, the compiler processes the mapping as a single global key-value pair.
4.  **Value Resolution**: Fetches the raw value for each mapping's `local_key` variable from the loaded environment.
    *   *Warning*: If a mapped variable is missing from the system environment, the CLI displays a warning but proceeds to compile the remaining defined mappings.

### Example Walkthrough
Given the following environment state:
*   Local `.env` contains: `LOCAL_ROC_AUTH_TOKEN="sk_resmate_9921"`
*   Workspace contains agents: `oracle-pr-agent.yaml` and `jira-sync-agent.yaml`
*   `resmate.yaml` contains:
    ```yaml
    env_mappings:
      - local_key: LOCAL_ROC_AUTH_TOKEN
        remote_key: ROC_AUTH_${assigned_agent}
        scope: agents
        agent_interpolation: true
    ```

The compiled target secret payload will generate two separate, agent-scoped credentials:
1.  **Secret Key**: `ROC_AUTH_oracle-pr-agent` | **Secret Value**: `"sk_resmate_9921"`
2.  **Secret Key**: `ROC_AUTH_jira-sync-agent` | **Secret Value**: `"sk_resmate_9921"`

### Payload Dispatching
Once compilation is complete, the CLI packages the credentials into a structured `SetSecretsRequest` compatible with Smriti's server-side API:

```json
{
  "serviceType": "aws_secrets_manager", 
  "description": "Deployment credentials for PR extraction agent",
  "secret": [
    {
      "key": "ROC_AUTH_oracle-pr-agent",
      "value": "sk_resmate_9921"
    },
    {
      "key": "ROC_AUTH_jira-sync-agent",
      "value": "sk_resmate_9921"
    }
  ]
}
```

---

## 4. Bulk Secret Deployment: `resmate env push`

Deploy local environment variables mapped in `resmate.yaml` to Smriti's credentials store.

```bash
resmate env push [--service-type <type>] [--description <desc>]
```

### Options
*   `--service-type`: Maps the target cloud secret manager (`aws_secrets_manager` or `awssecretsmanager` maps to platform `AwsSecretsManager`; other values default to platform `Local` key-value encryption).
*   `--description`: Custom description for the deployment credentials payload.

---

## 5. Strict Write-Only Security Policy

To protect operational integrity and prevent unauthorized credential exposure, the platform and CLI enforce a strict, unidirectional **Write-Only Secrets Policy**.

1.  **Platform Limitations**: The Smriti secrets controller only exposes write-based endpoints (`POST /set-secrets`) and delete operations. Smriti exposes **no GET endpoints** that return raw secret values back to API consumers. Once a secret is pushed, it cannot be read back.
2.  **No Decryption in CLI**: The `resmate` CLI strictly forbids retrieving, showing, or decrypting secrets stored on the platform. Commands like `resmate env get` or `resmate env show` do not exist.
3.  **Clean Terminal Output**: To prevent leaks during demonstrations or screen shares, the CLI terminal output after running a push command displays only success statuses, key names, and string lengths, completely concealing the raw values:

```text
🔒 Secret mapping compiled successfully.
Pushed 2 secrets to Smriti (Local Storage manager):
  - [Success] Key: ROC_AUTH_oracle-pr-agent (Length: 15)
  - [Success] Key: ROC_AUTH_jira-sync-agent (Length: 15)
Bulk deployment finalized. Values have been encrypted and stored.
```
