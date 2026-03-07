#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$ROOT/target/debug/swarmux"
STATE_DIR="$(mktemp -d)"
REPO_DIR="$(mktemp -d)"
SESSION="swarmux-e2e-codex"

cleanup() {
  tmux kill-session -t "$SESSION" >/dev/null 2>&1 || true
  rm -rf "$STATE_DIR" "$REPO_DIR"
}
trap cleanup EXIT

cargo build --quiet --manifest-path "$ROOT/Cargo.toml"

git -C "$REPO_DIR" init -b main >/dev/null

export SWARMUX_HOME="$STATE_DIR"

"$BIN" init >/dev/null

PAYLOAD="$(jq -nc \
  --arg repo_root "$REPO_DIR" \
  --arg session "$SESSION" \
  --arg worktree "$REPO_DIR" \
  '{
    title: "Codex e2e task",
    repo_ref: "e2e",
    repo_root: $repo_root,
    mode: "manual",
    worktree: $worktree,
    session: $session,
    command: [
      "codex","exec",
      "-m","gpt-5.1-codex-mini",
      "--dangerously-bypass-approvals-and-sandbox",
      "-C",$repo_root,
      "Create a file named e2e-proof.txt with the exact contents READY and then stop."
    ]
  }')"

TASK_ID="$("$BIN" --output json submit --json "$PAYLOAD" | jq -r '.id')"
"$BIN" --output json start "$TASK_ID" >/dev/null

for _ in $(seq 1 90); do
  if [ -f "$REPO_DIR/e2e-proof.txt" ]; then
    break
  fi
  sleep 2
done

"$BIN" --output json reconcile >/dev/null
STATUS="$("$BIN" --output json show "$TASK_ID" | jq -r '.state')"

if [ ! -f "$REPO_DIR/e2e-proof.txt" ]; then
  echo "e2e-proof.txt was not created" >&2
  "$BIN" --output json logs "$TASK_ID" --raw || true
  exit 1
fi

if [ "$(cat "$REPO_DIR/e2e-proof.txt")" != "READY" ]; then
  echo "e2e-proof.txt content mismatch" >&2
  cat "$REPO_DIR/e2e-proof.txt" >&2
  exit 1
fi

if [ "$STATUS" != "succeeded" ]; then
  echo "task did not reconcile to succeeded: $STATUS" >&2
  "$BIN" --output json logs "$TASK_ID" --raw || true
  exit 1
fi

printf 'codex e2e succeeded for task %s\n' "$TASK_ID"
