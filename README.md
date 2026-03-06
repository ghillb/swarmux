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
  "repo": "demo",
  "repo_root": "/path/to/repo",
  "mode": "manual",
  "worktree": "/path/to/repo",
  "session": "swarmux-demo",
  "command": ["bash", "-lc", "echo READY"]
}'
swarmux --output json list
swarmux popup --once
```

## Release model

The repo is wired for:

- semantic PR titles
- `release-plz` release PRs
- GitHub-only releases
- `cargo-dist` archives for Linux/macOS
