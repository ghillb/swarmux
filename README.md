# swarmux

Agent-first tmux swarm orchestration for local coding tasks.

`swarmux` gives coding agents a narrow control plane for submitting, starting, inspecting, steering, reconciling, and pruning local work. Humans keep tmux visibility; agents get machine-readable commands and strict input validation.

## Requirements

- `tmux`
- `git`
- a POSIX shell at `/bin/sh`
- optional: `bd` when `SWARMUX_BACKEND=beads`

## Install

For now, install from source:

```bash
cargo install --path .
```

If you use the optional beads-rust backend and only have `br` installed, add a `bd` shim.

## Quick start

```bash
swarmux doctor
swarmux init
swarmux --output json schema
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
swarmux popup --once
```

## How it works

`swarmux` stores task state in either `files` (default) or `beads` (`SWARMUX_BACKEND=beads`), but runtime execution is always tmux-driven and command-agnostic. The `command` array from `submit` is executed as-is inside a tmux session.

```mermaid
flowchart TD
    A[Agent or user] --> B[swarmux CLI]
    B --> C[Validate payload and command]
    C --> D{State backend}
    D -->|files| E[Local files store]
    D -->|beads| F[bd adapter]

    C --> G[start or delegate]
    G --> H[runtime::start_task]
    H --> I["tmux new-session + command"]
    I --> L[logs + exit marker]
    L --> M[reconcile updates task state]
    M --> D
```

For completion notifications, use `swarmux notify --tmux` for one-shot delivery or `swarmux watch --tmux` for a foreground polling loop that reconciles and emits `tmux display-message` when tasks enter terminal states.
