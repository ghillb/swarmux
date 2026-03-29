---
layout: default
title: Get Started
description: "Install swarmux, wire tmux, and run the current task workflow."
---

## Requirements

- `tmux`
- `tmux` 3.7 or newer
- `git`
- POSIX shell at `/bin/sh`
- optional: `bd` when using `SWARMUX_BACKEND=beads`

## Install prompt

Paste this into your coding agent:

```bash
bash -lc 'set -euo pipefail
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) target=x86_64-unknown-linux-gnu ;;
  Darwin-arm64) target=aarch64-apple-darwin ;;
  *) echo "unsupported platform" >&2; exit 1 ;;
esac
bin_dir="$(IFS=:; for dir in $PATH; do [ -d "$dir" ] && [ -w "$dir" ] && { printf %s "$dir"; break; }; done)"
: "${bin_dir:=$HOME/.local/bin}"
mkdir -p "$bin_dir" "$HOME/.agents/skills/swarmux"
curl -fsSL "https://github.com/ghillb/swarmux/releases/latest/download/swarmux-${target}.tar.xz" | tar -xJf - -C /tmp
install -m 0755 /tmp/swarmux "$bin_dir/swarmux"
curl -fsSL "https://github.com/ghillb/swarmux/raw/main/.agents/skills/swarmux/SKILL.md" -o "$HOME/.agents/skills/swarmux/SKILL.md"
export PATH="$bin_dir:$PATH"
swarmux --help >/dev/null'
```

This also installs the optional `swarmux` skill under `~/.agents/skills/swarmux` for agents that load global skills.

## Tmux setup

Install TPM if you do not have it yet:

```bash
git clone https://github.com/tmux-plugins/tpm ~/.tmux/plugins/tpm
```

Add this to `~/.config/tmux/tmux.conf`:

```tmux
set -g @plugin 'tmux-plugins/tpm'
set -g @plugin 'ghillb/swarmux'
set -g @swarmux-dispatch-key 'T'
set -g @swarmux-pane-switch-key 'C-M-Space'
set -g @swarmux-sidebar-key 'M-s'
set -g @swarmux-overview-key 'F8'
set -g @swarmux-index-keys 'M-1 M-2 M-3 M-4 M-5 M-6 M-7 M-8 M-9'
# Optional when swarmux is not already on PATH:
# set -g @swarmux-bin '/absolute/path/to/swarmux'

run '~/.tmux/plugins/tpm/tpm'
```

Reload tmux, then install plugins:

```bash
tmux source-file ~/.config/tmux/tmux.conf
~/.tmux/plugins/tpm/bin/install_plugins
```

The plugin only installs bindings. It does not install or upgrade the `swarmux` binary.

Default bindings:

- `prefix + T`: connected dispatch prompt for the current pane
- `C-M-Space`: pane switcher popup
- `M-s`: sidebar
- `F8`: overview popup
- `M-1` through `M-9`: jump to managed panes by index

Set any of the `@swarmux-*-key` values to an empty string if you want to disable that binding.

## First run

```bash
swarmux doctor
swarmux init
swarmux schema
```

Structured commands emit JSON by default. Use `--human` for compact task summaries. TUI commands ignore `--output`.

## Spawn a task

`prefix + T` opens the connected dispatch prompt in tmux.

Equivalent CLI:

```bash
swarmux dispatch --connected --human --prompt "fix tests"
```

For explicit payloads, use `submit`:

```bash
swarmux submit --json '{
  "title": "hello",
  "repo_ref": "demo",
  "repo_root": "/path/to/repo",
  "mode": "manual",
  "worktree": "/path/to/repo",
  "session": "swarmux-demo",
  "command": ["codex","exec","-m","gpt-5.3-codex","echo hi from task"]
}'
```

## Inspect and steer

```bash
swarmux overview --tui
swarmux overview --once
swarmux panes
swarmux panes jump --index 1
```

`overview --tui` has `Tasks` and `Stats`. Inside `Tasks`, `f` cycles `active -> terminal -> all`. `Enter` jumps to the task session. `x` stops an active task. `X` kills it.

`M-1` through `M-9` use `swarmux panes jump --index <n>` under the hood, targeting the first nine managed panes in `swarmux panes` order.

## Runtime choices

- `headless`: default detached runner
- `mirrored`: visible CLI runner with output mirrored to logs
- `tui`: full-screen interactive app in its own tmux session

`tui` tasks still use the task session for execution. Use `swarmux attach <id>` when you want to enter one.

## Optional config

If you want a default connected command or named agents, add `~/.config/swarmux/config.toml`:

```toml
home = "/home/you/.local/state/swarmux"
backend = "files"

[connected]
runtime = "mirrored"
command = ["codex", "exec"]

[agents.claude]
command = ["claude", "-p"]
```

## Task management

These are the main task lifecycle commands:

- `swarmux submit` creates a task record
- `swarmux dispatch` submits and starts in one step
- `swarmux start <id>` launches a queued task
- `swarmux list` shows tasks
- `swarmux show <id>` opens one task
- `swarmux attach <id>` enters the task session

Use these for control and recovery:

- `swarmux wait <id>` blocks until a task reaches a target state
- `swarmux watch <id>` streams task status and logs
- `swarmux logs <id> --raw` prints the stored log
- `swarmux set-ref <id> <url>` links task and PR or issue
- `swarmux reconcile` repairs state after exits or session loss
- `swarmux notify --tmux` sends a tmux message
- `swarmux prune --apply` removes finished managed worktrees and sessions

Task-scoped wait and watch:

```bash
swarmux wait <id> --states succeeded,failed --timeout-ms 600000
swarmux watch <id> --states waiting_input,succeeded,failed,canceled --lines 40
swarmux set-ref <id> "https://github.com/owner/repo/pull/123"
```

`watch`/`notify` show a compact excerpt inline:

```text
swarmux 4rh succeeded what is the time currently ...current time is 23:14:05
```

Task logs are timestamped in UTC:

```text
2026-03-14T10:22:31Z spawned swx-swarmux-4rh
2026-03-14T10:22:35Z current time is 23:14:05
```
