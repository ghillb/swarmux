#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$SCRIPT_DIR/common.sh"

bin="$(require_swarmux_bin)"
pane_id="${1:?missing pane id}"
command="exec $(shell_quote "$bin") panes switch --tui --pane-id $(shell_quote "$pane_id")"

tmux display-popup -B -w 100% -h 100% -E "$command"
