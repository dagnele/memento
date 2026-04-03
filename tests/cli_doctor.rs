mod support;

use tempfile::tempdir;

use support::*;

#[test]
fn doctor_reports_workspace_and_cache_information() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp)
        .args(["init", "--model", "bge-small-en-v1.5"])
        .assert()
        .success();

    let mut server = start_server(&temp, true);

    let output = base_command(&temp)
        .arg("doctor")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = strip_ansi(&String::from_utf8(output).expect("stdout is utf-8"));
    assert!(stdout.contains("memento doctor workspace health report"));
    assert!(stdout.contains("ok workspace .memento"));
    assert!(stdout.contains("ok config .memento/config.toml"));
    assert!(stdout.contains("ok index .memento/index.db"));
    assert!(stdout.contains("ok user_dir .memento/user"));
    assert!(stdout.contains("ok agent_dir .memento/agent"));
    assert!(stdout.contains("warn model_cache"));
    assert!(stdout.contains(
        "ok config_load version=4 model=bge-small-en-v1.5 dim=384 segment_lines=40 overlap=10"
    ));
    assert!(stdout.contains("warn embedding_active model=bge-small-en-v1.5 dim=384"));
    assert!(stdout.contains("ok index_open .memento/index.db"));
    assert!(stdout.contains("ok index_schema version=6"));
    assert!(stdout.contains("ok embedding_workspace model=bge-small-en-v1.5 dim=384"));
    assert!(stdout.contains("ok item_count 1"));
    assert!(stdout.contains("ok embedding_match workspace matches active profile"));
    assert!(stdout.contains("info test_embedding enabled"));

    stop_server(&mut server);
}

#[test]
fn remote_commands_fail_cleanly_when_server_is_not_running() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp).arg("init").assert().success();
    write_server_port(&temp, reserve_port());

    let output = base_command(&temp)
        .arg("doctor")
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();

    let stderr = strip_ansi(&String::from_utf8(output).expect("stderr is utf-8"));
    assert!(stderr.contains("failed to reach Memento server"));
    assert!(stderr.contains("run `memento serve` first"));
}

#[test]
fn remote_commands_reject_invalid_server_port_config() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp).arg("init").assert().success();
    replace_config_line(&temp, "server_port = ", "server_port = 0");

    let output = base_command(&temp)
        .arg("doctor")
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();

    let stderr = strip_ansi(&String::from_utf8(output).expect("stderr is utf-8"));
    assert!(stderr.contains("server_port must be greater than 0"));
}
