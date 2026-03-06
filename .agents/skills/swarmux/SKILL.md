---
name: swarmux
description: Use this skill when you need tmux-first local task orchestration for coding agents - submit/start/delegate tasks, inspect logs/status, send input, reconcile crashed sessions, and prune finished auto worktrees. Prefer for multi-task local runs requiring machine-readable control and operator tmux visibility.
---

# Swarmux

Use `swarmux` as the control plane for local coding tasks.

Setup reference: `reference/setup.md`

## When to use

- User asks to run multiple coding tasks locally with tmux visibility.
- User wants an agent control plane with JSON outputs (`submit`, `start`, `logs`, `show`, `list`).
- User needs lifecycle control (`send`, `stop`, `done`, `fail`, `reconcile`, `prune`).
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
4. Submit tasks with raw JSON payloads.
5. Use `start` or `delegate` to launch work.
6. Use `logs`, `show`, `list`, `popup`, and `reconcile` for supervision.
7. Use `stop`, `done`, `fail`, and `prune` for explicit control.

## Safety

- Inputs are validated defensively because agents hallucinate paths and identifiers.
- `logs` is sanitized by default; use `--raw` only when needed.
- `prune` is dry-run by default. Add `--apply` only when intentional.
- The beads backend is optional. Default backend is `files`.
