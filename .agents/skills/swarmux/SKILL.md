---
name: swarmux
description: Use this skill when you need tmux-first local task orchestration for coding agents - task submission, start, inspect, attach, wait, watch, reconcile, stop, and prune. Prefer for multi-task local runs requiring machine-readable control and operator tmux visibility.
---

# Swarmux

Use `swarmux` as the control plane for local coding tasks.

Setup reference: [`docs/getting-started.md`](docs/getting-started.md)

## Primary interface

Task management is the main interface.

- `submit` creates a task record
- `dispatch` submits and starts in one step
- `start` launches a queued task
- `list` shows tasks
- `show` opens one task
- `attach` enters a task session
- `wait` blocks until a task reaches target states
- `watch` streams task status and logs
- `logs` prints stored logs
- `set-ref` links tasks to PRs or issues
- `reconcile` repairs crashed or lost sessions
- `stop`, `done`, and `fail` change task lifecycle state
- `prune` cleans up finished work

## When to use

- User asks to run multiple coding tasks locally with tmux visibility.
- User wants an agent control plane with JSON outputs for task lifecycle commands.
- User needs lifecycle control for running tasks, task recovery, or cleanup.
- User needs backend-backed task state (`files` default, optional `beads` via `bd`).
- User wants inspectable local execution instead of hidden/background orchestration.

## Agent command model

- Runtime command is provided by task payload `command` and executed as-is in tmux.
- Do not assume Codex-only runtime; Codex is a common example, not a hard requirement.

## Invariants

- Prefer `--output json` for all machine consumption.
- Prefer raw payload input with `--json` or `--json-file` for mutating commands.
- Use `--dry-run` before real mutations when validating a payload or workflow.
- Treat `schema` as the source of truth for command shape.
- Never bypass `swarmux` with ad hoc tmux or git worktree commands unless you are repairing a broken session.

## Workflow

1. Run `swarmux doctor`.
2. Run `swarmux init`.
3. Inspect command shapes with `swarmux --output json schema`.
4. Submit or dispatch tasks with raw JSON payloads or tmux-friendly flags.
5. Use `list`, `show`, `attach`, `wait`, and `watch` for supervision.
6. Use `stop`, `done`, `fail`, `set-ref`, `reconcile`, and `prune` for explicit control.

## Safety

- Inputs are validated defensively because agents hallucinate paths and identifiers.
- `logs` is sanitized by default; use `--raw` only when needed.
- `prune` is dry-run by default. Add `--apply` only when intentional.
- The beads backend is optional. Default backend is `files`.
