# Swarmux setup

## tmux F8 popup

Use this binding:

```tmux
bind -n F8 display-popup -T "Swarmux" -w 90% -h 80% -E "sh -lc 'swarmux popup --once; printf \"\\nPress Enter to close...\"; read _'"
```

## tmux completion notifications

Run a foreground watcher in the background from tmux:

```tmux
bind-key W run-shell -b 'swarmux --output json watch --tmux >/dev/null 2>&1'
```

One-shot completion delivery:

```tmux
bind-key N run-shell -b 'swarmux --output json notify --tmux >/dev/null 2>&1'
```

Reload tmux:

```bash
tmux source-file ~/.config/tmux/tmux.conf
```
