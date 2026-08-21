# ResMate CLI — Install & Quick Start

Extract the archive for your platform, then run the install script. No Rust toolchain required.

| Platform | Archive | Install |
|----------|---------|---------|
| macOS (Apple Silicon) | `vgen-*-darwin-arm64.tar.gz` | `./install.sh` |
| macOS (Intel) | `vgen-*-darwin-x64.tar.gz` | `./install.sh` |
| Linux (x64) | `vgen-*-linux-x64.tar.gz` | `./install.sh` |
| Windows (x64) | `vgen-*-windows-x64.zip` | PowerShell: `powershell -ExecutionPolicy Bypass -File .\install.ps1` or Git Bash: `./install.sh` |

## Default install locations

| OS | Binaries | Templates |
|----|----------|-----------|
| macOS / Linux (user) | `~/.local/bin/` | `~/.local/share/vgen/templates/` |
| macOS / Linux (system) | `/usr/local/bin/` (or `--prefix`) | `/usr/local/share/vgen/templates/` |
| Windows (user) | `%LOCALAPPDATA%\Programs\ResMate\bin\` | `%LOCALAPPDATA%\Programs\ResMate\share\templates\` |

Install scripts set `VGEN_TEMPLATES_DIR` and add the binary directory to `PATH`.

## Unix install options

```bash
./install.sh                  # user-local (default, no sudo)
./install.sh --prefix /opt/vgen --system
./install.sh --dry-run
```

After install, reload your shell:

```bash
source ~/.vgen/env
vgen --help
```

## Windows install options

```powershell
.\install.ps1
.\install.ps1 -Scope User -InstallDir "$env:LOCALAPPDATA\Programs\ResMate"
.\install.ps1 -Scope Machine   # requires admin
```

```bash
# Git Bash / MSYS shell
./install.sh
source ~/.vgen/env
```

Open a **new** terminal after install so `PATH` updates.

If PowerShell blocks the script (`running scripts is disabled`), use:

```powershell
powershell -ExecutionPolicy Bypass -File .\install.ps1
```

## OS security / first-run prompts

Downloaded release archives may carry OS-specific “untrusted file” markers. Install scripts clear what they can automatically; some prompts still require user action or code signing.

| OS | What happens | Handled by install script |
|----|--------------|---------------------------|
| **macOS** | Gatekeeper quarantine (`com.apple.quarantine` on extracted binaries) can block execution | `install.sh` runs `xattr -cr` on bundle and installed binaries (macOS only) |
| **Windows** | Mark of the Web (`Zone.Identifier`) on downloaded files | `install.ps1` runs `Unblock-File` on source and installed `.exe` files |
| **Windows** | PowerShell execution policy may block `install.ps1` | Run with `-ExecutionPolicy Bypass` (see above) |
| **Windows** | SmartScreen “Windows protected your PC” for unsigned executables | **Not bypassed** — user must choose “More info” → “Run anyway”, or sign binaries in a future release |

Manual fallback if a binary still will not run:

```bash
# macOS (installed path)
xattr -cr ~/.local/bin/vgen ~/.local/bin/vgen-mcp
```

```powershell
# Windows (installed path)
Unblock-File "$env:LOCALAPPDATA\Programs\ResMate\bin\vgen.exe"
Unblock-File "$env:LOCALAPPDATA\Programs\ResMate\bin\vgen-mcp.exe"
```

## Bootstrap a use-case workspace

```bash
mkdir ~/my-use-case && cd ~/my-use-case
vgen init --name my-use-case
vgen scaffold oracle-pr --name my-use-case   # optional
vgen doctor
```

Configure API access in `.env`:

```bash
VGEN_API_KEY=<your-key>
```

See [docs/QUICKSTART.md](docs/QUICKSTART.md) for the full pre-push loop and MCP setup.

## Troubleshooting

- **macOS — “damaged” or “cannot be opened”:** Re-run `./install.sh` or manually `xattr -cr` on the installed binaries (see OS security section).
- **Windows — SmartScreen blocks `vgen.exe`:** Click “More info” → “Run anyway” (unsigned build). `Unblock-File` alone does not dismiss SmartScreen.
- **Windows — `install.ps1` blocked:** Use `powershell -ExecutionPolicy Bypass -File .\install.ps1`.
- **`vgen init` — templates not found:** Re-run `install.sh` / `install.ps1`, or set `VGEN_TEMPLATES_DIR` to your templates directory (must contain `workspace/`).
- **`vgen workflow validate`:** Requires a build linked to `smriti_client` (included in release binaries). No runtime API call.
- **Refresh kit docs without reinstalling CLI:** Copy a newer `templates/` tree over your install templates directory.

## Maintainer build

From `resmed_vgen-cli` with sibling `resmedai-core-framework`:

```bash
VGEN_CORE_FRAMEWORK=../resmedai-core-framework ./scripts/package-release.sh
```

Record `VGEN_CORE_REF` (framework git ref) in release notes when pinning builds.
