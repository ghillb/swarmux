#!/usr/bin/env bash

CURRENT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPTS_DIR="$CURRENT_DIR/integrations/tmux-plugin/scripts"

get_option() {
  local option="$1"
  local default="$2"
  local value
  value="$(tmux show-option -gqv "$option" 2>/dev/null || true)"
  printf '%s\n' "${value:-$default}"
}

bind_prefix_if_set() {
  local key="$1"
  local command="$2"
  [ -n "$key" ] || return 0
  tmux bind-key "$key" "$command"
}

bind_root_if_set() {
  local key="$1"
  local command="$2"
  [ -n "$key" ] || return 0
  tmux bind-key -n "$key" "$command"
}

DISPATCH_KEY="$(get_option "@swarmux-dispatch-key" "T")"
PANE_SWITCH_KEY="$(get_option "@swarmux-pane-switch-key" "C-M-Space")"
SIDEBAR_KEY="$(get_option "@swarmux-sidebar-key" "M-s")"
OVERVIEW_KEY="$(get_option "@swarmux-overview-key" "F8")"
INDEX_KEYS="$(get_option "@swarmux-index-keys" "M-1 M-2 M-3 M-4 M-5 M-6 M-7 M-8 M-9")"

bind_prefix_if_set "$DISPATCH_KEY" "command-prompt -p \"Task\" \"run-shell -F 'bash \\\"$SCRIPTS_DIR/dispatch.sh\\\" \\\"#{pane_id}\\\" \\\"%1\\\"'\""
bind_root_if_set "$PANE_SWITCH_KEY" "run-shell -F \"bash \\\"$SCRIPTS_DIR/panes-switch.sh\\\" \\\"#{pane_id}\\\"\""
bind_root_if_set "$SIDEBAR_KEY" "run-shell -F \"bash \\\"$SCRIPTS_DIR/sidebar.sh\\\" \\\"#{pane_id}\\\"\""
bind_root_if_set "$OVERVIEW_KEY" "run-shell \"bash \\\"$SCRIPTS_DIR/overview.sh\\\"\""

index=1
for key in $INDEX_KEYS; do
  [ "$index" -le 9 ] || break
  bind_root_if_set "$key" "run-shell \"bash \\\"$SCRIPTS_DIR/jump-index.sh\\\" \\\"$index\\\"\""
  index=$((index + 1))
done
