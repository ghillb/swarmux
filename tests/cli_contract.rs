use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn schema_is_available_as_machine_readable_json() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_swarmux"));
    command.args(["--output", "json", "schema"]);
    command
        .assert()
        .success()
        .stdout(predicate::str::contains("\"commands\""))
        .stdout(predicate::str::contains("\"submit\""))
        .stdout(predicate::str::contains("\"notify\""))
        .stdout(predicate::str::contains("\"watch\""))
        .stdout(predicate::str::contains("\"json_input\""));
}

#[test]
fn submit_supports_raw_json_payloads_in_dry_run_mode() {
    let payload = r#"{
      "title":"Implement acceptance criteria",
      "repo_ref":"core",
      "repo_root":"/tmp/core",
      "mode":"manual",
      "worktree":"/tmp/swarmux-worktree",
      "session":"swarmux-task-1",
      "command":["codex","exec","Implement acceptance criteria"]
    }"#;

    let mut command = Command::new(env!("CARGO_BIN_EXE_swarmux"));
    command.args(["--output", "json", "submit", "--dry-run", "--json", payload]);
    command
        .assert()
        .success()
        .stdout(predicate::str::contains("\"dry_run\":true"))
        .stdout(predicate::str::contains(
            "\"title\":\"Implement acceptance criteria\"",
        ))
        .stdout(predicate::str::contains("\"session\":\"swarmux-task-1\""));
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
