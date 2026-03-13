---
layout: default
title: Get Started
description: "Install and run swarmux with tmux popup supervision."
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
  --prompt "fix tests" \
  -- codex exec
```

## tmux popup mapping

Use this mapping to open a snapshot popup and keep it open until Enter:

```tmux
bind -n <key> display-popup -T "Swarmux" -w 90% -h 80% -E "sh -lc 'swarmux popup --once; printf \"\\nPress Enter to close...\"; read _'"
```

Reload tmux:

```bash
tmux source-file ~/.config/tmux/tmux.conf
```

## tmux delegation and notifications

Use tmux itself for the prompt UI and keep `swarmux` non-interactive:

```tmux
bind-key D command-prompt -p "Task" "run-shell 'swarmux --output json dispatch --connected --prompt \"%1\" -- codex exec'"
bind-key W run-shell -b 'swarmux --output json watch --tmux >/dev/null 2>&1'
bind-key N run-shell -b 'swarmux --output json notify --tmux >/dev/null 2>&1'
```

## Operator commands

```bash
swarmux --output json show <id>
swarmux --output json logs <id> --raw
swarmux --output json reconcile
swarmux --output json notify --tmux
swarmux --output json watch --tmux
swarmux --output json prune --apply
```
