use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct Harness {
    home: TempDir,
    bin: TempDir,
    fake_root: TempDir,
}

impl Harness {
    fn new() -> Self {
        let home = TempDir::new().unwrap();
        let bin = TempDir::new().unwrap();
        let fake_root = TempDir::new().unwrap();

        fs::create_dir_all(fake_root.path().join("sessions")).unwrap();
        fs::write(fake_root.path().join("git.log"), "").unwrap();
        fs::create_dir_all(fake_root.path().join("repo").join(".git-fake-branches")).unwrap();
        write_fake_tmux(bin.path().join("tmux"), fake_root.path());
        write_fake_git(bin.path().join("git"), fake_root.path());

        Self {
            home,
            bin,
            fake_root,
        }
    }

    fn run(&self, args: &[&str]) -> assert_cmd::assert::Assert {
        let mut command = Command::new(env!("CARGO_BIN_EXE_swarmux"));
        let path = format!(
            "{}:{}",
            self.bin.path().display(),
            std::env::var("PATH").unwrap()
        );

        command.env("SWARMUX_HOME", self.home.path());
        command.env("SWARMUX_CONFIG_HOME", self.home.path().join("config-home"));
        command.env("SWARMUX_FAKE_TMUX_ROOT", self.fake_root.path());
        command.env("SWARMUX_FAKE_GIT_ROOT", self.fake_root.path());
        command.env(
            "SWARMUX_FAKE_GIT_LOG",
            self.fake_root.path().join("git.log"),
        );
        command.env("PATH", path);
        command.args(args);
        command.assert()
    }

    fn run_in_tmux(&self, args: &[&str]) -> assert_cmd::assert::Assert {
        let mut command = Command::new(env!("CARGO_BIN_EXE_swarmux"));
        let path = format!(
            "{}:{}",
            self.bin.path().display(),
            std::env::var("PATH").unwrap()
        );

        command.env("SWARMUX_HOME", self.home.path());
        command.env("SWARMUX_CONFIG_HOME", self.home.path().join("config-home"));
        command.env("SWARMUX_FAKE_TMUX_ROOT", self.fake_root.path());
        command.env("SWARMUX_FAKE_GIT_ROOT", self.fake_root.path());
        command.env(
            "SWARMUX_FAKE_GIT_LOG",
            self.fake_root.path().join("git.log"),
        );
        command.env("PATH", path);
        command.env("TMUX", "/tmp/fake-tmux,123,0");
        command.args(args);
        command.assert()
    }

    fn run_in_tmux_pane(
        &self,
        pane_id: &str,
        pane_path: &str,
        args: &[&str],
    ) -> assert_cmd::assert::Assert {
        let mut command = Command::new(env!("CARGO_BIN_EXE_swarmux"));
        let path = format!(
            "{}:{}",
            self.bin.path().display(),
            std::env::var("PATH").unwrap()
        );

        command.env("SWARMUX_HOME", self.home.path());
        command.env("SWARMUX_CONFIG_HOME", self.home.path().join("config-home"));
        command.env("SWARMUX_FAKE_TMUX_ROOT", self.fake_root.path());
        command.env("SWARMUX_FAKE_GIT_ROOT", self.fake_root.path());
        command.env(
            "SWARMUX_FAKE_GIT_LOG",
            self.fake_root.path().join("git.log"),
        );
        command.env("PATH", path);
        command.env("TMUX", "/tmp/fake-tmux,123,0");
        command.env("TMUX_PANE", pane_id);
        command.env("SWARMUX_FAKE_TMUX_PANE_PATH", pane_path);
        command.args(args);
        command.assert()
    }
}

#[test]
fn manual_start_send_logs_reconcile_and_stop_work() {
    let harness = Harness::new();
    harness.run(&["init"]).success();

    let payload = format!(
        "{{\"title\":\"Runtime task\",\"repo_ref\":\"core\",\"repo_root\":\"{}\",\"mode\":\"manual\",\"worktree\":\"/tmp/swarmux-runtime\",\"session\":\"swarmux-runtime\",\"command\":[\"echo\",\"runtime\"]}}",
        harness.fake_root.path().join("repo").display()
    );

    let submitted = harness
        .run(&["--output", "json", "submit", "--json", &payload])
        .success()
        .get_output()
        .stdout
        .clone();
    let submitted: Value = serde_json::from_slice(&submitted).unwrap();
    let task_id = submitted["id"].as_str().unwrap().to_owned();

    harness
        .run(&["--output", "json", "start", &task_id])
        .success()
        .stdout(predicate::str::contains("\"state\":\"running\""));

    harness
        .run(&["--output", "json", "send", &task_id, "--input", "run tests"])
        .success();

    harness
        .run(&["--output", "json", "logs", &task_id, "--raw"])
        .success()
        .stdout(predicate::str::contains("T"))
        .stdout(predicate::str::contains("run tests"));

    fs::remove_file(
        harness
            .fake_root
            .path()
            .join("sessions")
            .join("swarmux-runtime.pane"),
    )
    .unwrap();

    harness
        .run(&["--output", "json", "reconcile"])
        .success()
        .stdout(predicate::str::contains("\"updated\":1"));

    harness
        .run(&["--output", "json", "show", &task_id])
        .success()
        .stdout(predicate::str::contains("\"state\":\"succeeded\""));

    let second = format!(
        "{{\"title\":\"Stop task\",\"repo_ref\":\"core\",\"repo_root\":\"{}\",\"mode\":\"manual\",\"worktree\":\"/tmp/swarmux-stop\",\"session\":\"swarmux-stop\",\"command\":[\"echo\",\"stop\"]}}",
        harness.fake_root.path().join("repo").display()
    );
    let submitted = harness
        .run(&["--output", "json", "submit", "--json", &second])
        .success()
        .get_output()
        .stdout
        .clone();
    let submitted: Value = serde_json::from_slice(&submitted).unwrap();
    let stop_id = submitted["id"].as_str().unwrap().to_owned();

    harness
        .run(&["--output", "json", "start", &stop_id])
        .success();
    harness
        .run(&["--output", "json", "stop", &stop_id, "--kill"])
        .success()
        .stdout(predicate::str::contains("\"state\":\"canceled\""));
}

#[test]
fn delegate_auto_mode_creates_worktree_and_prune_removes_it() {
    let harness = Harness::new();
    harness.run(&["init"]).success();

    let payload = format!(
        "{{\"title\":\"Auto task\",\"repo_ref\":\"core\",\"repo_root\":\"{}\",\"mode\":\"auto\",\"command\":[\"echo\",\"auto\"]}}",
        harness.fake_root.path().join("repo").display()
    );

    let delegated = harness
        .run(&["--output", "json", "delegate", "--json", &payload])
        .success()
        .get_output()
        .stdout
        .clone();
    let delegated: Value = serde_json::from_slice(&delegated).unwrap();
    let started = &delegated["started"];
    let task_id = started["id"].as_str().unwrap().to_owned();
    let session = started["session"].as_str().unwrap().to_owned();
    let worktree = started["worktree"].as_str().unwrap().to_owned();
    let branch = started["branch"].as_str().unwrap().to_owned();

    assert!(session.starts_with("swx-core-"));
    assert!(branch.starts_with("swx/"));
    assert!(worktree.contains(&task_id));

    let git_log = fs::read_to_string(harness.fake_root.path().join("git.log")).unwrap();
    assert!(git_log.contains("worktree add -B"));

    harness
        .run(&[
            "--output",
            "json",
            "done",
            &task_id,
            "--reason",
            "manual_done",
        ])
        .success();

    harness
        .run(&["--output", "json", "prune", "--apply"])
        .success()
        .stdout(predicate::str::contains("\"worktree_removed\":1"));

    let git_log = fs::read_to_string(harness.fake_root.path().join("git.log")).unwrap();
    assert!(git_log.contains("worktree remove -f"));
    assert!(git_log.contains("branch -D"));
}

#[test]
fn dispatch_auto_mode_creates_worktree_from_flags() {
    let harness = Harness::new();
    harness.run(&["init"]).success();

    let dispatched = harness
        .run(&[
            "--output",
            "json",
            "dispatch",
            "--title",
            "Dispatch task",
            "--repo-ref",
            "core",
            "--repo-root",
            &harness.fake_root.path().join("repo").display().to_string(),
            "--",
            "echo",
            "dispatch",
        ])
        .success()
        .get_output()
        .stdout
        .clone();
    let dispatched: Value = serde_json::from_slice(&dispatched).unwrap();
    let started = &dispatched["started"];

    assert_eq!(started["title"], "Dispatch task");
    assert_eq!(started["state"], "running");
    assert!(
        started["session"]
            .as_str()
            .unwrap()
            .starts_with("swx-core-")
    );

    let git_log = fs::read_to_string(harness.fake_root.path().join("git.log")).unwrap();
    assert!(git_log.contains("worktree add -B"));
}

#[test]
fn dispatch_defaults_title_from_command_when_omitted() {
    let harness = Harness::new();
    harness.run(&["init"]).success();

    let dispatched = harness
        .run(&[
            "--output",
            "json",
            "dispatch",
            "--repo-ref",
            "core",
            "--repo-root",
            &harness.fake_root.path().join("repo").display().to_string(),
            "--",
            "codex",
            "exec",
            "fix tests",
        ])
        .success()
        .get_output()
        .stdout
        .clone();
    let dispatched: Value = serde_json::from_slice(&dispatched).unwrap();
    let started = &dispatched["started"];

    assert_eq!(started["title"], "codex exec fix tests");
    assert_eq!(started["state"], "running");
}

#[test]
fn dispatch_connected_infers_repo_and_origin_from_tmux_pane() {
    let harness = Harness::new();
    harness.run(&["init"]).success();

    let repo_root = harness.fake_root.path().join("repo");
    let pane_path = repo_root.display().to_string();
    let dispatched = harness
        .run_in_tmux_pane(
            "%42",
            &pane_path,
            &[
                "--output",
                "json",
                "dispatch",
                "--connected",
                "--prompt",
                "fix tests",
                "--",
                "codex",
                "exec",
            ],
        )
        .success()
        .get_output()
        .stdout
        .clone();
    let dispatched: Value = serde_json::from_slice(&dispatched).unwrap();
    let started = &dispatched["started"];

    assert_eq!(started["title"], "fix tests");
    assert_eq!(started["repo_root"], repo_root.display().to_string());
    assert_eq!(started["repo"], "repo");
    assert_eq!(started["mode"], "auto");
    assert_eq!(started["runtime"], "headless");
    assert_eq!(started["command"][0], "codex");
    assert_eq!(started["command"][1], "exec");
    assert_eq!(started["command"][2], "fix tests");
    assert_eq!(started["origin"]["pane_id"], "%42");
    assert_eq!(started["origin"]["pane_current_path"], pane_path);
    assert_eq!(started["origin"]["session_name"], "origin-session");
    assert_eq!(started["origin"]["window_name"], "origin-window");
}

#[test]
fn dispatch_connected_mirrored_flag_sets_runtime() {
    let harness = Harness::new();
    harness.run(&["init"]).success();

    let repo_root = harness.fake_root.path().join("repo");
    let pane_path = repo_root.display().to_string();
    let dispatched = harness
        .run_in_tmux_pane(
            "%52",
            &pane_path,
            &[
                "--output",
                "json",
                "dispatch",
                "--connected",
                "--mirrored",
                "--prompt",
                "inspect mirrored",
                "--",
                "codex",
                "exec",
            ],
        )
        .success()
        .get_output()
        .stdout
        .clone();
    let dispatched: Value = serde_json::from_slice(&dispatched).unwrap();
    let started = &dispatched["started"];

    assert_eq!(started["runtime"], "mirrored");
    assert_eq!(started["state"], "running");
}

#[test]
fn dispatch_connected_uses_configured_default_command() {
    let harness = Harness::new();
    fs::create_dir_all(harness.home.path().join("config-home").join("swarmux")).unwrap();
    fs::write(
        harness
            .home
            .path()
            .join("config-home")
            .join("swarmux")
            .join("config.toml"),
        "[connected]\ncommand = [\"claude\", \"-p\"]\n",
    )
    .unwrap();
    harness.run(&["init"]).success();

    let repo_root = harness.fake_root.path().join("repo");
    let pane_path = repo_root.display().to_string();
    let dispatched = harness
        .run_in_tmux_pane(
            "%43",
            &pane_path,
            &[
                "--output",
                "json",
                "dispatch",
                "--connected",
                "--prompt",
                "summarize repo",
            ],
        )
        .success()
        .get_output()
        .stdout
        .clone();
    let dispatched: Value = serde_json::from_slice(&dispatched).unwrap();
    let started = &dispatched["started"];

    assert_eq!(started["command"][0], "claude");
    assert_eq!(started["command"][1], "-p");
    assert_eq!(started["command"][2], "summarize repo");
}

#[test]
fn dispatch_connected_uses_named_agent_from_config() {
    let harness = Harness::new();
    fs::create_dir_all(harness.home.path().join("config-home").join("swarmux")).unwrap();
    fs::write(
        harness
            .home
            .path()
            .join("config-home")
            .join("swarmux")
            .join("config.toml"),
        "[agents.codex]\ncommand = [\"codex\", \"exec\"]\n",
    )
    .unwrap();
    harness.run(&["init"]).success();

    let repo_root = harness.fake_root.path().join("repo");
    let pane_path = repo_root.display().to_string();
    let dispatched = harness
        .run_in_tmux_pane(
            "%44",
            &pane_path,
            &[
                "--output",
                "json",
                "dispatch",
                "--connected",
                "--agent",
                "codex",
                "--prompt",
                "fix lint",
            ],
        )
        .success()
        .get_output()
        .stdout
        .clone();
    let dispatched: Value = serde_json::from_slice(&dispatched).unwrap();
    let started = &dispatched["started"];

    assert_eq!(started["command"][0], "codex");
    assert_eq!(started["command"][1], "exec");
    assert_eq!(started["command"][2], "fix lint");
}

#[test]
fn dispatch_connected_uses_default_agent_from_config() {
    let harness = Harness::new();
    fs::create_dir_all(harness.home.path().join("config-home").join("swarmux")).unwrap();
    fs::write(
        harness
            .home
            .path()
            .join("config-home")
            .join("swarmux")
            .join("config.toml"),
        "[connected]\nagent = \"claude\"\n\n[agents.claude]\ncommand = [\"claude\", \"-p\"]\n",
    )
    .unwrap();
    harness.run(&["init"]).success();

    let repo_root = harness.fake_root.path().join("repo");
    let pane_path = repo_root.display().to_string();
    let dispatched = harness
        .run_in_tmux_pane(
            "%45",
            &pane_path,
            &[
                "--output",
                "json",
                "dispatch",
                "--connected",
                "--prompt",
                "summarize diff",
            ],
        )
        .success()
        .get_output()
        .stdout
        .clone();
    let dispatched: Value = serde_json::from_slice(&dispatched).unwrap();
    let started = &dispatched["started"];

    assert_eq!(started["command"][0], "claude");
    assert_eq!(started["command"][1], "-p");
    assert_eq!(started["command"][2], "summarize diff");
}

#[test]
fn dispatch_connected_uses_configured_default_runtime() {
    let harness = Harness::new();
    fs::create_dir_all(harness.home.path().join("config-home").join("swarmux")).unwrap();
    fs::write(
        harness
            .home
            .path()
            .join("config-home")
            .join("swarmux")
            .join("config.toml"),
        "[connected]\nruntime = \"mirrored\"\ncommand = [\"codex\", \"exec\"]\n",
    )
    .unwrap();
    harness.run(&["init"]).success();

    let repo_root = harness.fake_root.path().join("repo");
    let pane_path = repo_root.display().to_string();
    let dispatched = harness
        .run_in_tmux_pane(
            "%53",
            &pane_path,
            &[
                "--output",
                "json",
                "dispatch",
                "--connected",
                "--prompt",
                "configured mirrored",
            ],
        )
        .success()
        .get_output()
        .stdout
        .clone();
    let dispatched: Value = serde_json::from_slice(&dispatched).unwrap();
    let started = &dispatched["started"];

    assert_eq!(started["runtime"], "mirrored");
    assert_eq!(started["command"][0], "codex");
}

#[test]
fn notify_reports_terminal_tasks_once_and_can_emit_tmux_messages() {
    let harness = Harness::new();
    harness.run(&["init"]).success();

    let payload = format!(
        "{{\"title\":\"Notify task\",\"repo_ref\":\"core\",\"repo_root\":\"{}\",\"mode\":\"manual\",\"worktree\":\"/tmp/swarmux-notify\",\"session\":\"swarmux-notify\",\"command\":[\"echo\",\"notify\"]}}",
        harness.fake_root.path().join("repo").display()
    );

    let submitted = harness
        .run(&["--output", "json", "submit", "--json", &payload])
        .success()
        .get_output()
        .stdout
        .clone();
    let submitted: Value = serde_json::from_slice(&submitted).unwrap();
    let task_id = submitted["id"].as_str().unwrap().to_owned();

    harness
        .run(&["--output", "json", "start", &task_id])
        .success()
        .stdout(predicate::str::contains("\"state\":\"running\""));

    fs::remove_file(
        harness
            .fake_root
            .path()
            .join("sessions")
            .join("swarmux-notify.pane"),
    )
    .unwrap();

    let notified = harness
        .run_in_tmux(&["--output", "json", "notify", "--tmux"])
        .success()
        .get_output()
        .stdout
        .clone();
    let notified: Value = serde_json::from_slice(&notified).unwrap();
    assert_eq!(notified["reconciled"]["updated"], 1);
    assert_eq!(notified["count"], 1);
    assert_eq!(notified["notifications"][0]["id"], task_id);
    assert_eq!(notified["notifications"][0]["state"], "succeeded");
    assert!(
        notified["notifications"][0]["output_excerpt"]
            .as_str()
            .unwrap()
            .starts_with("...")
    );

    let display_log = fs::read_to_string(harness.fake_root.path().join("display.log")).unwrap();
    assert!(display_log.contains("swarmux"));
    assert!(display_log.contains(&task_id));
    assert!(display_log.contains("Notify task"));

    harness
        .run_in_tmux(&["--output", "json", "notify", "--tmux"])
        .success()
        .stdout(predicate::str::contains("\"count\":0"));

    let display_log = fs::read_to_string(harness.fake_root.path().join("display.log")).unwrap();
    assert_eq!(display_log.lines().count(), 1);
}

#[test]
fn watch_can_emit_tmux_messages_and_exit_after_max_iterations() {
    let harness = Harness::new();
    harness.run(&["init"]).success();

    let payload = format!(
        "{{\"title\":\"Watch task\",\"repo_ref\":\"core\",\"repo_root\":\"{}\",\"mode\":\"manual\",\"worktree\":\"/tmp/swarmux-watch\",\"session\":\"swarmux-watch\",\"command\":[\"echo\",\"watch\"]}}",
        harness.fake_root.path().join("repo").display()
    );

    let submitted = harness
        .run(&["--output", "json", "submit", "--json", &payload])
        .success()
        .get_output()
        .stdout
        .clone();
    let submitted: Value = serde_json::from_slice(&submitted).unwrap();
    let task_id = submitted["id"].as_str().unwrap().to_owned();

    harness
        .run(&["--output", "json", "start", &task_id])
        .success()
        .stdout(predicate::str::contains("\"state\":\"running\""));

    fs::remove_file(
        harness
            .fake_root
            .path()
            .join("sessions")
            .join("swarmux-watch.pane"),
    )
    .unwrap();

    let watched = harness
        .run_in_tmux(&[
            "--output",
            "json",
            "watch",
            "--tmux",
            "--interval-ms",
            "1",
            "--max-iterations",
            "1",
        ])
        .success()
        .get_output()
        .stdout
        .clone();
    let watched: Value = serde_json::from_slice(&watched).unwrap();
    assert_eq!(watched["reconciled"]["updated"], 1);
    assert_eq!(watched["count"], 1);
    assert_eq!(watched["notifications"][0]["id"], task_id);

    let display_log = fs::read_to_string(harness.fake_root.path().join("display.log")).unwrap();
    assert!(display_log.contains("Watch task"));
}

#[test]
fn watch_text_mode_can_print_output_excerpt_for_completed_tasks() {
    let harness = Harness::new();
    harness.run(&["init"]).success();

    let payload = format!(
        "{{\"title\":\"Watch tail task\",\"repo_ref\":\"core\",\"repo_root\":\"{}\",\"mode\":\"manual\",\"worktree\":\"/tmp/swarmux-watch-tail\",\"session\":\"swarmux-watch-tail\",\"command\":[\"echo\",\"watch-tail\"]}}",
        harness.fake_root.path().join("repo").display()
    );

    let submitted = harness
        .run(&["--output", "json", "submit", "--json", &payload])
        .success()
        .get_output()
        .stdout
        .clone();
    let submitted: Value = serde_json::from_slice(&submitted).unwrap();
    let task_id = submitted["id"].as_str().unwrap().to_owned();

    harness
        .run(&["--output", "json", "start", &task_id])
        .success()
        .stdout(predicate::str::contains("\"state\":\"running\""));

    fs::remove_file(
        harness
            .fake_root
            .path()
            .join("sessions")
            .join("swarmux-watch-tail.pane"),
    )
    .unwrap();

    harness
        .run(&["watch", "--interval-ms", "1", "--max-iterations", "1"])
        .success()
        .stdout(predicate::str::contains("time"))
        .stdout(predicate::str::contains("Watch tail task"))
        .stdout(predicate::str::contains("..."));
}

#[test]
fn watch_text_mode_can_show_optional_tokens_column() {
    let harness = Harness::new();
    harness.run(&["init"]).success();

    let payload = format!(
        "{{\"title\":\"Watch tokens task\",\"repo_ref\":\"core\",\"repo_root\":\"{}\",\"mode\":\"manual\",\"worktree\":\"/tmp/swarmux-watch-tokens\",\"session\":\"swarmux-watch-tokens\",\"command\":[\"echo\",\"watch-tokens\"]}}",
        harness.fake_root.path().join("repo").display()
    );

    let submitted = harness
        .run(&["--output", "json", "submit", "--json", &payload])
        .success()
        .get_output()
        .stdout
        .clone();
    let submitted: Value = serde_json::from_slice(&submitted).unwrap();
    let task_id = submitted["id"].as_str().unwrap().to_owned();

    harness
        .run(&["--output", "json", "start", &task_id])
        .success()
        .stdout(predicate::str::contains("\"state\":\"running\""));

    fs::remove_file(
        harness
            .fake_root
            .path()
            .join("sessions")
            .join("swarmux-watch-tokens.pane"),
    )
    .unwrap();

    harness
        .run(&[
            "watch",
            "--interval-ms",
            "1",
            "--max-iterations",
            "1",
            "--show-tokens",
        ])
        .success()
        .stdout(predicate::str::contains("tokens"))
        .stdout(predicate::str::contains(&task_id));
}

fn write_fake_tmux(path: PathBuf, root: &Path) {
    let script = format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
root="{root}"
sessions="$root/sessions"
cmd="${{1:-}}"
shift || true

session_file() {{
  printf '%s/%s.pane\n' "$sessions" "$1"
}}

session_log_file() {{
  printf '%s/%s.logfile\n' "$sessions" "$1"
}}

case "$cmd" in
  has-session)
    if [ "${{1:-}}" = "-t" ] && [ -f "$(session_file "$2")" ]; then
      exit 0
    fi
    exit 1
    ;;
  new-session)
    session=""
    workdir=""
    log_file=""
    while [ "$#" -gt 0 ]; do
      case "$1" in
        -s) session="$2"; shift 2 ;;
        -c) workdir="$2"; shift 2 ;;
        -d) shift ;;
        /bin/sh) shift 2; log_file="$1"; break ;;
        *) shift ;;
      esac
    done
    printf 'spawned %s\n' "$session" > "$(session_file "$session")"
    printf 'cwd %s\n' "$workdir" >> "$(session_file "$session")"
    if [ -n "$log_file" ]; then
      mkdir -p "$(dirname "$log_file")"
      printf '%s\n' "$log_file" > "$(session_log_file "$session")"
      printf '2026-03-13T22:00:00Z spawned %s\n' "$session" > "$log_file"
      printf '2026-03-13T22:00:01Z __SWARMUX_EXIT_CODE__=0\n' >> "$log_file"
    fi
    ;;
  pipe-pane)
    while [ "$#" -gt 0 ]; do
      case "$1" in
        -t) session="$2"; shift 2 ;;
        -o) shift ;;
        *)
          log_file="$(printf '%s' "$1" | sed -n "s/.*swarmux-pipe '\\([^']*\\)'.*/\\1/p")"
          if [ -n "$log_file" ]; then
            printf '%s\n' "$log_file" > "$(session_log_file "$session")"
          fi
          shift
          ;;
      esac
    done
    ;;
  respawn-pane)
    session=""
    log_file=""
    while [ "$#" -gt 0 ]; do
      case "$1" in
        -t) session="$2"; shift 2 ;;
        -k) shift ;;
        /bin/sh) shift 2; log_file="$1"; break ;;
        *) shift ;;
      esac
    done
    if [ -n "$log_file" ]; then
      printf '%s\n' "$log_file" > "$(session_log_file "$session")"
      printf '2026-03-13T22:00:02Z codex mirrored started\n' >> "$log_file"
      printf '2026-03-13T22:00:03Z __SWARMUX_EXIT_CODE__=0\n' >> "$log_file"
    fi
    ;;
  capture-pane)
    while [ "$#" -gt 0 ]; do
      case "$1" in
        -t) session="$2"; shift 2 ;;
        *) shift ;;
      esac
    done
    cat "$(session_file "$session")"
    ;;
  send-keys)
    while [ "$#" -gt 0 ]; do
      case "$1" in
        -t) session="$2"; shift 2 ;;
        C-m) shift ;;
        C-c)
          printf '^C\n' >> "$(session_file "$session")"
          if [ -f "$(session_log_file "$session")" ]; then
            printf '2026-03-13T22:00:02Z ^C\n' >> "$(cat "$(session_log_file "$session")")"
          fi
          shift
          ;;
        *)
          printf '%s\n' "$1" >> "$(session_file "$session")"
          if [ -f "$(session_log_file "$session")" ]; then
            printf '2026-03-13T22:00:02Z > %s\n' "$1" >> "$(cat "$(session_log_file "$session")")"
          fi
          shift
          ;;
      esac
    done
    ;;
  kill-session)
    if [ "${{1:-}}" = "-t" ]; then
      rm -f "$(session_file "$2")"
      rm -f "$(session_log_file "$2")"
      exit 0
    fi
    exit 1
    ;;
  attach-session)
    if [ "${{1:-}}" = "-t" ]; then
      cat "$(session_file "$2")"
      exit 0
    fi
    exit 1
    ;;
  display-message)
    if [ "${{1:-}}" = "-p" ] && [ "${{2:-}}" = "-t" ]; then
      target="$3"
      format="${{4:-}}"
      pane_path="${{SWARMUX_FAKE_TMUX_PANE_PATH:-$root/repo}}"
      case "$format" in
        '#{{session_name}}') printf 'origin-session\n' ;;
        '#{{window_id}}') printf '@9\n' ;;
        '#{{window_name}}') printf 'origin-window\n' ;;
        '#{{pane_current_path}}') printf '%s\n' "$pane_path" ;;
        *) echo "unexpected tmux format: $format" >&2; exit 1 ;;
      esac
      exit 0
    fi
    printf '%s\n' "${{1:-}}" >> "$root/display.log"
    ;;
  *)
    echo "unexpected tmux command: $cmd" >&2
    exit 1
    ;;
esac
"#,
        root = root.display()
    );

    fs::write(&path, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
    }
}

fn write_fake_git(path: PathBuf, root: &Path) {
    let script = format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
root="{root}"
log="$root/git.log"
printf '%s\n' "$*" >> "$log"

repo_root=""
if [ "${{1:-}}" = "-C" ]; then
  repo_root="$2"
  shift 2
fi

branch_file() {{
  printf '%s/.git-fake-branches/%s\n' "$repo_root" "$1"
}}

case "${{1:-}}" in
  worktree)
    case "${{2:-}}" in
      add)
        branch="$4"
        worktree="$5"
        mkdir -p "$worktree"
        mkdir -p "$(dirname "$(branch_file "$branch")")"
        touch "$(branch_file "$branch")"
        ;;
      remove)
        worktree="$4"
        rm -rf "$worktree"
        ;;
    esac
    ;;
  show-ref)
    ref="${{4:-}}"
    branch="${{ref#refs/heads/}}"
    if [ -f "$(branch_file "$branch")" ]; then
      exit 0
    fi
    exit 1
    ;;
  branch)
    if [ "${{2:-}}" = "-D" ]; then
      rm -f "$(branch_file "$3")"
    fi
    ;;
  rev-parse)
    printf '%s\n' "$repo_root"
    ;;
  *)
    ;;
esac
"#,
        root = root.display()
    );

    fs::write(&path, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
    }
}
