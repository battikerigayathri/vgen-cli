#!/usr/bin/env bash
# Build resmate CLI for Windows (x64 or x86) from macOS.
# One-time: rustup target add x86_64-pc-windows-gnu i686-pc-windows-gnu
#           brew install mingw-w64

set -e

ARCH=""
RELEASE=true

usage() {
  echo "Usage: $0 --arch <x64|x86> [--debug]"
  echo "  --arch x64   Build for 64-bit Windows (x86_64-pc-windows-gnu)"
  echo "  --arch x86   Build for 32-bit Windows (i686-pc-windows-gnu)"
  echo "  --debug      Build debug binary (default: release)"
  exit 1
}

while [[ $# -gt 0 ]]; do
  case $1 in
    --arch)
      ARCH="$2"
      shift 2
      ;;
    --debug)
      RELEASE=false
      shift
      ;;
    -h|--help)
      usage
      ;;
    *)
      echo "Unknown option: $1"
      usage
      ;;
  esac
done

case "$ARCH" in
  x64|x86_64)
    TARGET="x86_64-pc-windows-gnu"
    ;;
  x86|i686)
    TARGET="i686-pc-windows-gnu"
    ;;
  *)
    echo "Error: --arch must be x64 or x86 (got: ${ARCH:-<none>})"
    usage
    ;;
esac

echo "Building for Windows: $TARGET (arch: $ARCH)"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
cd "$ROOT"

if [[ "$RELEASE" == true ]]; then
  cargo build --release --target "$TARGET" --bin resmate --bin resmate-mcp
  EXE="$ROOT/target/$TARGET/release/resmate.exe"
  MCP_EXE="$ROOT/target/$TARGET/release/resmate-mcp.exe"
else
  cargo build --target "$TARGET" --bin resmate --bin resmate-mcp
  EXE="$ROOT/target/$TARGET/debug/resmate.exe"
  MCP_EXE="$ROOT/target/$TARGET/debug/resmate-mcp.exe"
fi

echo "Done. Binaries:"
echo "  resmate:     $EXE"
echo "  resmate-mcp: $MCP_EXE"
