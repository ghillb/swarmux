use assert_cmd::Command;
use predicates::prelude::*;
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
        write_fake_bd(bin.path().join("bd"), fake_root.path());
        std::fs::create_dir_all(fake_root.path().join("sessions")).unwrap();
        write_fake_tmux(bin.path().join("tmux"), fake_root.path());
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
        command.env("SWARMUX_BACKEND", "beads");
        command.env("SWARMUX_FAKE_BD_ROOT", self.fake_root.path());
        command.env("PATH", path);
        command.args(args);
        command.assert()
    }

    fn run_with_config_backend(&self, args: &[&str]) -> assert_cmd::assert::Assert {
        let mut command = Command::new(env!("CARGO_BIN_EXE_swarmux"));
        let path = format!(
            "{}:{}",
            self.bin.path().display(),
            std::env::var("PATH").unwrap()
        );
        let config_dir = self.home.path().join("config-home").join("swarmux");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("config.toml"), "backend = \"beads\"\n").unwrap();

        command.env("SWARMUX_HOME", self.home.path());
        command.env("SWARMUX_CONFIG_HOME", self.home.path().join("config-home"));
        command.env("SWARMUX_FAKE_BD_ROOT", self.fake_root.path());
        command.env("PATH", path);
        command.args(args);
        command.assert()
    }
}

#[test]
fn beads_backend_supports_doctor_init_submit_show_and_done() {
    let harness = Harness::new();

    harness
        .run(&["doctor"])
        .success()
        .stdout(predicate::str::contains("[ok] backend=beads"));

    harness.run(&["init"]).success();

    let payload = r#"{
      "title":"Beads task",
      "repo_ref":"core",
      "repo_root":"/tmp/core",
      "mode":"manual",
      "worktree":"/tmp/swarmux-beads",
      "session":"swarmux-beads",
      "command":["echo","beads"]
    }"#;

    let submitted = harness
        .run(&["--output", "json", "submit", "--json", payload])
        .success()
        .get_output()
        .stdout
        .clone();
    let submitted: serde_json::Value = serde_json::from_slice(&submitted).unwrap();
    let task_id = submitted["id"].as_str().unwrap().to_owned();

    harness
        .run(&["--output", "json", "show", &task_id])
        .success()
        .stdout(predicate::str::contains("\"title\":\"Beads task\""));

    harness
        .run(&[
            "--output",
            "json",
            "done",
            &task_id,
            "--reason",
            "manual_done",
        ])
        .success()
        .stdout(predicate::str::contains("\"state\":\"succeeded\""));

    harness
        .run(&["--output", "json", "list"])
        .success()
        .stdout(predicate::str::contains(&task_id));
}

#[test]
fn beads_backend_supports_start_and_reconcile() {
    let harness = Harness::new();
    harness.run(&["init"]).success();

    let payload = r#"{
      "title":"Beads runtime",
      "repo_ref":"core",
      "repo_root":"/tmp/core",
      "mode":"manual",
      "worktree":"/tmp/swarmux-beads-runtime",
      "session":"swarmux-beads-runtime",
      "command":["echo","runtime"]
    }"#;

    let submitted = harness
        .run(&["--output", "json", "submit", "--json", payload])
        .success()
        .get_output()
        .stdout
        .clone();
    let submitted: serde_json::Value = serde_json::from_slice(&submitted).unwrap();
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
            .join("swarmux-beads-runtime.pane"),
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
}

#[test]
fn beads_backend_supports_set_ref() {
    let harness = Harness::new();
    harness.run(&["init"]).success();

    let payload = r#"{
      "title":"Beads ref",
      "repo_ref":"core",
      "repo_root":"/tmp/core",
      "mode":"manual",
      "worktree":"/tmp/swarmux-beads-ref",
      "session":"swarmux-beads-ref",
      "command":["echo","ref"]
    }"#;

    let submitted = harness
        .run(&["--output", "json", "submit", "--json", payload])
        .success()
        .get_output()
        .stdout
        .clone();
    let submitted: serde_json::Value = serde_json::from_slice(&submitted).unwrap();
    let task_id = submitted["id"].as_str().unwrap().to_owned();

    harness
        .run(&[
            "--output",
            "json",
            "set-ref",
            &task_id,
            "https://github.com/example/repo/pull/456",
        ])
        .success()
        .stdout(predicate::str::contains(
            "\"external_ref\":\"https://github.com/example/repo/pull/456\"",
        ));

    harness
        .run(&["--output", "json", "show", &task_id])
        .success()
        .stdout(predicate::str::contains(
            "\"external_ref\":\"https://github.com/example/repo/pull/456\"",
        ));
}

#[test]
fn config_file_can_select_beads_backend() {
    let harness = Harness::new();

    harness
        .run_with_config_backend(&["doctor"])
        .success()
        .stdout(predicate::str::contains("[ok] backend=beads"));
}

fn write_fake_bd(path: PathBuf, root: &Path) {
    let script = format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
root="{root}"
db="$root/issues"
mkdir -p "$db"
counter_file="$root/counter"
[ -f "$counter_file" ] || printf '0' > "$counter_file"

next_id() {{
  count="$(cat "$counter_file")"
  next=$((count + 1))
  printf '%s' "$next" > "$counter_file"
  printf 'swx-%04d\n' "$next"
}}

sub="${{1:-}}"
shift || true

case "$sub" in
  init)
    mkdir -p "$root/.beads"
    printf '{{"ok":true}}\n'
    ;;
  create)
    title=""
    priority="2"
    labels=""
    description=""
    external_ref=""
    while [ "$#" -gt 0 ]; do
      case "$1" in
        --title|-title) title="$2"; shift 2 ;;
        -p|--priority) priority="$2"; shift 2 ;;
        -l|--labels) labels="$2"; shift 2 ;;
        -d|--description) description="$2"; shift 2 ;;
        --external-ref) external_ref="$2"; shift 2 ;;
        --json|-t|--type) shift ;;
        *) shift ;;
      esac
    done
    id="$(next_id)"
    printf '{{"id":"%s","title":"%s","status":"open","priority":%s,"labels":["%s"],"notes":"","external_ref":"%s","description":"%s"}}\n' "$id" "$title" "$priority" "$labels" "$external_ref" "$description" > "$db/$id.json"
    printf '{{"id":"%s"}}\n' "$id"
    ;;
  show)
    id="$1"
    cat "$db/$id.json" | jq -s .
    ;;
  list)
    jq -s . "$db"/*.json
    ;;
  update)
    id="$1"
    shift
    status=""
    labels=""
    notes=""
    external_ref=""
    while [ "$#" -gt 0 ]; do
      case "$1" in
        --status) status="$2"; shift 2 ;;
        --set-labels) labels="$2"; shift 2 ;;
        --notes) notes="$2"; shift 2 ;;
        --external-ref) external_ref="$2"; shift 2 ;;
        --json) shift ;;
        *) shift ;;
      esac
    done
    tmp="$(mktemp)"
    jq \
      --arg status "$status" \
      --arg labels "$labels" \
      --arg notes "$notes" \
      --arg external_ref "$external_ref" \
      'if $status != "" then .status = $status else . end
       | if $labels != "" then .labels = ($labels | split(",")) else . end
       | if $notes != "" then .notes = $notes else . end
       | if $external_ref != "" then .external_ref = $external_ref else . end' \
      "$db/$id.json" > "$tmp"
    mv "$tmp" "$db/$id.json"
    printf '{{"ok":true}}\n'
    ;;
  *)
    echo "unexpected bd command: $sub" >&2
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
  --help)
    printf 'tmux fake\n'
    ;;
  has-session)
    if [ "${{1:-}}" = "-t" ] && [ -f "$(session_file "$2")" ]; then
      exit 0
    fi
    exit 1
    ;;
  new-session)
    session=""
    log_file=""
    while [ "$#" -gt 0 ]; do
      case "$1" in
        -s) session="$2"; shift 2 ;;
        -c) shift 2 ;;
        -d) shift ;;
        /bin/sh) shift 2; log_file="$1"; break ;;
        *) shift ;;
      esac
    done
    mkdir -p "$(dirname "$log_file")"
    printf 'spawned %s\n' "$session" > "$(session_file "$session")"
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
  send-keys|kill-session|attach-session)
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
