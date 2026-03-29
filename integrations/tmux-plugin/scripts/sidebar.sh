#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$SCRIPT_DIR/common.sh"

bin="$(require_swarmux_bin)"
pane_id="${1:?missing pane id}"

"$bin" panes switch --launch-sidebar --pane-id "$pane_id"
