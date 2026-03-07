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
fn popup_title_and_once_render_summary() {
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

    run(&home, &["popup", "--title"])
        .success()
        .stdout(predicate::str::contains("Swarmux"));

    run(&home, &["popup", "--once"])
        .success()
        .stdout(predicate::str::contains("total=1"))
        .stdout(predicate::str::contains("succeeded=1"));
}
