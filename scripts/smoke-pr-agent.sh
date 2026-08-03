#!/usr/bin/env bash
# Local integration smoke for pr-agent-v2 workspace (no CI).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PR_AGENT_ROOT="${PR_AGENT_ROOT:-/Users/roshankgujarathi/Workspace/ResMed/pr-agent-v2}"
BIN="${RESMATE_BIN:-$ROOT/target/debug/resmate}"

if [[ ! -x "$BIN" ]]; then
  echo "Building resmate..."
  (cd "$ROOT" && cargo build --quiet)
fi

if [[ ! -d "$PR_AGENT_ROOT" ]]; then
  echo "SKIP: pr-agent-v2 not found at $PR_AGENT_ROOT"
  exit 0
fi

cd "$PR_AGENT_ROOT"

echo "== smoke: workspace info =="
"$BIN" --json workspace info | jq -e '.ok == true' >/dev/null

echo "== smoke: graph =="
"$BIN" --json graph | jq -e '.ok == true' >/dev/null

echo "== smoke: workflow validate oracle-purchase-requisition =="
"$BIN" --json workflow validate oracle-purchase-requisition | jq -e '.ok == true' >/dev/null

echo "== smoke: validate (local) =="
"$BIN" --json validate | jq -e '.data.summary.error_count == 0' >/dev/null

echo "== smoke: push-all dry-run =="
"$BIN" --json push-all --dry-run | jq -e '.ok == true' >/dev/null

if [[ -n "${RESMATE_API_KEY:-}" ]]; then
  echo "== smoke: validate --remote =="
  "$BIN" --json validate --remote | jq -e '.data.summary.error_count == 0' >/dev/null

  echo "== smoke: diff tool roc-search-users =="
  "$BIN" --json diff tool roc-search-users | jq -e '.data.has_changes == false' >/dev/null
else
  echo "SKIP: remote checks (RESMATE_API_KEY not set)"
fi

echo "Smoke passed."
