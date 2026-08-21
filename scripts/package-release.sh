#!/usr/bin/env bash
# Build release binaries and assemble platform archives under dist/.
#
# Prerequisites:
#   - Rust stable with targets for desired platforms
#   - Sibling resmedai-core-framework checkout (smriti_client path dep)
#   - macOS Windows cross: brew install mingw-w64, rustup target add x86_64-pc-windows-gnu
#
# Environment:
#   VGEN_CORE_FRAMEWORK  Path to core-framework repo (default: ../resmedai-core-framework)
#   VGEN_CORE_REF        Git ref to clone if core repo missing (default: main)
#   VGEN_TARGETS         Space-separated platform slugs to build (default: host + feasible cross)
#                           Values: darwin-arm64 darwin-x64 linux-x64 windows-x64
#
# Usage:
#   ./scripts/package-release.sh
#   VGEN_TARGETS="darwin-arm64" ./scripts/package-release.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# Always stage from the repo target dir so packaging does not pick up a stale binary
# when an outer environment (e.g. CI/sandbox) overrides CARGO_TARGET_DIR.
export CARGO_TARGET_DIR="$ROOT/target"
VERSION="$(grep '^version' "$ROOT/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')"
CORE_REPO="${VGEN_CORE_FRAMEWORK:-$ROOT/../resmedai-core-framework}"
CORE_REF="${VGEN_CORE_REF:-main}"
DIST="$ROOT/dist"

rust_target_for_slug() {
  case "$1" in
    darwin-arm64) echo "aarch64-apple-darwin" ;;
    darwin-x64) echo "x86_64-apple-darwin" ;;
    linux-x64) echo "x86_64-unknown-linux-gnu" ;;
    windows-x64) echo "x86_64-pc-windows-gnu" ;;
    *) return 1 ;;
  esac
}

is_windows_slug() {
  [[ "$1" == "windows-x64" ]]
}

ensure_core_framework() {
  if [[ -d "$CORE_REPO/lib/smriti_client" ]]; then
    echo "Using core framework: $CORE_REPO"
    return 0
  fi
  echo "Core framework not found at $CORE_REPO"
  if [[ -n "${VGEN_SKIP_CLONE:-}" ]]; then
    echo "Error: VGEN_SKIP_CLONE set and smriti_client missing." >&2
    exit 1
  fi
  echo "Cloning resmedai-core-framework (ref: $CORE_REF)..."
  git clone --depth 1 --branch "$CORE_REF" \
    "https://github.com/ResMed/resmedai-core-framework.git" "$CORE_REPO" || {
    echo "Error: clone failed. Set VGEN_CORE_FRAMEWORK to an existing checkout." >&2
    exit 1
  }
}

detect_host_slug() {
  local os arch
  os="$(uname -s)"
  case "$os" in
    Darwin) os="darwin" ;;
    Linux) os="linux" ;;
    *) os="unknown" ;;
  esac
  case "$(uname -m)" in
    arm64|aarch64) arch="arm64" ;;
    x86_64|amd64) arch="x64" ;;
    *) arch="unknown" ;;
  esac
  echo "${os}-${arch}"
}

default_targets() {
  local host="$1"
  echo "$host"
  case "$host" in
    darwin-arm64)
      if rustup target list --installed 2>/dev/null | grep -q '^x86_64-apple-darwin$'; then
        echo "darwin-x64"
      fi
      if rustup target list --installed 2>/dev/null | grep -q '^x86_64-pc-windows-gnu$'; then
        echo "windows-x64"
      fi
      ;;
    darwin-x64)
      if rustup target list --installed 2>/dev/null | grep -q '^aarch64-apple-darwin$'; then
        echo "darwin-arm64"
      fi
      if rustup target list --installed 2>/dev/null | grep -q '^x86_64-pc-windows-gnu$'; then
        echo "windows-x64"
      fi
      ;;
    linux-x64)
      if rustup target list --installed 2>/dev/null | grep -q '^x86_64-pc-windows-gnu$'; then
        echo "windows-x64"
      fi
      ;;
  esac
}

target_installed() {
  local slug="$1"
  local rust
  rust="$(rust_target_for_slug "$slug")" || return 1
  rustup target list --installed 2>/dev/null | grep -qx "$rust"
}

release_dir_for_rust_target() {
  local rust="$1"
  local explicit="$ROOT/target/$rust/release"
  local native="$ROOT/target/release"
  if [[ -d "$explicit" ]]; then
    echo "$explicit"
  else
    echo "$native"
  fi
}

build_slug() {
  local slug="$1"
  local rust
  rust="$(rust_target_for_slug "$slug")"
  echo "==> Building $slug ($rust)"
  cd "$ROOT"
  if is_windows_slug "$slug"; then
    "$ROOT/scripts/build-windows.sh" --arch x64
  else
    cargo build --release --target "$rust" --bin vgen --bin vgen-mcp
  fi
}

write_checksum() {
  local archive="$1"
  local base
  base="$(basename "$archive")"
  if command -v shasum >/dev/null 2>&1; then
    (cd "$DIST" && shasum -a 256 "$base" > "${base}.sha256")
  elif command -v sha256sum >/dev/null 2>&1; then
    (cd "$DIST" && sha256sum "$base" > "${base}.sha256")
  fi
}

stage_archive() {
  local slug="$1"
  local rust
  rust="$(rust_target_for_slug "$slug")"
  local bundle="vgen-${VERSION}-${slug}"
  local stage="$DIST/.staging-${slug}"
  local bundle_dir="$stage/$bundle"
  local target_dir
  target_dir="$(release_dir_for_rust_target "$rust")"

  rm -rf "$stage"
  mkdir -p "$bundle_dir/bin" "$bundle_dir/share/vgen/templates" "$bundle_dir/docs"

  if is_windows_slug "$slug"; then
    cp "$target_dir/vgen.exe" "$target_dir/vgen-mcp.exe" "$bundle_dir/bin/"
  else
    if [[ ! -f "$target_dir/vgen" ]]; then
      echo "Error: missing $target_dir/vgen after build" >&2
      return 1
    fi
    cp "$target_dir/vgen" "$target_dir/vgen-mcp" "$bundle_dir/bin/"
    chmod 755 "$bundle_dir/bin/vgen" "$bundle_dir/bin/vgen-mcp"
  fi

  cp -a "$ROOT/templates/." "$bundle_dir/share/vgen/templates/"
  echo "$VERSION" > "$bundle_dir/VERSION"
  cp "$ROOT/scripts/install.sh" "$bundle_dir/install.sh"
  if is_windows_slug "$slug"; then
    cp "$ROOT/scripts/install.ps1" "$bundle_dir/install.ps1"
  fi
  chmod 755 "$bundle_dir/install.sh"
  cp "$ROOT/docs/README-INSTALL.md" "$bundle_dir/README-INSTALL.md"
  cp "$ROOT/docs/QUICKSTART.md" "$bundle_dir/docs/QUICKSTART.md"
  cp "$ROOT/docs/mcp-setup-snippet.json" "$bundle_dir/docs/mcp-setup-snippet.json"
  if [[ -f "$ROOT/LICENSE" ]]; then
    cp "$ROOT/LICENSE" "$bundle_dir/LICENSE"
  fi

  mkdir -p "$DIST"
  if is_windows_slug "$slug"; then
    local archive="$DIST/${bundle}.zip"
    (cd "$stage" && zip -qr "$archive" "$bundle")
    write_checksum "$archive"
    echo "Created $archive"
  else
    local archive="$DIST/${bundle}.tar.gz"
    tar -C "$stage" -czf "$archive" "$bundle"
    write_checksum "$archive"
    echo "Created $archive"
  fi

  rm -rf "$stage"
}

ensure_core_framework
mkdir -p "$DIST"

HOST="$(detect_host_slug)"
echo "ResMate packaging v$VERSION (host: $HOST)"
echo "Core framework ref: $CORE_REF"

if [[ -n "${VGEN_TARGETS:-}" ]]; then
  read -r -a TARGET_SLUGS <<< "$VGEN_TARGETS"
else
  TARGET_SLUGS=()
  while IFS= read -r slug; do
    [[ -n "$slug" ]] && TARGET_SLUGS+=("$slug")
  done < <(default_targets "$HOST")
fi

BUILT=()
SKIPPED=()

for slug in "${TARGET_SLUGS[@]}"; do
  if ! rust_target_for_slug "$slug" >/dev/null 2>&1; then
    echo "Warning: unknown platform slug '$slug', skipping." >&2
    SKIPPED+=("$slug (unknown)")
    continue
  fi
  if ! target_installed "$slug"; then
    echo "Skipping $slug — Rust target $(rust_target_for_slug "$slug") not installed."
    echo "  Install with: rustup target add $(rust_target_for_slug "$slug")"
    SKIPPED+=("$slug (target not installed)")
    continue
  fi
  if ! build_slug "$slug"; then
    echo "Warning: build failed for $slug, skipping archive." >&2
    SKIPPED+=("$slug (build failed)")
    continue
  fi
  if ! stage_archive "$slug"; then
    echo "Warning: staging failed for $slug." >&2
    SKIPPED+=("$slug (stage failed)")
    continue
  fi
  BUILT+=("$slug")
done

echo ""
echo "Packaging complete."
if ((${#BUILT[@]} > 0)); then
  echo "  Built:   ${BUILT[*]}"
else
  echo "  Built:   (none)"
fi
if ((${#SKIPPED[@]} > 0)); then
  echo "  Skipped: ${SKIPPED[*]}"
fi
echo "  Output:  $DIST/"
