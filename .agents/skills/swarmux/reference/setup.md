# Swarmux setup

## tmux overview popup

You can use binding like this:

```tmux
bind -n F8 display-popup -T "Swarmux" -w 90% -h 80% -E "sh -lc 'swarmux overview --once; printf \"\\nPress Enter to close...\"; read _'"
```

`overview` defaults to `--scope non-terminal`, so the popup shows active tasks first. Use `swarmux overview --once --scope all` or `--scope terminal` when needed.

## tmux pane tree switcher

Use this binding for the pane-first tmux tree view:

```tmux
bind -n C-M-Space run-shell "swarmux panes switch"
```

`swarmux panes switch` refreshes pane metadata and opens the native tmux `choose-tree` popup. The filtered session patterns come from `[tmux].session_ignore` in `~/.config/swarmux/config.toml`; leave it unset to show all sessions.

## tmux task dispatch

Use tmux for the prompt UI and connected dispatch from the current pane:

```tmux
bind-key D command-prompt -p "Task" "run-shell 'swarmux --output json dispatch --connected --pane-id \"#{pane_id}\" --prompt \"%1\" -- codex exec'"
```

To avoid repeating the command prefix, set a default in `$XDG_CONFIG_HOME/swarmux/config.toml`:

```toml
[connected]
runtime = "mirrored"
command = ["codex", "exec"]
```

Then the binding can be:

```tmux
bind-key D command-prompt -p "Task" "run-shell 'swarmux --output json dispatch --connected --pane-id \"#{pane_id}\" --prompt \"%1\"'"
```

For multiple runners, configure named agents:

```toml
[connected]
agent = "codex"
runtime = "mirrored"

[agents.codex]
command = ["codex", "exec"]

[agents.claude]
command = ["claude", "-p"]
```

Then the binding can target a configured agent:

```tmux
bind-key D command-prompt -p "Task" "run-shell 'swarmux --output json dispatch --connected --pane-id \"#{pane_id}\" --agent codex --prompt \"%1\"'"
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

Completion lines include a compact output excerpt:

```text
swarmux 4rh succeeded what is the time currently ...current time is 23:14:05
```

Task logs are timestamped in UTC.

Reload tmux:

```bash
tmux source-file ~/.config/tmux/tmux.conf
```
