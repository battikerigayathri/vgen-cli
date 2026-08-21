#!/usr/bin/env bash
# Maintainer script: refresh templates/workspace from core cli-context.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CORE_CTX="${VGEN_CORE_CLI_CONTEXT:-$ROOT/../resmedai-core-framework/cli-context}"

if [[ ! -d "$CORE_CTX" ]]; then
  echo "Core cli-context not found at: $CORE_CTX" >&2
  echo "Set VGEN_CORE_CLI_CONTEXT to the cli-context directory." >&2
  exit 1
fi

rsync -a --delete --exclude='.git' "$CORE_CTX/" "$ROOT/templates/workspace/"
echo "Synced $CORE_CTX -> $ROOT/templates/workspace/"
