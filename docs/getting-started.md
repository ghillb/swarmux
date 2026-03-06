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
  "repo": "demo",
  "repo_root": "/path/to/repo",
  "mode": "manual",
  "worktree": "/path/to/repo",
  "session": "swarmux-demo",
  "command": ["bash", "-lc", "echo READY"]
}'
swarmux --output json list
swarmux --output json start swx-REPLACE_ME
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

## Operator commands

```bash
swarmux --output json show swx-REPLACE_ME
swarmux --output json logs swx-REPLACE_ME --raw
swarmux --output json reconcile
swarmux --output json prune --apply
```
