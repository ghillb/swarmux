use assert_cmd::Command;
use predicates::prelude::*;
use regex::Regex;
use serde_json::Value;
use std::fs;
use std::process::Command as StdCommand;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

fn run(home: &TempDir, args: &[&str]) -> assert_cmd::assert::Assert {
    let mut command = Command::new(env!("CARGO_BIN_EXE_swarmux"));
    command.env("SWARMUX_HOME", home.path());
    command.env("SWARMUX_CONFIG_HOME", home.path().join("config-home"));
    command.args(args);
    command.assert()
}

#[test]
fn init_creates_state_layout_and_paths_reports_it() {
    let home = TempDir::new().unwrap();

    run(&home, &["init"]).success();

    run(&home, &["--output", "json", "paths"])
        .success()
        .stdout(predicate::str::contains(
            home.path().to_string_lossy().as_ref(),
        ))
        .stdout(predicate::str::contains("\"tasks_dir\""))
        .stdout(predicate::str::contains("\"events_file\""))
        .stdout(predicate::str::contains("\"notify_file\""))
        .stdout(predicate::str::contains("\"config_file\""));

    assert!(home.path().join("tasks").is_dir());
    assert!(home.path().join("logs").is_dir());
    assert!(home.path().join("locks").is_dir());
    assert!(home.path().join("events.jsonl").is_file());
    assert!(home.path().join("config-home").join("swarmux").is_dir());
}

#[test]
fn submit_show_list_done_and_fail_round_trip_through_files_backend() {
    let home = TempDir::new().unwrap();
    run(&home, &["init"]).success();

    let payload_one = r#"{
      "title":"First task",
      "repo_ref":"core",
      "repo_root":"/tmp/core",
      "mode":"manual",
      "worktree":"/tmp/swarmux-one",
      "session":"swarmux-one",
      "command":["echo","one"]
    }"#;

    let payload_two = r#"{
      "title":"Second task",
      "repo_ref":"core",
      "repo_root":"/tmp/core",
      "mode":"manual",
      "worktree":"/tmp/swarmux-two",
      "session":"swarmux-two",
      "command":["echo","two"]
    }"#;

    let first = run(
        &home,
        &["--output", "json", "submit", "--json", payload_one],
    )
    .success()
    .get_output()
    .stdout
    .clone();
    let first: Value = serde_json::from_slice(&first).unwrap();
    let first_id = first["id"].as_str().unwrap().to_owned();
    let id_pattern = Regex::new(r"^[0-9a-z]{3,12}$").unwrap();
    assert!(id_pattern.is_match(&first_id));
    assert!(!first_id.starts_with("swx-"));

    let second = run(
        &home,
        &["--output", "json", "submit", "--json", payload_two],
    )
    .success()
    .get_output()
    .stdout
    .clone();
    let second: Value = serde_json::from_slice(&second).unwrap();
    let second_id = second["id"].as_str().unwrap().to_owned();
    assert!(id_pattern.is_match(&second_id));
    assert!(!second_id.starts_with("swx-"));

    let list = run(&home, &["--output", "json", "list"])
        .success()
        .get_output()
        .stdout
        .clone();
    let list: Value = serde_json::from_slice(&list).unwrap();
    assert_eq!(list["tasks"].as_array().unwrap().len(), 2);

    let show = run(&home, &["--output", "json", "show", &first_id])
        .success()
        .get_output()
        .stdout
        .clone();
    let show: Value = serde_json::from_slice(&show).unwrap();
    assert_eq!(show["id"], first_id);
    assert_eq!(show["state"], "queued");

    run(
        &home,
        &[
            "--output",
            "json",
            "done",
            &first_id,
            "--reason",
            "manual_done",
        ],
    )
    .success()
    .stdout(predicate::str::contains("\"state\":\"succeeded\""));

    run(
        &home,
        &[
            "--output",
            "json",
            "fail",
            &second_id,
            "--reason",
            "manual_fail",
            "--error",
            "boom",
        ],
    )
    .success()
    .stdout(predicate::str::contains("\"state\":\"failed\""))
    .stdout(predicate::str::contains("\"last_error\":\"boom\""));
}

#[test]
fn show_and_list_support_field_projection() {
    let home = TempDir::new().unwrap();
    run(&home, &["init"]).success();

    let payload = r#"{
      "title":"Projected task",
      "repo_ref":"core",
      "repo_root":"/tmp/core",
      "mode":"manual",
      "worktree":"/tmp/swarmux-project",
      "session":"swarmux-project",
      "command":["echo","project"]
    }"#;

    let submitted = run(&home, &["--output", "json", "submit", "--json", payload])
        .success()
        .get_output()
        .stdout
        .clone();
    let submitted: Value = serde_json::from_slice(&submitted).unwrap();
    let task_id = submitted["id"].as_str().unwrap().to_owned();

    let show = run(
        &home,
        &["--output", "json", "show", &task_id, "--fields", "id,state"],
    )
    .success()
    .get_output()
    .stdout
    .clone();
    let show: Value = serde_json::from_slice(&show).unwrap();
    assert_eq!(show.as_object().unwrap().len(), 2);
    assert_eq!(show["id"], task_id);
    assert_eq!(show["state"], "queued");

    let list = run(&home, &["--output", "json", "list", "--fields", "id,state"])
        .success()
        .get_output()
        .stdout
        .clone();
    let list: Value = serde_json::from_slice(&list).unwrap();
    let first = &list["tasks"].as_array().unwrap()[0];
    assert_eq!(first.as_object().unwrap().len(), 2);
    assert_eq!(first["state"], "queued");
}

#[test]
fn set_ref_updates_external_ref_in_files_backend() {
    let home = TempDir::new().unwrap();
    run(&home, &["init"]).success();

    let payload = r#"{
      "title":"Ref task",
      "repo_ref":"core",
      "repo_root":"/tmp/core",
      "mode":"manual",
      "worktree":"/tmp/swarmux-ref",
      "session":"swarmux-ref",
      "command":["echo","ref"]
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
            "set-ref",
            &task_id,
            "https://github.com/example/repo/pull/123",
        ],
    )
    .success()
    .stdout(predicate::str::contains(
        "\"external_ref\":\"https://github.com/example/repo/pull/123\"",
    ));

    run(&home, &["--output", "json", "show", &task_id])
        .success()
        .stdout(predicate::str::contains(
            "\"external_ref\":\"https://github.com/example/repo/pull/123\"",
        ));
}

#[test]
fn wait_returns_the_first_matching_task() {
    let home = TempDir::new().unwrap();
    run(&home, &["init"]).success();

    let payload = r#"{
      "title":"Wait task",
      "repo_ref":"core",
      "repo_root":"/tmp/core",
      "mode":"manual",
      "worktree":"/tmp/swarmux-wait",
      "session":"swarmux-wait",
      "command":["echo","wait"]
    }"#;

    let submitted = run(&home, &["--output", "json", "submit", "--json", payload])
        .success()
        .get_output()
        .stdout
        .clone();
    let submitted: Value = serde_json::from_slice(&submitted).unwrap();
    let task_id = submitted["id"].as_str().unwrap().to_owned();

    let home_path = home.path().to_path_buf();
    let delayed_task_id = task_id.clone();
    let worker = thread::spawn(move || {
        thread::sleep(Duration::from_millis(150));
        let status = StdCommand::new(env!("CARGO_BIN_EXE_swarmux"))
            .env("SWARMUX_HOME", &home_path)
            .env("SWARMUX_CONFIG_HOME", home_path.join("config-home"))
            .args([
                "--output",
                "json",
                "done",
                &delayed_task_id,
                "--reason",
                "manual_done",
            ])
            .status()
            .unwrap();
        assert!(status.success());
    });

    run(
        &home,
        &[
            "--output",
            "json",
            "wait",
            &task_id,
            "--states",
            "succeeded",
            "--interval-ms",
            "50",
            "--timeout-ms",
            "2000",
        ],
    )
    .success()
    .stdout(predicate::str::contains("\"state\":\"succeeded\""));

    worker.join().unwrap();
}

#[test]
fn watch_streams_polls_until_a_task_matches() {
    let home = TempDir::new().unwrap();
    run(&home, &["init"]).success();

    let payload = r#"{
      "title":"Watch task",
      "repo_ref":"core",
      "repo_root":"/tmp/core",
      "mode":"manual",
      "worktree":"/tmp/swarmux-watch",
      "session":"swarmux-watch",
      "command":["echo","watch"]
    }"#;

    let submitted = run(&home, &["--output", "json", "submit", "--json", payload])
        .success()
        .get_output()
        .stdout
        .clone();
    let submitted: Value = serde_json::from_slice(&submitted).unwrap();
    let task_id = submitted["id"].as_str().unwrap().to_owned();

    let home_path = home.path().to_path_buf();
    let delayed_task_id = task_id.clone();
    let worker = thread::spawn(move || {
        thread::sleep(Duration::from_millis(150));
        let status = StdCommand::new(env!("CARGO_BIN_EXE_swarmux"))
            .env("SWARMUX_HOME", &home_path)
            .env("SWARMUX_CONFIG_HOME", home_path.join("config-home"))
            .args([
                "--output",
                "json",
                "done",
                &delayed_task_id,
                "--reason",
                "manual_done",
            ])
            .status()
            .unwrap();
        assert!(status.success());
    });

    run(
        &home,
        &[
            "--output",
            "json",
            "watch",
            &task_id,
            "--states",
            "succeeded",
            "--interval-ms",
            "50",
            "--timeout-ms",
            "2000",
        ],
    )
    .success()
    .stdout(predicate::str::contains("\"type\":\"poll\""))
    .stdout(predicate::str::contains("\"type\":\"matched\""))
    .stdout(predicate::str::contains("\"state\":\"succeeded\""));

    worker.join().unwrap();
}

#[test]
fn config_file_can_define_state_home() {
    let config_home = TempDir::new().unwrap();
    let state_home = config_home.path().join("custom-state");
    let config_dir = config_home.path().join("swarmux");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        format!("home = {:?}\nbackend = \"files\"\n", state_home),
    )
    .unwrap();

    let mut command = Command::new(env!("CARGO_BIN_EXE_swarmux"));
    command.env("SWARMUX_CONFIG_HOME", config_home.path());
    command.args(["--output", "json", "init"]);
    command.assert().success();

    assert!(state_home.join("tasks").is_dir());
    assert!(state_home.join("logs").is_dir());
}
