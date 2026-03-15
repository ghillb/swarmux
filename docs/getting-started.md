---
layout: default
title: Get Started
description: "Install and run swarmux with tmux overview supervision."
---

## Requirements

- `tmux`
- `git`
- POSIX shell at `/bin/sh`
- optional: `bd` when using `SWARMUX_BACKEND=beads`

## Install

```bash
cargo install --path .
```

If using the optional beads backend, ensure `bd` is available on `PATH`.

If your agent runtime loads global skills from `~/.agents/skills`, place the
official `swarmux` skill there:

```bash
mkdir -p ~/.agents/skills/swarmux
# download or copy .agents/skills/swarmux/SKILL.md into:
~/.agents/skills/swarmux/SKILL.md
```

If your runtime uses a different global skills directory, place the same
directory there instead.

## Initialize and inspect schema

```bash
swarmux doctor
swarmux init
swarmux --output json schema
```

## Submit and start a task

```bash
swarmux --output json submit --json '{
  "title": "hello",
  "repo_ref": "demo",
  "repo_root": "/path/to/repo",
  "mode": "manual",
  "worktree": "/path/to/repo",
  "session": "swarmux-demo",
  "command": ["codex","exec","-m","gpt-5.3-codex","echo hi from task"]
}'
swarmux --output json list
swarmux --output json start <id>
```

## Dispatch a task from tmux-friendly flags

```bash
swarmux --output json dispatch \
  --title "hello" \
  --repo-ref demo \
  --repo-root /path/to/repo \
  -- codex exec -m gpt-5.3-codex "echo hi from task"
```

## Connected dispatch from the current tmux pane

```bash
swarmux --output json dispatch \
  --connected \
  --mirrored \
  --prompt "fix tests" \
  -- codex exec
```

To make the command prefix optional, add a config file:

```toml
# $XDG_CONFIG_HOME/swarmux/config.toml
home = "/home/you/.local/state/swarmux"
backend = "files" # or "beads"

[connected]
runtime = "mirrored"
command = ["codex", "exec"]
```

Then connected dispatch can omit the command prefix:

```bash
swarmux --output json dispatch --connected --prompt "fix tests"
```

Manual TUI task:

```bash
swarmux --output json submit --json '{
  "title": "tui task",
  "repo_ref": "demo",
  "repo_root": "/path/to/repo",
  "mode": "manual",
  "runtime": "tui",
  "worktree": "/path/to/repo",
  "session": "swarmux-demo-tui",
  "command": ["my-tui-agent", "fix tests"]
}'
swarmux --output json start <id>
swarmux attach <id>
```

You can also configure named agent runners:

```toml
# $XDG_CONFIG_HOME/swarmux/config.toml
[connected]
agent = "codex"
runtime = "mirrored"

[agents.codex]
command = ["codex", "exec"]

[agents.claude]
command = ["claude", "-p"]
```

Then dispatch can target a specific configured agent:

```bash
swarmux --output json dispatch --connected --agent claude --prompt "summarize diff"
```

Runtime choices:

```text
headless  logs-first detached runner
mirrored  visible task session with pane output mirrored into logs
tui       full-screen interactive app in its own tmux session
```

`headless` stays the default when no runtime override is configured.
`mirrored` is for visible non-TUI CLI runners.
`tui` is for full-screen interactive programs started in detached tmux task sessions; use `swarmux attach <id>` when you want to enter them.

Connected dispatch still appends `--prompt` as the trailing command argument for every runtime, including `tui`. Use `tui` there only with commands that support that calling convention.

## tmux popup mapping

Use this mapping to open a snapshot popup and keep it open until Enter:

```tmux
bind -n <key> display-popup -T "Swarmux" -w 90% -h 80% -E "sh -lc 'swarmux overview --once; printf \"\\nPress Enter to close...\"; read _'"
```

Reload tmux:

```bash
tmux source-file ~/.config/tmux/tmux.conf
```

`overview` filters rendered rows with `--scope terminal|non-terminal|all`. The default is `non-terminal`.

Use tmux itself for the prompt UI and keep `swarmux` non-interactive:

```tmux
bind-key D command-prompt -p "Task" "run-shell 'swarmux --output json dispatch --connected --pane-id \"#{pane_id}\" --agent codex --prompt \"%1\"'"
bind-key N run-shell -b 'swarmux --output json notify --tmux >/dev/null 2>&1'
```

Keep popup/window presentation in tmux itself. `swarmux` starts and attaches sessions, but it does not manage popup or window layout for TUI tasks.

Task-scoped wait and watch:

```bash
swarmux --output json wait <id> --states succeeded,failed --timeout-ms 600000
swarmux --output json watch <id> --states waiting_input,succeeded,failed,canceled --lines 40
swarmux --output json set-ref <id> "https://github.com/owner/repo/pull/123"
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

## Operator commands

```bash
swarmux --output json show <id>
swarmux --output json logs <id> --raw
swarmux --output json wait <id> --states succeeded,failed
swarmux --output json watch <id> --lines 40
swarmux --output json set-ref <id> "https://github.com/owner/repo/pull/123"
swarmux --output json reconcile
swarmux --output json notify --tmux
swarmux --output json prune --apply
```
