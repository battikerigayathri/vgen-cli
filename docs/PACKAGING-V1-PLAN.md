# ResMate CLI v1.0 — Packaging & Release Plan

**Status:** Planning (not implemented)  
**Target version:** `1.0.0`  
**Repo:** [`resmed_vgen-cli`](../.)  
**Related:** [AUTHORING-KIT-ARCHITECTURE.md](./AUTHORING-KIT-ARCHITECTURE.md) · [PHASE3A-P2-PLAN.md](./PHASE3A-P2-PLAN.md) · [README.md](../README.md)

---

## 1. Goals & scope

### What v1.0 delivers

A **standalone developer package** that lets a ResMate use-case author:

1. Install `vgen` and `vgen-mcp` globally (on `PATH`) without Rust toolchain knowledge.
2. Run `vgen init`, `vgen scaffold`, and all P0–P3B CLI commands **out of the box**.
3. Bootstrap a full authoring workspace (kit copy from co-installed templates) on any supported OS.

### In scope (P0 — must ship)

| Item | Detail |
|------|--------|
| Version bump | `0.1.0` → `1.0.0` in `Cargo.toml`; `kit_version` follows via `env!("CARGO_PKG_VERSION")` |
| Release artifacts | Per-platform `.tar.gz` (macOS, Linux) and `.zip` (Windows; macOS alt) |
| Binaries | `vgen` + `vgen-mcp` for darwin-arm64, darwin-x64, linux-x64, windows-x64 |
| Templates | Co-installed under `share/vgen/templates/` (see §6) |
| Install scripts | `install.sh` (macOS/Linux), `install.ps1` (Windows) |
| Build script | `scripts/package-release.sh` — clones sibling core-framework, builds all targets, assembles archives |
| CLI lookup | Extend `src/kit/load.rs` with install-time default paths (see §8) |
| Docs | `CHANGELOG.md`, release quick-start in archive + README update |

### Out of scope (deferred)

| Item | Phase |
|------|-------|
| `rust-embed` templates in binary | P1 / v1.1 |
| Publish to crates.io / Homebrew / winget | P1+ |
| GitHub Actions release workflow | P1 (manual build OK for v1.0) |
| `smriti_client` as published crate (no path dep) | P2 |
| Linux arm64, Windows arm64, musl static builds | P2 |
| Code signing / notarization (macOS) | P2 |

### Key decision: co-installed templates (v1.0)

**Recommendation: co-installed templates via install script — not `rust-embed` for v1.0.**

| Approach | Pros | Cons |
|----------|------|------|
| **Co-installed (chosen)** | No rebuild when kit docs change; diffable template tree in release; matches existing `templates_root()` filesystem model; ~564 KB / 119 files is trivial beside binaries; install script can refresh templates independently | Requires install script + default path lookup; two on-disk locations (bin + share) |
| `rust-embed` (v1.1) | Single binary; works without share tree | Every kit doc fix requires recompile + redeploy; harder to inspect/update templates; adds compile-time complexity (`embed.rs` planned but not present) |

Rationale aligns with [AUTHORING-KIT-ARCHITECTURE.md](./AUTHORING-KIT-ARCHITECTURE.md) §8 and [PHASE3A-P2-PLAN.md](./PHASE3A-P2-PLAN.md): filesystem-first for v1; embed as follow-up when offline single-file distribution is a hard requirement.

---

## 2. Release artifact layout

Same logical contents in every archive; compression differs by platform convention.

### Directory tree (inside each archive)

```
vgen-1.0.0-<platform>-<arch>/
├── VERSION                          # "1.0.0"
├── LICENSE                          # if repo has one; else add
├── README-INSTALL.md                # platform-agnostic install + quick start
├── install.sh                       # macOS / Linux
├── install.ps1                      # Windows (PowerShell; primary on Windows)
├── bin/
│   ├── vgen                      # vgen.exe on Windows
│   └── vgen-mcp                  # vgen-mcp.exe on Windows
├── share/
│   └── vgen/
│       └── templates/
│           ├── workspace/           # authoring kit (copied on init)
│           ├── recipes/             # scaffold sources
│           └── seed/                # vgen.yaml.tmpl, gitignore
└── docs/
    ├── QUICKSTART.md                # post-install flow (§7)
    └── mcp-setup-snippet.json       # Cursor MCP template with placeholders
```

### Archive naming

| Platform | Arch | Format | Example filename |
|----------|------|--------|------------------|
| macOS | arm64 | `.tar.gz` | `vgen-1.0.0-darwin-arm64.tar.gz` |
| macOS | x64 | `.tar.gz` | `vgen-1.0.0-darwin-x64.tar.gz` |
| macOS | arm64/x64 | `.zip` (alt) | `vgen-1.0.0-darwin-arm64.zip` (optional P1) |
| Linux | x64 | `.tar.gz` | `vgen-1.0.0-linux-x64.tar.gz` |
| Windows | x64 | `.zip` | `vgen-1.0.0-windows-x64.zip` |

**P0 minimum:** four primary artifacts (darwin-arm64, darwin-x64, linux-x64, windows-x64). Mac `.zip` duplicates are P1 unless release tooling already emits both.

### Checksums (P0)

Ship alongside archives on release page:

```
vgen-1.0.0-*.tar.gz.sha256
vgen-1.0.0-*.zip.sha256
```

---

## 3. Version bump

### Files to change

| File | Change |
|------|--------|
| [`Cargo.toml`](../Cargo.toml) | `version = "1.0.0"` |
| [`templates/seed/vgen.yaml.tmpl`](../templates/seed/vgen.yaml.tmpl) | No edit required — `kit_version` rendered at init from `kit_version()` → `CARGO_PKG_VERSION` |
| `CHANGELOG.md` (new) | Add `## [1.0.0] - YYYY-MM-DD` with P0–P3B feature summary |
| [`README.md`](../README.md) | Add "Installing v1.0" section linking to release artifacts + install scripts |
| [`docs/agent-authoring-guide.md`](./agent-authoring-guide.md) | Note standalone install path |
| Archive `VERSION` | Written by packaging script at build time |

### Version surfacing in CLI

Already wired:

- `src/kit/load.rs` — `kit_version()` → `env!("CARGO_PKG_VERSION")`
- `src/commands/init.rs` — writes `kit_version` into `vgen.yaml`
- `src/mcp/handler.rs` — MCP `serverInfo.version`

Optional P1: `vgen --version` / `vgen version` subcommand (not required for v1.0 if `--help` shows crate version via clap).

### Changelog skeleton

```markdown
# Changelog

## [1.0.0] - 2026-XX-XX

### Added
- Standalone install packages (tar.gz / zip) with co-installed authoring kit templates
- install.sh (macOS/Linux) and install.ps1 (Windows)
- …

### Changed
- Stable 1.0.0 release; kit_version tracks CLI version

### Known limitations
- `vgen workflow validate` requires smriti_client linked at build time (path dep); see packaging script
```

---

## 4. Build pipeline

### Prerequisites (maintainer machine)

```bash
# Rust stable
rustup update stable

# Targets (P0)
rustup target add \
  aarch64-apple-darwin \
  x86_64-apple-darwin \
  x86_64-unknown-linux-gnu \
  x86_64-pc-windows-gnu

# Windows cross-compile from macOS (existing)
brew install mingw-w64   # per README + scripts/build-windows.sh

# Linux cross from macOS (if not building on Linux CI)
# Option A: build linux-x64 on a Linux runner / VM (recommended)
# Option B: cargo-zigbuild / cross (P1 document if adopted)
```

### Sibling `smriti_client` handling

Current dependency ([`Cargo.toml`](../Cargo.toml)):

```toml
smriti_client = { path = "../resmedai-core-framework/lib/smriti_client" }
```

**v1.0 approach:** packaging script ensures sibling checkout before `cargo build`.

New script: **`scripts/package-release.sh`**

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$(grep '^version' "$ROOT/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')"
CORE_REPO="${VGEN_CORE_FRAMEWORK:-$ROOT/../resmedai-core-framework}"
CORE_REF="${VGEN_CORE_REF:-main}"   # or tag, e.g. vX.Y.Z

# 1. Ensure core-framework present
if [[ ! -d "$CORE_REPO/lib/smriti_client" ]]; then
  git clone --depth 1 --branch "$CORE_REF" \
    git@github.com:ResMed/resmedai-core-framework.git "$CORE_REPO"
fi

# 2. Build release binaries per target (both bins)
build_target() {
  local target="$1"
  cargo build --release --target "$target" --bin vgen --bin vgen-mcp
}

# 3. Stage share/templates from $ROOT/templates/
# 4. Write VERSION, README-INSTALL.md, copy install scripts
# 5. tar.gz or zip → dist/vgen-${VERSION}-<platform>-<arch>.{tar.gz,zip}
# 6. sha256sum → dist/*.sha256
```

**Document in script header and README:**

- `VGEN_CORE_FRAMEWORK` — override path to checkout
- `VGEN_CORE_REF` — pin framework git ref used for release (record in release notes)

**Future decoupling (not v1.0):**

| Phase | Approach |
|-------|----------|
| **P2** | Replace path dep with git-pinned `smriti_client` crate in core-framework; release builds `cargo vendor` or fetch tagged crate |
| **P3** | Extract minimal workflow validator crate; drop runtime dependency on full core-framework |

### Per-target build commands

| Target | Command | Output binaries |
|--------|---------|-----------------|
| macOS arm64 | `cargo build --release --target aarch64-apple-darwin --bin vgen --bin vgen-mcp` | `target/aarch64-apple-darwin/release/{vgen,vgen-mcp}` |
| macOS x64 | `cargo build --release --target x86_64-apple-darwin --bin vgen --bin vgen-mcp` | `target/x86_64-apple-darwin/release/...` |
| Linux x64 | `cargo build --release --target x86_64-unknown-linux-gnu --bin vgen --bin vgen-mcp` | `target/x86_64-unknown-linux-gnu/release/...` |
| Windows x64 | `./scripts/build-windows.sh --arch x64` **(extend to build both bins)** | `target/x86_64-pc-windows-gnu/release/{vgen,vgen-mcp}.exe` |

### Extend `scripts/build-windows.sh`

Today it only builds `vgen` (default `--bin`). Change to:

```bash
cargo build --release --target "$TARGET" --bin vgen --bin vgen-mcp
```

### Build output directory

```
dist/
├── vgen-1.0.0-darwin-arm64.tar.gz
├── vgen-1.0.0-darwin-arm64.tar.gz.sha256
├── vgen-1.0.0-darwin-x64.tar.gz
├── vgen-1.0.0-linux-x64.tar.gz
├── vgen-1.0.0-windows-x64.zip
└── …
```

### Pre-release validation (maintainer)

```bash
cd resmed_vgen-cli
VGEN_CORE_FRAMEWORK=../resmedai-core-framework ./scripts/package-release.sh
cargo test                                    # from source checkout
./scripts/smoke-pr-agent.sh                   # against pr-agent-v2 (optional API checks)
```

---

## 5. Install script behavior

### Default install layout

| OS | Binaries (`PATH`) | Templates | Config snippet |
|----|-------------------|-----------|----------------|
| **macOS / Linux (user)** | `$HOME/.local/bin/` | `$HOME/.local/share/vgen/templates/` | `$HOME/.vgen/env` |
| **macOS / Linux (system)** | `$PREFIX/bin/` (default `/usr/local/bin`) | `$PREFIX/share/vgen/templates/` | `/etc/profile.d/vgen.sh` or `$PREFIX/etc/vgen/env` |
| **Windows (user)** | `%LOCALAPPDATA%\Programs\ResMate\bin\` | `%LOCALAPPDATA%\Programs\ResMate\share\templates\` | User env vars via `[Environment]::SetEnvironmentVariable` |
| **Windows (system)** | `%ProgramFiles%\ResMate\bin\` | `%ProgramFiles%\ResMate\share\templates\` | Machine env (requires admin) |

**P0 default:** user-local install (no `sudo` / no admin).

### `install.sh` (macOS / Linux) — step-by-step

New file: **`scripts/install.sh`** (copied into release root as `./install.sh`).

```bash
./install.sh [--prefix /usr/local] [--system] [--dry-run]
```

| Step | Action |
|------|--------|
| 1 | Detect OS; resolve `INSTALL_ROOT` (user-local or `--prefix`) |
| 2 | Resolve `BIN_DIR=$INSTALL_ROOT/bin` or `$HOME/.local/bin` |
| 3 | Resolve `SHARE_DIR=$INSTALL_ROOT/share/vgen` or `$HOME/.local/share/vgen` |
| 4 | Create directories: `$BIN_DIR`, `$SHARE_DIR/templates` |
| 5 | Copy `bin/vgen`, `bin/vgen-mcp` → `$BIN_DIR/` (mode `755`) |
| 6 | Copy `share/vgen/templates/*` → `$SHARE_DIR/templates/` (preserve tree) |
| 7 | Write `$HOME/.vgen/env` (or `$PREFIX/etc/vgen/env`): `export VGEN_TEMPLATES_DIR="$SHARE_DIR/templates"` |
| 8 | Idempotent shell hook: append `source "$HOME/.vgen/env"` to `~/.zshrc` / `~/.bashrc` if missing |
| 9 | Ensure `$BIN_DIR` on `PATH` (append `export PATH="$BIN_DIR:$PATH"` in same env file) |
| 10 | Print success + **post-install quick start** (§7) |
| 11 | Optional `--dry-run`: print actions only |

**Uninstall (P1):** `scripts/uninstall.sh` removing bins, share tree, env snippet.

### `install.ps1` (Windows) — step-by-step

New file: **`scripts/install.ps1`** (copied into release root).

```powershell
.\install.ps1 [-Scope User|Machine] [-InstallDir "$env:LOCALAPPDATA\Programs\ResMate"]
```

| Step | Action |
|------|--------|
| 1 | Require PowerShell 5.1+; `-ExecutionPolicy Bypass` documented in README-INSTALL |
| 2 | Resolve `$InstallDir`, `$BinDir = Join-Path $InstallDir "bin"`, `$TemplateDir = Join-Path $InstallDir "share\templates" |
| 3 | Create directories |
| 4 | Copy `bin\vgen.exe`, `bin\vgen-mcp.exe` → `$BinDir` |
| 5 | Copy `share\vgen\templates\*` → `$TemplateDir` recursively |
| 6 | Set **user** environment variables (persistent): `VGEN_TEMPLATES_DIR=$TemplateDir` |
| 7 | Prepend `$BinDir` to user `PATH` if not present |
| 8 | Print success + quick start; note **new terminal** required for PATH |
| 9 | Optional: write `%USERPROFILE%\.cursor\mcp.json` snippet if `-ConfigureMcp` (P1) |

**Batch wrapper (optional P1):** `install.cmd` calling `powershell -ExecutionPolicy Bypass -File install.ps1`.

### Template resolution after install

Priority (see §8 for CLI code):

1. `VGEN_TEMPLATES_DIR` — set by install script (explicit, overridable)
2. Install-time defaults compiled into `load.rs` (works even if user skips sourcing env file)
3. `CARGO_MANIFEST_DIR/templates` — dev checkout only
4. Error with remediation text pointing to re-run install or set env var

---

## 6. Template deployment

### Source of truth

```
resmed_vgen-cli/templates/
├── workspace/     # 119 files total under templates/ (~564 KB today)
├── recipes/
└── seed/
```

Packaging copies **`templates/`** verbatim to **`share/vgen/templates/`** in the archive.

### Install destinations

| Install mode | Templates path |
|--------------|----------------|
| Unix user | `$HOME/.local/share/vgen/templates/` |
| Unix system | `/usr/local/share/vgen/templates/` (or `$PREFIX/share/vgen/templates/`) |
| Windows user | `%LOCALAPPDATA%\Programs\ResMate\share\templates\` |
| Windows system | `%ProgramFiles%\ResMate\share\templates\` |

### Validation gate (install script)

After copy, verify:

```bash
test -d "$SHARE_DIR/templates/workspace" \
  && test -d "$SHARE_DIR/templates/recipes" \
  && test -f "$SHARE_DIR/templates/seed/vgen.yaml.tmpl"
```

PowerShell equivalent: `Test-Path` on the same three paths.

### Kit refresh without reinstalling CLI (P1)

Document manual refresh:

```bash
cp -a /path/to/new/templates/* "$HOME/.local/share/vgen/templates/"
```

Future: `vgen kit refresh` (P3 per AUTHORING-KIT-ARCHITECTURE.md §10).

---

## 7. Post-install developer flow

Include in **`docs/QUICKSTART.md`** (inside archive) and **`README-INSTALL.md`**.

### Unix

```bash
# 1. Install (from extracted archive)
cd vgen-1.0.0-darwin-arm64
./install.sh

# 2. Reload shell (or open new terminal)
source ~/.vgen/env   # if not auto-sourced

# 3. Verify CLI
vgen --help
vgen-mcp --help 2>/dev/null || true   # MCP has no subcommands; smoke via doctor

# 4. Bootstrap workspace
mkdir ~/my-use-case && cd ~/my-use-case
vgen init --name my-use-case
vgen scaffold oracle-pr --name my-use-case   # optional

# 5. Configure API access
cat > .env <<'EOF'
VGEN_API_KEY=<your-key>
EOF

# 6. Pre-push gates
vgen doctor
vgen graph
vgen validate
vgen push-all --dry-run
```

### Windows (PowerShell)

```powershell
cd vgen-1.0.0-windows-x64
.\install.ps1
# Open new terminal
mkdir $HOME\my-use-case; cd $HOME\my-use-case
vgen init --name my-use-case
vgen doctor
```

### MCP (Cursor)

After install, point MCP at installed binary ([`docs/mcp-setup.md`](./mcp-setup.md)):

```json
{
  "mcpServers": {
    "vgen": {
      "command": "C:\\Users\\<you>\\AppData\\Local\\Programs\\ResMate\\bin\\vgen-mcp.exe",
      "args": [],
      "env": {
        "VGEN_API_KEY": "${env:VGEN_API_KEY}",
        "VGEN_TEMPLATES_DIR": "${env:VGEN_TEMPLATES_DIR}"
      }
    }
  }
}
```

Install script can print OS-specific absolute path for copy-paste.

### Expected `vgen init` outcome

Same as dev checkout ([`src/commands/init.rs`](../src/commands/init.rs)):

- `AGENTS.md`, `.cursor/skills/vgen-use-case/SKILL.md`, `platform/`, `cli/`, etc.
- `vgen.yaml` with `kit_version: "1.0.0"`
- Empty live dirs: `tools/`, `agents/`, `assistants/`, `hitl/`, `workflows/`

---

## 8. CLI code changes needed

### P0 — `src/kit/load.rs`

Extend `templates_root()` resolution **before** erroring. Proposed order:

| # | Source | Notes |
|---|--------|-------|
| 1 | `VGEN_TEMPLATES_DIR` | Existing; validate `workspace/` subdir |
| 2 | **Install-relative to executable** | `canonicalize(exe_dir/../share/vgen/templates)` — works for portable unzip layout |
| 3 | **Unix user default** | `$HOME/.local/share/vgen/templates` via `dirs` crate (already a dependency) |
| 4 | **Unix system default** | `/usr/local/share/vgen/templates` |
| 5 | **Windows user default** | `%LOCALAPPDATA%\Programs\ResMate\share\templates` |
| 6 | **Windows system default** | `%ProgramFiles%\ResMate\share\templates` |
| 7 | `CARGO_MANIFEST_DIR/templates` | Dev / `cargo test` only (existing) |
| 8 | Error | Include all attempted paths + "Re-run install.sh or set VGEN_TEMPLATES_DIR" |

**Estimated diff:** ~60–90 lines in [`src/kit/load.rs`](../src/kit/load.rs); helper `fn candidate_template_roots() -> Vec<PathBuf>`.

```rust
// Pseudocode — not implemented
fn installed_template_candidates() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            v.push(parent.join("../share/vgen/templates"));
        }
    }
    if let Some home) = dirs::home_dir() {
        v.push(home.join(".local/share/vgen/templates"));
    }
    #[cfg(unix)]
    v.push(PathBuf::from("/usr/local/share/vgen/templates"));
    #[cfg(windows)]
    { /* LOCALAPPDATA / ProgramFiles */ }
    v
}
```

Each candidate: accept only if `join("workspace").is_dir()`.

### P0 — tests

| File | Change |
|------|--------|
| `tests/init_authoring_kit_test.rs` (or new `tests/templates_root_test.rs`) | Assert `templates_root()` finds `CARGO_MANIFEST_DIR/templates` in dev |
| New test with temp dir | Set `VGEN_TEMPLATES_DIR` to temp templates tree; init succeeds |

### P1 — `vgen doctor`

Add check `templates_available`: calls `templates_root()`; reports path or remediation. Helps authors who installed CLI but skipped env hook.

**File:** [`src/doctor.rs`](../src/doctor.rs) (~20 lines).

### P1 — version command

Optional `vgen version --json` emitting `{ "version": "1.0.0", "kit_templates": "<path>" }`.

### Not needed for v1.0

- `src/kit/embed.rs` — defer to rust-embed milestone
- Changes to `init` / `scaffold` copy logic — already use `templates_root()`

---

## 9. Implementation phases

### P0 — Must ship for v1.0.0

| # | Task | Files | Est. |
|---|------|-------|------|
| 1 | Bump version to `1.0.0` | `Cargo.toml`, `CHANGELOG.md` | S |
| 2 | Extend `templates_root()` default lookup | `src/kit/load.rs`, tests | M |
| 3 | `scripts/package-release.sh` | new | L |
| 4 | Extend `scripts/build-windows.sh` for `vgen-mcp` | `scripts/build-windows.sh` | S |
| 5 | `scripts/install.sh` + release copy | new | M |
| 6 | `scripts/install.ps1` + release copy | new | M |
| 7 | Archive docs: `README-INSTALL.md`, `docs/QUICKSTART.md`, `VERSION` | new / generated | S |
| 8 | README install section | `README.md` | S |
| 9 | Manual build all 4 targets + smoke on fresh VMs | — | L |
| 10 | GitHub release upload (manual) + checksums | — | S |

**P0 exit criteria:**

```bash
# On clean macOS VM after install.sh:
vgen init --name test && test -f AGENTS.md
vgen workflow validate --help   # binary runs; full validate needs workspace workflow

# On clean Windows VM after install.ps1:
vgen init --name test
```

### P1 — Nice to have (v1.0.x)

| Task | Notes |
|------|-------|
| `vgen doctor` templates check | §8 |
| Mac `.zip` alternate format | Same tree as `.tar.gz` |
| `install.cmd` Windows wrapper | Calls `install.ps1` |
| `scripts/uninstall.sh` / `uninstall.ps1` | Reverse install |
| GitHub Actions `release.yml` | Matrix build on tag push |
| `vgen version` subcommand | Operator visibility |
| MCP auto-config flag on install | `-ConfigureMcp` |

### P2 — Future

| Task | Notes |
|------|-------|
| **`smriti_client` standalone** | Git-pinned dep; document `VGEN_CORE_REF` in releases |
| **`rust-embed` templates** | Single-binary distribution; feature flag `embedded-templates` |
| Linux arm64, musl | Expand matrix |
| Homebrew formula / winget manifest | Package manager installs |
| macOS notarization | Gatekeeper-friendly |

### P3 — Future

| Task | Notes |
|------|-------|
| `vgen kit refresh` | Merge kit docs without touching live artifacts |
| Vendored workflow validator crate | Remove core-framework build dependency entirely |

---

## 10. Testing checklist

Run on **fresh machines** (no Rust, no sibling repos) per artifact.

### macOS (arm64 + x64)

- [ ] Extract `vgen-1.0.0-darwin-*.tar.gz`
- [ ] `./install.sh` completes without sudo
- [ ] New shell: `which vgen` → `~/.local/bin/vgen`
- [ ] `echo $VGEN_TEMPLATES_DIR` → `~/.local/share/vgen/templates`
- [ ] `vgen init --name smoke` → `AGENTS.md`, `.cursor/skills/...`, `vgen.yaml` with `kit_version: "1.0.0"`
- [ ] `vgen scaffold minimal --name smoke` → recipe files
- [ ] `vgen doctor --offline` → passes workspace checks
- [ ] `vgen-mcp` responds to `tools/list` JSON-RPC smoke
- [ ] Optional: `vgen workflow validate` against scaffolded workflow folder

### Linux (x64)

- [ ] Same as macOS via `install.sh`
- [ ] Confirm dynamic linker: `ldd $(which vgen)` — document glibc minimum (build on oldest target or manylinux)

### Windows (x64)

- [ ] Extract `vgen-1.0.0-windows-x64.zip`
- [ ] `powershell -ExecutionPolicy Bypass -File .\install.ps1`
- [ ] New cmd/PowerShell: `where vgen` → `%LOCALAPPDATA%\Programs\ResMate\bin\vgen.exe`
- [ ] `[Environment]::GetEnvironmentVariable("VGEN_TEMPLATES_DIR","User")` set correctly
- [ ] `vgen init --name smoke` succeeds
- [ ] `vgen-mcp.exe` JSON-RPC smoke

### Regression (maintainer checkout)

- [ ] `cargo test` still passes with `CARGO_MANIFEST_DIR/templates`
- [ ] `VGEN_TEMPLATES_DIR` override still wins over defaults
- [ ] `./scripts/smoke-pr-agent.sh` against `pr-agent-v2`

### Negative tests

- [ ] Delete `VGEN_TEMPLATES_DIR` and default dirs → `vgen init` error message lists remediation
- [ ] Run install twice → idempotent, no duplicate PATH entries

---

## 11. Open questions / decisions log

| ID | Question | Decision | Date | Owner |
|----|----------|----------|------|-------|
| D1 | Co-installed vs `rust-embed` templates? | **Co-installed for v1.0**; rust-embed P2/v1.1 | 2026-07-04 | Packaging plan |
| D2 | User-local vs system default install? | **User-local default**; `--prefix` / `-Scope Machine` opt-in | 2026-07-04 | Packaging plan |
| D3 | Pin `resmedai-core-framework` ref per release? | **Yes** — record `VGEN_CORE_REF` in CHANGELOG/release notes | TBD | Release manager |
| D4 | Linux build host: cross vs native? | **Prefer native linux-x64 runner** for glibc compatibility; document minimum Ubuntu version | TBD | Release manager |
| D5 | Ship macOS `.zip` in P0? | **Optional P1** unless dual-format required day one | 2026-07-04 | Packaging plan |
| D6 | Code signing (macOS/Windows)? | **Defer P2**; document Gatekeeper/SmartScreen bypass for internal dev | 2026-07-04 | Packaging plan |
| D7 | Include `LICENSE` file in repo? | Add if missing before first public release | TBD | Legal / release |
| D8 | Minimum supported OS versions? | Propose: macOS 12+, Ubuntu 20.04+, Windows 10+ | TBD | QA |

---

## Appendix A — File change summary

| Path | Action | Phase |
|------|--------|-------|
| `Cargo.toml` | Modify version | P0 |
| `CHANGELOG.md` | Add | P0 |
| `src/kit/load.rs` | Extend template lookup | P0 |
| `src/doctor.rs` | Templates check | P1 |
| `scripts/package-release.sh` | Add | P0 |
| `scripts/install.sh` | Add | P0 |
| `scripts/install.ps1` | Add | P0 |
| `scripts/build-windows.sh` | Build both bins | P0 |
| `scripts/uninstall.sh` / `uninstall.ps1` | Add | P1 |
| `docs/PACKAGING-V1-PLAN.md` | This document | — |
| `docs/QUICKSTART.md` | Add (also staged in archive) | P0 |
| `README.md` | Install section | P0 |
| `.github/workflows/release.yml` | CI release | P1 |

---

## Appendix B — Related commands (maintainer cheat sheet)

```bash
# Dev smoke (from source)
cargo test
cargo run -- init --name local-test
VGEN_TEMPLATES_DIR=./templates vgen init --name explicit

# Package (once script exists)
VGEN_CORE_FRAMEWORK=../resmedai-core-framework \
VGEN_CORE_REF=main \
  ./scripts/package-release.sh

# Verify archive contents
tar tzf dist/vgen-1.0.0-darwin-arm64.tar.gz | head
unzip -l dist/vgen-1.0.0-windows-x64.zip | head
```

---

*Plan only — no implementation in this document. Execute P0 tasks in follow-up PR(s) before tagging `v1.0.0`.*
