#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$SCRIPT_DIR/common.sh"

bin="$(require_swarmux_bin)"
command="exec $(shell_quote "$bin") overview --tui"

tmux display-popup -B -w 100% -h 100% -E "$command"
