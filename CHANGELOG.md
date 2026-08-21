# Changelog

All notable changes to the ResMate CLI are documented here.

## [1.0.0] - 2026-07-04

### Added

- Standalone install packages (`.tar.gz` for macOS/Linux, `.zip` for Windows) with co-installed authoring kit templates
- `install.sh` (macOS/Linux) and `install.ps1` (Windows) for user-local installs without Rust toolchain
- `scripts/package-release.sh` — builds release binaries and assembles platform archives under `dist/`
- Install-time template lookup in `templates_root()` (env var, install-relative, OS defaults, dev checkout)
- Release docs: `README-INSTALL.md`, `docs/QUICKSTART.md`, MCP setup snippet in archive
- P0–P3B CLI commands: workspace info, doctor, graph, validate, workflow validate, push-all, init, scaffold, sync, diff, MCP server

### Changed

- Stable 1.0.0 release; `kit_version` in `vgen.yaml` tracks CLI version via `CARGO_PKG_VERSION`

### Known limitations

- `vgen workflow validate` requires `smriti_client` linked at build time (path dependency to `resmedai-core-framework`); release builds must use `VGEN_CORE_FRAMEWORK`
- Templates are co-installed under `share/vgen/templates/` (not embedded in the binary); `rust-embed` deferred to v1.1
- Linux x64 builds should be produced on a Linux host for glibc compatibility; macOS cross-compile to Linux is not included in P0
- macOS/Windows code signing and notarization deferred
