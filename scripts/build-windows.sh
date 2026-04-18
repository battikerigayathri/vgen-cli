#!/usr/bin/env bash
# Build vgen CLI for Windows (x64 or x86) from macOS.
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
cd "$ROOT"

if [[ "$RELEASE" == true ]]; then
  cargo build --release --target "$TARGET"
  EXE="$ROOT/target/$TARGET/release/vgen.exe"
else
  cargo build --target "$TARGET"
  EXE="$ROOT/target/$TARGET/debug/vgen.exe"
fi

echo "Done. Binary: $EXE"
