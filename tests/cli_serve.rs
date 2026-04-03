mod support;

use std::fs;

use reqwest::blocking::Client;
use tempfile::tempdir;

use support::*;

#[test]
fn serve_indexes_default_agent_skill_guide() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("init")
        .assert()
        .success();

    let mut server = start_server(&temp, true);

    let output = base_command(&temp)
        .args(["show", "mem://agent/skills/memento/SKILL.md"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = strip_ansi(&String::from_utf8(output).expect("stdout is utf-8"));
    assert!(stdout.contains("mem://agent/skills/memento/SKILL.md"));
    assert!(stdout.contains(".memento/agent/skills/memento/SKILL.md"));

    let output = base_command(&temp)
        .args(["cat", "mem://agent/skills/memento/SKILL.md"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = strip_ansi(&String::from_utf8(output).expect("stdout is utf-8"));
    assert!(stdout.contains("Use `memento` to store and search local project knowledge."));
    assert!(stdout.contains("## Using the CLI"));
    assert!(stdout.contains("Run `memento serve` before using server-backed commands"));
    assert!(stdout.contains("## Using the MCP server"));
    assert!(
        stdout
            .contains("The MCP endpoint shares the same `http://127.0.0.1:<server_port>` address")
    );
    assert!(stdout.contains("### MCP tools"));
    assert!(stdout.contains("- `remember`"));
    assert!(stdout.contains("- `show`"));
    assert!(stdout.contains("- `find`"));
    assert!(stdout.contains("memento add <path>..."));

    stop_server(&mut server);
}

#[test]
fn serve_indexes_all_agent_skill_files() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("init")
        .assert()
        .success();

    let skills_dir = temp.path().join(".memento").join("agent").join("skills");
    fs::create_dir_all(skills_dir.join("rust")).expect("create nested skills dir");
    fs::write(skills_dir.join("debugging.md"), "Debugging checklist")
        .expect("write debugging skill");
    fs::write(
        skills_dir.join("rust").join("refactor.md"),
        "Refactor carefully",
    )
    .expect("write nested skill");

    let mut server = start_server(&temp, true);

    let output = base_command(&temp)
        .args(["ls", "mem://agent/skills"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = strip_ansi(&String::from_utf8(output).expect("stdout is utf-8"));
    assert!(stdout.contains("mem://agent/skills/memento"));
    assert!(stdout.contains("mem://agent/skills/debugging.md"));
    assert!(stdout.contains("mem://agent/skills/rust"));

    let server_stderr =
        fs::read_to_string(temp.path().join("server.stderr.log")).expect("read server stderr log");
    assert!(server_stderr.contains(
        "memento auto-indexing agent item mem://agent/skills/memento/SKILL.md from .memento/agent/skills/memento/SKILL.md"
    ));
    assert!(server_stderr.contains(
        "memento auto-indexing agent item mem://agent/skills/debugging.md from .memento/agent/skills/debugging.md"
    ));
    assert!(server_stderr.contains(
        "memento auto-indexing agent item mem://agent/skills/rust/refactor.md from .memento/agent/skills/rust/refactor.md"
    ));

    let output = base_command(&temp)
        .args(["show", "mem://agent/skills/rust/refactor.md"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = strip_ansi(&String::from_utf8(output).expect("stdout is utf-8"));
    assert!(stdout.contains("mem://agent/skills/rust/refactor.md"));
    assert!(stdout.contains(".memento/agent/skills/rust/refactor.md"));

    stop_server(&mut server);
}

#[test]
fn serve_auto_indexes_agent_and_user_namespace_files_with_extensions() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("init")
        .assert()
        .success();

    let agent_dir = temp.path().join(".memento").join("agent");
    let user_dir = temp.path().join(".memento").join("user");

    fs::create_dir_all(agent_dir.join("notes")).expect("create nested agent dir");
    fs::create_dir_all(user_dir.join("preferences")).expect("create nested user dir");
    fs::write(
        agent_dir.join("notes").join("release-plan.txt"),
        "Ship in phases",
    )
    .expect("write agent note");
    fs::write(
        user_dir.join("preferences").join("editor.json"),
        "{\"tabSize\": 2}",
    )
    .expect("write user preference");

    let mut server = start_server(&temp, true);

    let agent_show = base_command(&temp)
        .args(["show", "mem://agent/notes/release-plan.txt"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let agent_show_stdout = strip_ansi(&String::from_utf8(agent_show).expect("stdout is utf-8"));
    assert!(agent_show_stdout.contains("mem://agent/notes/release-plan.txt"));
    assert!(agent_show_stdout.contains(".memento/agent/notes/release-plan.txt"));

    let user_show = base_command(&temp)
        .args(["show", "mem://user/preferences/editor.json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let user_show_stdout = strip_ansi(&String::from_utf8(user_show).expect("stdout is utf-8"));
    assert!(user_show_stdout.contains("mem://user/preferences/editor.json"));
    assert!(user_show_stdout.contains(".memento/user/preferences/editor.json"));

    let server_stderr =
        fs::read_to_string(temp.path().join("server.stderr.log")).expect("read server stderr log");
    assert!(server_stderr.contains(
        "memento auto-indexing agent item mem://agent/notes/release-plan.txt from .memento/agent/notes/release-plan.txt"
    ));
    assert!(server_stderr.contains(
        "memento auto-indexing user item mem://user/preferences/editor.json from .memento/user/preferences/editor.json"
    ));

    stop_server(&mut server);
}

#[test]
fn serve_debug_logs_find_timing_details() {
    let temp = tempdir().expect("create temp dir");
    let notes = temp.path().join("notes.txt");

    fs::write(
        &notes,
        "alpha topic intro\nalpha topic details\nbeta topic note",
    )
    .expect("write notes");

    let mut init = base_command(&temp);
    init.env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["init", "--model", "bge-small-en-v1.5"])
        .assert()
        .success();

    let mut server = start_server_with_args(&temp, true, &["--debug"]);

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["add", "notes.txt"])
        .assert()
        .success();

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["find", "alpha details"])
        .assert()
        .success();

    stop_server(&mut server);

    let stderr =
        fs::read_to_string(temp.path().join("server.stderr.log")).expect("read server stderr log");

    assert!(stderr.contains("[memento:debug] server debug logging enabled"));
    assert!(stderr.contains("command=find phase=execute latency_ms="));
    assert!(stderr.contains("command=find status=ok total_latency_ms="));
    assert!(stderr.contains("[memento:timing] find_total="));
}

#[test]
fn serve_dir_flag_overrides_working_directory() {
    use std::process::Command as ProcessCommand;

    let workspace = tempdir().expect("create workspace dir");
    let wrong_cwd = tempdir().expect("create wrong cwd dir");

    base_command(&workspace)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("init")
        .assert()
        .success();

    let exe = assert_cmd::cargo::cargo_bin("memento");
    let port = reserve_port();
    write_server_port(&workspace, port);
    let stdout_path = workspace.path().join("server.stdout.log");
    let stderr_path = workspace.path().join("server.stderr.log");

    let mut server = ProcessCommand::new(exe)
        .current_dir(wrong_cwd.path())
        .env(
            "MEMENTO_MODEL_CACHE_DIR",
            workspace.path().join("model-cache"),
        )
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args([
            "serve",
            "--dir",
            workspace.path().to_str().expect("workspace path is utf-8"),
        ])
        .stdout(std::fs::File::create(&stdout_path).expect("create server stdout log"))
        .stderr(std::fs::File::create(&stderr_path).expect("create server stderr log"))
        .spawn()
        .expect("start server");

    wait_for_server(&mut server, port, &stdout_path, &stderr_path);

    let resp = Client::new()
        .get(format!("http://127.0.0.1:{port}/health"))
        .send()
        .expect("health request");
    assert!(resp.status().is_success());

    let _ = server.kill();
    let _ = server.wait();
}
