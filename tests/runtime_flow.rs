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
        --) shift; log_file="$1"; break ;;
        *) shift ;;
      esac
    done
    mkdir -p "$(dirname "$log_file")"
    printf 'spawned %s\n' "$session" > "$(session_file "$session")"
    printf 'cwd %s\n' "$workdir" >> "$(session_file "$session")"
    printf 'spawned %s\n__SWARMUX_EXIT_CODE__=0\n' "$session" > "$log_file"
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
        C-c) printf '^C\n' >> "$(session_file "$session")"; shift ;;
        *) printf '%s\n' "$1" >> "$(session_file "$session")"; shift ;;
      esac
    done
    ;;
  kill-session)
    if [ "${{1:-}}" = "-t" ]; then
      rm -f "$(session_file "$2")"
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
