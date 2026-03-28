use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn schema_is_available_as_machine_readable_json() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_swarmux"));
    command.args(["schema"]);
    command
        .assert()
        .success()
        .stdout(predicate::str::contains("\"commands\""))
        .stdout(predicate::str::contains("\"dispatch\""))
        .stdout(predicate::str::contains("\"submit\""))
        .stdout(predicate::str::contains("\"panes\""))
        .stdout(predicate::str::contains("\"switch\""))
        .stdout(predicate::str::contains("\"notify\""))
        .stdout(predicate::str::contains("\"wait\""))
        .stdout(predicate::str::contains("\"watch\""))
        .stdout(predicate::str::contains("\"set-ref\""))
        .stdout(predicate::str::contains("\"json_input\""))
        .stdout(predicate::str::contains("\"supports_tui\":true"))
        .stdout(predicate::str::contains(
            "\"runtime_values\":[\"headless\",\"mirrored\",\"tui\"]",
        ));
}

#[test]
fn default_help_reports_json_output() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_swarmux"));
    command.args(["--help"]);
    command
        .assert()
        .success()
        .stdout(predicate::str::contains("[default: json]"));
}

#[test]
fn overview_help_exposes_tui_mode() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_swarmux"));
    command.args(["overview", "--help"]);
    command
        .assert()
        .success()
        .stdout(predicate::str::contains("--tui"));
}

#[test]
fn overview_tui_ignores_output_mode() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_swarmux"));
    command.args(["--output", "json", "overview", "--tui"]);
    command
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "overview --tui requires an interactive terminal",
        ))
        .stderr(predicate::str::contains("requires text output").not());
}

#[test]
fn panes_switch_help_exposes_tui_mode() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_swarmux"));
    command.args(["panes", "switch", "--help"]);
    command
        .assert()
        .success()
        .stdout(predicate::str::contains("--tui"))
        .stdout(predicate::str::contains("--tui-sidebar"))
        .stdout(predicate::str::contains("--launch-sidebar"))
        .stdout(predicate::str::contains("--pane-id"));
}

#[test]
fn panes_switch_tui_ignores_output_mode() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_swarmux"));
    command.args(["--output", "json", "panes", "switch", "--tui"]);
    command
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "panes switch --tui requires an interactive terminal",
        ))
        .stderr(predicate::str::contains("requires text output").not());
}

#[test]
fn submit_supports_raw_json_payloads_in_dry_run_mode() {
    let payload = r#"{
      "title":"Implement acceptance criteria",
      "repo_ref":"core",
      "repo_root":"/tmp/core",
      "mode":"manual",
      "runtime":"tui",
      "worktree":"/tmp/swarmux-worktree",
      "session":"swarmux-task-1",
      "command":["codex","exec","Implement acceptance criteria"]
    }"#;

    let mut command = Command::new(env!("CARGO_BIN_EXE_swarmux"));
    command.args(["submit", "--dry-run", "--json", payload]);
    command
        .assert()
        .success()
        .stdout(predicate::str::contains("\"dry_run\":true"))
        .stdout(predicate::str::contains(
            "\"title\":\"Implement acceptance criteria\"",
        ))
        .stdout(predicate::str::contains("\"runtime\":\"tui\""))
        .stdout(predicate::str::contains("\"session\":\"swarmux-task-1\""));
}

#[test]
fn dispatch_human_dry_run_prints_summary_text() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_swarmux"));
    command.args([
        "dispatch",
        "--dry-run",
        "--human",
        "--repo-ref",
        "core",
        "--repo-root",
        "/tmp/core",
        "--title",
        "Human task",
        "--",
        "codex",
        "hi",
    ]);
    command
        .assert()
        .success()
        .stdout(predicate::str::contains("dry-run"))
        .stdout(predicate::str::contains("title: Human task"))
        .stdout(predicate::str::contains("repo: core"))
        .stdout(predicate::str::contains("command: codex hi"))
        .stdout(predicate::str::contains("{").not());
}

#[test]
fn schema_supports_explicit_text_output() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_swarmux"));
    command.args(["--output", "text", "schema"]);
    command
        .assert()
        .success()
        .stdout(predicate::str::contains("{\n"))
        .stdout(predicate::str::contains("  \"commands\""));
}

#[test]
fn submit_rejects_legacy_repo_key() {
    let payload = r#"{
      "title":"Legacy payload",
      "repo":"core",
      "repo_root":"/tmp/core",
      "mode":"manual",
      "worktree":"/tmp/swarmux-worktree",
      "session":"swarmux-task-legacy",
      "command":["echo","legacy"]
    }"#;

    let mut command = Command::new(env!("CARGO_BIN_EXE_swarmux"));
    command.args(["submit", "--dry-run", "--json", payload]);
    command.assert().failure().stderr(predicate::str::contains(
        "failed to parse submit payload JSON",
    ));
}
