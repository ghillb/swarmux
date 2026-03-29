#!/usr/bin/env bash
set -euo pipefail

swarmux_bin() {
  local configured
  configured="$(tmux show-option -gqv @swarmux-bin 2>/dev/null || true)"
  if [ -n "$configured" ]; then
    printf '%s\n' "$configured"
    return 0
  fi

  command -v swarmux
}

require_swarmux_bin() {
  local bin
  if ! bin="$(swarmux_bin 2>/dev/null)"; then
    tmux display-message "swarmux plugin: install swarmux or set @swarmux-bin"
    exit 1
  fi

  printf '%s\n' "$bin"
}

shell_quote() {
  printf '%q' "$1"
}
