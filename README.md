# resmate-cli

ResMate platform CLI.

## Installing v1.0 (standalone)

Download a release archive from the project releases page (or build locally with `./scripts/package-release.sh`).

| Platform | Archive | Install |
|----------|---------|---------|
| macOS / Linux | `resmate-*-<platform>-<arch>.tar.gz` | `./install.sh` |
| Windows | `resmate-*-windows-x64.zip` | `powershell -ExecutionPolicy Bypass -File .\install.ps1` |

Default user-local install puts binaries in `~/.local/bin` (Unix) or `%LOCALAPPDATA%\Programs\ResMate\bin` (Windows) and templates in the co-installed `share/resmate/templates/` tree. See [docs/README-INSTALL.md](docs/README-INSTALL.md) and [docs/QUICKSTART.md](docs/QUICKSTART.md).

Maintainer packaging (requires sibling `resmedai-core-framework`):

```bash
RESMATE_CORE_FRAMEWORK=../resmedai-core-framework ./scripts/package-release.sh
```

## Prerequisites / repo layout (from source)

Check out this repo **beside** `resmedai-core-framework` under the same parent directory:

```
Workspace/ResMed/
├── resmedai-core-framework/
│   └── lib/smriti_client/
└── resmed_resmate-cli/
```

`Cargo.toml` depends on the sibling path:

```toml
smriti_client = { path = "../resmedai-core-framework/lib/smriti_client" }
```

That crate is used **only** by `resmate workflow validate` for local workflow schema and semantic validation. That command does not make runtime calls to Prajna or Smriti APIs.

## Building

From `resmed_resmate-cli`:

```bash
cargo build
cargo test
```

To put `resmate` on your PATH:

```bash
cargo install --path .
```

Run the smoke commands below from a use-case **workspace root** (e.g. `pr-agent-v2`).

## P0 commands quick reference

Run from a use-case **workspace root** (e.g. `pr-agent-v2`). All commands support global `--json` for machine-readable output.

| Command | Purpose |
|---------|---------|
| `resmate workspace info` | Workspace root, artifact dirs, counts, config summary |
| `resmate doctor [--offline]` | Environment and connectivity diagnostics |
| `resmate graph [--assistant <name>]` | Dependency link graph (assistants → agents → tools → HITL/workflows) |
| `resmate validate [--strict]` | Aggregate validation (graph, artifacts, workflows, secrets) |
| `resmate workflow validate <folder>` | Local workflow schema + semantics (no API) |
| `resmate push-all --dry-run [--assistant <name>]` | Preview push plan in dependency order |

**Pre-push loop:** `doctor` → `graph` → `workflow validate` (if applicable) → `validate` → `push-all --dry-run` → individual `push` commands.

**Smoke test** (against `pr-agent-v2`):

```bash
resmate --json workspace info | jq '.ok == true'
resmate --json doctor | jq '.data.checks | length >= 3'
resmate --json graph | jq '[.data.edges[] | select(.kind=="broken_ref")] | length'
resmate --json workflow validate oracle-purchase-requisition | jq '.ok == true'
resmate --json validate | jq '.data.summary.error_count == 0'
resmate --json push-all --dry-run | jq '.data.steps | map(.resource_type)'
```

See [docs/PHASE1-P0-COMPLETE.md](docs/PHASE1-P0-COMPLETE.md), [docs/PHASE1-P0-PLAN.md](docs/PHASE1-P0-PLAN.md), and Jira epic [CGA-1094](https://resmedglobal.atlassian.net/browse/CGA-1094).

## P1 commands quick reference

Run from a use-case **workspace root** (e.g. `pr-agent-v2`). All commands support global `--json` for machine-readable output.

| Command | Purpose |
|---------|---------|
| `resmate explain <code>` | Explain a stable error code with remediation hints |
| `resmate explain --list` | List all known error codes |
| `resmate init` | Bootstrap workspace layout and `resmate.yaml` |
| `resmate scaffold <recipe>` | Apply a recipe template (e.g. `oracle-pr`) |
| `resmate push-all --yes` | Execute batch push in dependency order (hitl → workflow → tool → agent → assistant) |
| `resmate sync` | Pull remote assistant graph including HITL configs |
| `resmate-mcp` | Stdio MCP server wrapping inspect, validate, plan, and push tools |

**Agent loop:** `init` → edit artifacts → `validate` → `push-all --dry-run` → `push-all --yes`, or call the same flow via MCP tools.

**Smoke test** (against `pr-agent-v2`):

```bash
resmate explain BROKEN_AGENT_REF | grep -qi remediation
resmate --json workspace info | jq '.data.counts.hitl >= 1'
resmate --json push-all --dry-run | jq '.data.steps | map(.resource_type)'
resmate --json validate | jq '.data.summary.error_count == 0'
```

See [docs/PHASE2-P1-COMPLETE.md](docs/PHASE2-P1-COMPLETE.md), [docs/PHASE2-P1-PLAN.md](docs/PHASE2-P1-PLAN.md), [docs/mcp-setup.md](docs/mcp-setup.md), and Jira stories [CGA-1101](https://resmedglobal.atlassian.net/browse/CGA-1101)–[CGA-1106](https://resmedglobal.atlassian.net/browse/CGA-1106).

## Use-case authoring (quick start)

Bootstrap a full authoring workspace with one command — no manual `cli-context` copy:

```bash
mkdir my-assistant && cd my-assistant
resmate init --name my-assistant
resmate scaffold oracle-pr --name my-assistant   # optional recipe
resmate doctor && resmate validate && resmate push-all --dry-run
resmate push-all --yes
```

| Resource | Location |
|----------|----------|
| **Author guide** | [docs/agent-authoring-guide.md](docs/agent-authoring-guide.md) |
| **Kit (copied on init)** | [templates/workspace/](templates/workspace/) — `AGENTS.md`, `platform/`, `cli/`, `examples/` |
| **MCP setup** | [docs/mcp-setup.md](docs/mcp-setup.md) |
| **Phase 3A plan** | [docs/PHASE3A-P2-PLAN.md](docs/PHASE3A-P2-PLAN.md) |
| **Phase 3B complete** | [docs/PHASE3B-P2-COMPLETE.md](docs/PHASE3B-P2-COMPLETE.md) |

Legacy [docs/CLAD-resmate-use-cases.md](docs/CLAD-resmate-use-cases.md) is deprecated. Platform monorepo `cli-context/` redirects here — see [MIGRATION.md](../resmedai-core-framework/cli-context/MIGRATION.md).

## P3B commands quick reference

Run from a use-case **workspace root** (e.g. `pr-agent-v2`). Requires API credentials for remote checks.

| Command | Purpose |
|---------|---------|
| `resmate validate --remote` | Local validation plus remote drift rules (cached; skips offline with `REMOTE_SKIPPED_OFFLINE`) |
| `resmate diff <type> <name>` | Compare local artifact vs remote (`tool`, `agent`, `assistant`, `hitl`, `workflow`) |

**Pre-push loop (P3B):** `doctor` → `graph` → `workflow validate` → `validate` → `validate --remote` (optional) → `diff <type> <name>` (optional) → `push-all --dry-run`.

**Integration smoke** (from `resmed_resmate-cli`, against `pr-agent-v2`):

```bash
./scripts/smoke-pr-agent.sh
```

See [docs/PHASE3B-P2-COMPLETE.md](docs/PHASE3B-P2-COMPLETE.md) and Jira stories [CGA-1107](https://resmedglobal.atlassian.net/browse/CGA-1107)–[CGA-1112](https://resmedglobal.atlassian.net/browse/CGA-1112).

## Future / known limitation

Standalone CI may later switch to a git-pinned `smriti_client` or a vendored validator crate. The sibling-repo path dependency is intentional for now so workflow validation tracks platform schema changes during P0 development.

## Building for Windows from macOS

You can cross-compile the CLI to Windows (x64 or x86) on a Mac.

### One-time setup

```bash
# Rust Windows targets (GNU/MinGW)
rustup target add x86_64-pc-windows-gnu i686-pc-windows-gnu

# MinGW linker and runtime
brew install mingw-w64
```

### Build

```bash
# 64-bit Windows
./scripts/build-windows.sh --arch x64

# 32-bit Windows
./scripts/build-windows.sh --arch x86

# Debug build (optional)
./scripts/build-windows.sh --arch x64 --debug
```

Output:

- **x64:** `target/x86_64-pc-windows-gnu/release/resmate.exe`, `resmate-mcp.exe`
- **x86:** `target/i686-pc-windows-gnu/release/resmate.exe`, `resmate-mcp.exe`

Test the `.exe` on a real Windows machine (or use [Wine](https://www.winehq.org/) for a quick smoke test on macOS).
