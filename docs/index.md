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
      Run coding tasks in local tmux sessions with deterministic task state and
      operator visibility. Agents get a narrow CLI. Humans keep direct pane access.
    </p>
    <div class="chips">
      <span class="chip">tmux visibility</span>
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
$ swarmux --output json submit --json '{...}'
$ swarmux --output json start swx-...
$ swarmux popup --once</code></pre>
  </div>
</div>

## Why swarmux

- Run task commands in tmux sessions that operators can inspect live.
- Keep agent automation scriptable via machine-readable output.
- Reconcile task state after process exit or session loss.
- Prune managed worktrees and sessions after terminal states.

## Next step

Read <a href="{{ '/getting-started.html' | relative_url }}">Get Started</a> for setup, tmux mapping, and first task flow.
