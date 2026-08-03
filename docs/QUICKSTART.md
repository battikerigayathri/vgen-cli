# ResMate CLI Quick Start

Post-install flow for use-case authors. Assumes `resmate` and `resmate-mcp` are on `PATH` and `RESMATE_TEMPLATES_DIR` is set (install scripts do both).

## 1. Verify install

```bash
resmate --help
resmate doctor --offline   # from any directory; workspace checks need a project
```

On Windows (new PowerShell session):

```powershell
resmate --help
```

If the binary will not start after install, see **OS security / first-run prompts** in `README-INSTALL.md` (macOS quarantine, Windows `Unblock-File` / SmartScreen).

## 2. Create a workspace

```bash
mkdir ~/my-use-case && cd ~/my-use-case
resmate init --name my-use-case --description "My use case"
```

`init` targets an **empty or allowlisted** directory — only `.git`, `.gitignore`, `README.md`, and `.DS_Store` may already be present (safe to `git init` first). Any other file or directory makes init refuse with `INIT_REFUSED`, listing the offenders; pass `--force` to bootstrap-overwrite kit/seed files (never deletes live artifacts). `--description` is optional and defaults to `ResMate use case workspace`.

Expected output includes `AGENTS.md`, `.cursor/skills/resmate-use-case/SKILL.md`, `platform/`, `cli/`, empty live dirs (`tools/`, `agents/`, …), and `resmate.yaml` with `kit_version` and an RFC3339 `initialized_at`.

Optional recipe scaffold:

```bash
resmate scaffold oracle-pr --name my-use-case
```

Skip bundled examples:

```bash
resmate init --name my-use-case --no-examples
```

## 3. Configure API access

Create `.env` in the workspace root:

```bash
cat > .env <<'EOF'
RESMATE_API_KEY=<your-key>
# RESMATE_BASE_URL=https://...   # if non-default
EOF
```

## 4. Pre-push validation loop

Run from the workspace root:

```bash
resmate doctor
resmate graph
resmate workflow validate <workflow-folder>   # when applicable
resmate validate
resmate validate --remote                     # optional; needs API
resmate push-all --dry-run
resmate push-all --yes                        # after review
```

## 5. MCP (Cursor)

Point Cursor at the installed MCP binary. See `docs/mcp-setup-snippet.json` in this archive — replace `<RESMATE_MCP_PATH>` with your absolute path to `resmate-mcp` (or `resmate-mcp.exe` on Windows).

Smoke test:

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | resmate-mcp | head -c 200
```

## 6. Refresh authoring kit (optional)

To refresh kit-owned files (skills, rules, docs, `AGENTS.md`) in an existing workspace without touching live artifacts (`tools/`, `agents/`, `assistants/`, `hitl/`, `workflows/`) or user config, run from the workspace root:

```bash
resmate kit update              # skills/rules/docs/AGENTS.md; skips examples/
resmate kit update --examples   # also refresh examples/
resmate kit update --dry-run    # preview files that would change, no writes
resmate kit refresh             # alias for `kit update`
```

`kit update` also updates `kit_version` in `resmate.yaml` (preserving comments/formatting) unless `--dry-run` is set. It requires `resmate.yaml` to be present (fails with `NOT_A_WORKSPACE` otherwise) and is not blocked by the `init` emptiness gate.

To pick up newer CLI-bundled templates first (e.g. after upgrading the CLI), update `RESMATE_TEMPLATES_DIR` before running `kit update`:

```bash
cp -a /path/to/new/templates/* "$RESMATE_TEMPLATES_DIR/"
```

## Related docs

- Full install options: `README-INSTALL.md` (archive root)
- Author guide: shipped in workspace after `resmate init` → `docs/agent-authoring-guide.md` (via platform monorepo / release notes)
