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

        fs::write(fake_root.path().join("panes.tsv"), "").unwrap();
        fs::write(fake_root.path().join("git-status.txt"), "").unwrap();
        fs::write(fake_root.path().join("git-diff.txt"), "").unwrap();
        fs::write(fake_root.path().join("git-cached.txt"), "").unwrap();
        fs::write(fake_root.path().join("git-branch.txt"), "main\n").unwrap();
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
        command.env("SWARMUX_FAKE_GIT_ROOT", self.fake_root.path());
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
        command.env("SWARMUX_FAKE_GIT_ROOT", self.fake_root.path());
        command.env("PATH", path);
        command.env("TMUX", "/tmp/fake-tmux,123,0");
        command.args(args);
        command.assert()
    }
}

#[test]
fn panes_jump_focuses_managed_pane_by_index() {
    let harness = Harness::new();
    harness.run(&["init"]).success();

    let payload_one = format!(
        "{{\"title\":\"First pane\",\"repo_ref\":\"core\",\"repo_root\":\"{}\",\"mode\":\"manual\",\"worktree\":\"{}\",\"session\":\"swarmux-pane-1\",\"command\":[\"echo\",\"one\"]}}",
        harness.fake_root.path().join("repo").display(),
        harness.fake_root.path().join("repo").display()
    );
    let payload_two = format!(
        "{{\"title\":\"Second pane\",\"repo_ref\":\"core\",\"repo_root\":\"{}\",\"mode\":\"manual\",\"worktree\":\"{}\",\"session\":\"swarmux-pane-2\",\"command\":[\"echo\",\"two\"]}}",
        harness.fake_root.path().join("repo").display(),
        harness.fake_root.path().join("repo").display()
    );

    let first = harness
        .run(&["--output", "json", "submit", "--json", &payload_one])
        .success()
        .get_output()
        .stdout
        .clone();
    let first: Value = serde_json::from_slice(&first).unwrap();

    let second = harness
        .run(&["--output", "json", "submit", "--json", &payload_two])
        .success()
        .get_output()
        .stdout
        .clone();
    let second: Value = serde_json::from_slice(&second).unwrap();

    fs::write(
        harness.fake_root.path().join("panes.tsv"),
        format!(
            "other\t@9\t9\tother\t%99\t1\t0\t0\t/tmp/other\tbash\tother\nswarmux-pane-2\t@2\t2\twork\t%22\t1\t1\t0\t{}\tcodex\tsecond\nswarmux-pane-1\t@1\t1\twork\t%11\t1\t1\t0\t{}\tcodex\tfirst\n",
            harness.fake_root.path().join("repo").display(),
            harness.fake_root.path().join("repo").display()
        ),
    )
    .unwrap();

    let jumped = harness
        .run_in_tmux(&["--output", "json", "panes", "jump", "--index", "2"])
        .success()
        .get_output()
        .stdout
        .clone();
    let jumped: Value = serde_json::from_slice(&jumped).unwrap();

    assert_eq!(jumped["ok"], true);
    assert_eq!(jumped["index"], 2);
    assert_eq!(jumped["pane_id"], "%22");
    assert_eq!(jumped["session_name"], "swarmux-pane-2");
    assert_eq!(jumped["task_id"], second["id"]);
    assert_ne!(jumped["task_id"], first["id"]);

    let tmux_log = fs::read_to_string(harness.fake_root.path().join("tmux.log")).unwrap();
    assert!(tmux_log.contains("switch-client -t swarmux-pane-2"));
    assert!(tmux_log.contains("select-window -t @2"));
    assert!(tmux_log.contains("select-pane -t %22"));
}

#[test]
fn panes_jump_rejects_missing_slot_after_ignoring_unmanaged_panes() {
    let harness = Harness::new();
    harness.run(&["init"]).success();

    let payload = format!(
        "{{\"title\":\"Managed pane\",\"repo_ref\":\"core\",\"repo_root\":\"{}\",\"mode\":\"manual\",\"worktree\":\"{}\",\"session\":\"swarmux-pane-1\",\"command\":[\"echo\",\"one\"]}}",
        harness.fake_root.path().join("repo").display(),
        harness.fake_root.path().join("repo").display()
    );

    harness
        .run(&["--output", "json", "submit", "--json", &payload])
        .success();

    fs::write(
        harness.fake_root.path().join("panes.tsv"),
        format!(
            "swarmux-pane-1\t@1\t1\twork\t%11\t1\t1\t0\t{}\tcodex\tfirst\nother\t@9\t9\tother\t%99\t1\t0\t0\t/tmp/other\tbash\tother\n",
            harness.fake_root.path().join("repo").display()
        ),
    )
    .unwrap();

    harness
        .run_in_tmux(&["panes", "jump", "--index", "2"])
        .failure()
        .stderr(predicate::str::contains("no managed pane at index 2"));
}

fn write_fake_tmux(path: PathBuf, root: &Path) {
    let script = format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
root="{root}"
cmd="${{1:-}}"
shift || true
printf '%s\n' "$cmd $*" >> "$root/tmux.log"

case "$cmd" in
  --help)
    printf 'tmux fake\n'
    ;;
  -V)
    printf 'tmux 3.7\n'
    ;;
  list-panes)
    cat "$root/panes.tsv"
    ;;
  switch-client|select-window|select-pane|set-option)
    exit 0
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

repo_root=""
if [ "${{1:-}}" = "-C" ]; then
  repo_root="$2"
  shift 2
fi

repo_ok=false
if [ "$repo_root" = "$root/repo" ]; then
  repo_ok=true
fi

case "${{1:-}}" in
  branch)
    if [ "$repo_ok" = true ] && [ "${{2:-}}" = "--show-current" ]; then
      cat "$root/git-branch.txt"
      exit 0
    fi
    exit 1
    ;;
  status)
    if [ "$repo_ok" = true ]; then
      cat "$root/git-status.txt"
      exit 0
    fi
    exit 1
    ;;
  diff)
    if [ "$repo_ok" = true ]; then
      if [ "${{2:-}}" = "--cached" ] && [ "${{3:-}}" = "--numstat" ]; then
        cat "$root/git-cached.txt"
      elif [ "${{2:-}}" = "--numstat" ]; then
        cat "$root/git-diff.txt"
      else
        exit 1
      fi
      exit 0
    fi
    exit 1
    ;;
  *)
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
