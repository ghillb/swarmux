#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$SCRIPT_DIR/common.sh"

bin="$(require_swarmux_bin)"
index="${1:?missing index}"

"$bin" panes jump --index "$index" >/dev/null
