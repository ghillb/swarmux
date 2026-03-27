use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

fn run(home: &TempDir, args: &[&str]) -> assert_cmd::assert::Assert {
    let mut command = Command::new(env!("CARGO_BIN_EXE_swarmux"));
    command.env("SWARMUX_HOME", home.path());
    command.args(args);
    command.assert()
}

#[test]
fn overview_title_and_once_render_summary() {
    let home = TempDir::new().unwrap();
    run(&home, &["init"]).success();

    let payload = r#"{
      "title":"Popup task",
      "repo_ref":"core",
      "repo_root":"/tmp/core",
      "mode":"manual",
      "worktree":"/tmp/swarmux-popup",
      "session":"swarmux-popup",
      "command":["echo","popup"]
    }"#;

    let submitted = run(&home, &["--output", "json", "submit", "--json", payload])
        .success()
        .get_output()
        .stdout
        .clone();
    let submitted: Value = serde_json::from_slice(&submitted).unwrap();
    let task_id = submitted["id"].as_str().unwrap().to_owned();

    run(
        &home,
        &[
            "--output",
            "json",
            "done",
            &task_id,
            "--reason",
            "manual_done",
        ],
    )
    .success();

    run(&home, &["overview", "--title"])
        .success()
        .stdout(predicate::str::contains("Swarmux"));

    run(&home, &["overview", "--once"])
        .success()
        .stdout(predicate::str::contains("Swarmux popup").not())
        .stdout(predicate::str::contains("\"total\":1"))
        .stdout(predicate::str::contains("\"succeeded\":1"))
        .stdout(predicate::str::contains(&task_id).not());

    run(&home, &["overview", "--once", "--scope", "terminal"])
        .success()
        .stdout(predicate::str::contains(&task_id))
        .stdout(predicate::str::contains("\"state\":\"succeeded\""));
}

#[test]
fn overview_defaults_to_non_terminal_rows() {
    let home = TempDir::new().unwrap();
    run(&home, &["init"]).success();

    let payload = r#"{
      "title":"Running task",
      "repo_ref":"core",
      "repo_root":"/tmp/core",
      "mode":"manual",
      "worktree":"/tmp/swarmux-running",
      "session":"swarmux-running",
      "command":["echo","running"]
    }"#;

    let submitted = run(&home, &["--output", "json", "submit", "--json", payload])
        .success()
        .get_output()
        .stdout
        .clone();
    let submitted: Value = serde_json::from_slice(&submitted).unwrap();
    let task_id = submitted["id"].as_str().unwrap().to_owned();

    run(&home, &["--output", "json", "start", &task_id])
        .success()
        .stdout(predicate::str::contains("\"state\":\"running\""));

    run(&home, &["overview", "--once"])
        .success()
        .stdout(predicate::str::contains("\"running\":1"))
        .stdout(predicate::str::contains(&task_id))
        .stdout(predicate::str::contains("\"title\":\"Running task\""));
}
