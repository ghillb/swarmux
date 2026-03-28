---
layout: default
title: swarmux
description: "tmux-first local swarm orchestration for coding agents."
hide_title: true
---

<div class="hero">
  <div>
    <p class="eyebrow">tmux-first local control plane</p>
    <h1 class="hero-title">swarmux</h1>
    <p class="lead">
      Agent-native Rust CLI for tmux-based orchestration of local coding tasks.
      Tasks run in local tmux sessions with deterministic task state and direct
      human access and inspectability.
    </p>
    <div class="chips">
      <span class="chip">tmux visibility</span>
      <span class="chip">pane tree</span>
      <span class="chip">task states</span>
      <span class="chip">files/beads backends</span>
      <span class="chip">reconcile + prune</span>
    </div>
  </div>
  <div class="terminal">
    <div class="term-header">
      <div class="term-dots"><span></span><span></span><span></span></div>
      <span class="term-label">swarmux local operator view</span>
    </div>
    <pre><code>$ swarmux doctor
$ swarmux init
$ swarmux submit --json '{...}'
$ swarmux panes
$ swarmux start &lt;id&gt;
$ swarmux overview --tui</code></pre>
  </div>
</div>

<div class="screenshots">
  <img src="{{ '/assets/screenshots/overview.png' | relative_url }}" alt="swarmux overview tasks screenshot" class="shot shot-wide">
</div>

## Why swarmux

- swarmux extends tmux instead of introducing a separate orchestration runtime.
- Run each task in its own tmux session and git worktree, with direct operator visibility.
- Keep agent automation scriptable via JSON output by default.
- Add task-aware TUIs and agent-agnostic task management around the existing tmux workflow.
- Prune managed worktrees and sessions after terminal states.

## Next step

Read <a href="{{ '/getting-started.html' | relative_url }}">Get Started</a> for setup, tmux mapping, and first task flow.
