use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn submit_rejects_traversal_paths() {
    let payload = r#"{
      "title":"Traversal",
      "repo_ref":"core",
      "repo_root":"/tmp/core",
      "mode":"manual",
      "worktree":"../../.ssh",
      "session":"swarmux-task-2",
      "command":["echo","nope"]
    }"#;

    let mut command = Command::new(env!("CARGO_BIN_EXE_swarmux"));
    command.args(["submit", "--dry-run", "--json", payload]);
    command
        .assert()
        .failure()
        .stderr(predicate::str::contains("path traversal"));
}

#[test]
fn submit_rejects_reserved_characters_in_resource_names() {
    let payload = r#"{
      "title":"Bad session",
      "repo_ref":"core",
      "repo_root":"/tmp/core",
      "mode":"manual",
      "worktree":"/tmp/swarmux-worktree",
      "session":"swarmux%task",
      "command":["echo","nope"]
    }"#;

    let mut command = Command::new(env!("CARGO_BIN_EXE_swarmux"));
    command.args(["submit", "--dry-run", "--json", payload]);
    command
        .assert()
        .failure()
        .stderr(predicate::str::contains("must not contain %, ?, or #"));
}

#[test]
fn submit_rejects_control_characters() {
    let payload = "{\n  \"title\":\"Bad\\u0007Title\",\n  \"repo_ref\":\"core\",\n  \"repo_root\":\"/tmp/core\",\n  \"mode\":\"manual\",\n  \"worktree\":\"/tmp/swarmux-worktree\",\n  \"session\":\"swarmux-task-3\",\n  \"command\":[\"echo\",\"nope\"]\n}";

    let mut command = Command::new(env!("CARGO_BIN_EXE_swarmux"));
    command.args(["submit", "--dry-run", "--json", payload]);
    command
        .assert()
        .failure()
        .stderr(predicate::str::contains("control characters"));
}
