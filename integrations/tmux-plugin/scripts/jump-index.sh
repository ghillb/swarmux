#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$SCRIPT_DIR/common.sh"

bin="$(require_swarmux_bin)"
index="${1:?missing index}"
pane_id="${2:-}"

if [ -n "$pane_id" ]; then
  "$bin" panes jump --index "$index" --exclude-pane-id "$pane_id" >/dev/null
else
  "$bin" panes jump --index "$index" >/dev/null
fi
