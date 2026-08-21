# ResMate CLI Quick Start

Post-install flow for use-case authors. Assumes `vgen` and `vgen-mcp` are on `PATH` and `VGEN_TEMPLATES_DIR` is set (install scripts do both).

## 1. Verify install

```bash
vgen --help
vgen doctor --offline   # from any directory; workspace checks need a project
```

On Windows (new PowerShell session):

```powershell
vgen --help
```

If the binary will not start after install, see **OS security / first-run prompts** in `README-INSTALL.md` (macOS quarantine, Windows `Unblock-File` / SmartScreen).

## 2. Create a workspace

```bash
mkdir ~/my-use-case && cd ~/my-use-case
vgen init --name my-use-case --description "My use case"
```

`init` targets an **empty or allowlisted** directory — only `.git`, `.gitignore`, `README.md`, and `.DS_Store` may already be present (safe to `git init` first). Any other file or directory makes init refuse with `INIT_REFUSED`, listing the offenders; pass `--force` to bootstrap-overwrite kit/seed files (never deletes live artifacts). `--description` is optional and defaults to `ResMate use case workspace`.

Expected output includes `AGENTS.md`, `.cursor/skills/vgen-use-case/SKILL.md`, `platform/`, `cli/`, empty live dirs (`tools/`, `agents/`, …), and `vgen.yaml` with `kit_version` and an RFC3339 `initialized_at`.

Optional recipe scaffold:

```bash
vgen scaffold oracle-pr --name my-use-case
```

Skip bundled examples:

```bash
vgen init --name my-use-case --no-examples
```

## 3. Configure API access

Create `.env` in the workspace root:

```bash
cat > .env <<'EOF'
VGEN_API_KEY=<your-key>
# VGEN_BASE_URL=https://...   # if non-default
EOF
```

## 4. Pre-push validation loop

Run from the workspace root:

```bash
vgen doctor
vgen graph
vgen workflow validate <workflow-folder>   # when applicable
vgen validate
vgen validate --remote                     # optional; needs API
vgen push-all --dry-run
vgen push-all --yes                        # after review
```

## 5. MCP (Cursor)

Point Cursor at the installed MCP binary. See `docs/mcp-setup-snippet.json` in this archive — replace `<VGEN_MCP_PATH>` with your absolute path to `vgen-mcp` (or `vgen-mcp.exe` on Windows).

Smoke test:

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | vgen-mcp | head -c 200
```

## 6. Refresh authoring kit (optional)

To refresh kit-owned files (skills, rules, docs, `AGENTS.md`) in an existing workspace without touching live artifacts (`tools/`, `agents/`, `assistants/`, `hitl/`, `workflows/`) or user config, run from the workspace root:

```bash
vgen kit update              # skills/rules/docs/AGENTS.md; skips examples/
vgen kit update --examples   # also refresh examples/
vgen kit update --dry-run    # preview files that would change, no writes
vgen kit refresh             # alias for `kit update`
```

`kit update` also updates `kit_version` in `vgen.yaml` (preserving comments/formatting) unless `--dry-run` is set. It requires `vgen.yaml` to be present (fails with `NOT_A_WORKSPACE` otherwise) and is not blocked by the `init` emptiness gate.

To pick up newer CLI-bundled templates first (e.g. after upgrading the CLI), update `VGEN_TEMPLATES_DIR` before running `kit update`:

```bash
cp -a /path/to/new/templates/* "$VGEN_TEMPLATES_DIR/"
```

## Related docs

- Full install options: `README-INSTALL.md` (archive root)
- Author guide: shipped in workspace after `vgen init` → `docs/agent-authoring-guide.md` (via platform monorepo / release notes)
