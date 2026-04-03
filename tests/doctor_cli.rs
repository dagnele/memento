use std::fs;
use std::net::TcpStream;
use std::path::Path;
use std::time::{Duration, Instant};

use assert_cmd::Command;
use tempfile::tempdir;

fn strip_ansi(input: &str) -> String {
    let mut output = String::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();

            for next in chars.by_ref() {
                if ('@'..='~').contains(&next) {
                    break;
                }
            }

            continue;
        }

        output.push(ch);
    }

    output
}

fn base_command(temp: &tempfile::TempDir) -> Command {
    let mut command = Command::cargo_bin("memento").expect("binary exists");
    command
        .current_dir(temp.path())
        .env("MEMENTO_MODEL_CACHE_DIR", temp.path().join("model-cache"));
    command
}

fn start_server(temp: &tempfile::TempDir, test_embedding: bool) -> std::process::Child {
    use std::process::Command as ProcessCommand;

    let exe = assert_cmd::cargo::cargo_bin("memento");
    let port = reserve_port();
    write_server_port(temp, port);
    let stdout_path = temp.path().join("server.stdout.log");
    let stderr_path = temp.path().join("server.stderr.log");
    let mut command = ProcessCommand::new(exe);
    command
        .current_dir(temp.path())
        .env("MEMENTO_MODEL_CACHE_DIR", temp.path().join("model-cache"))
        .arg("serve")
        .stdout(std::fs::File::create(&stdout_path).expect("create server stdout log"))
        .stderr(std::fs::File::create(&stderr_path).expect("create server stderr log"));

    if test_embedding {
        command.env("MEMENTO_TEST_EMBEDDING", "1");
    }

    let mut child = command.spawn().expect("start server");
    wait_for_server(&mut child, port, &stdout_path, &stderr_path);
    child
}

fn stop_server(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

use std::sync::atomic::{AtomicU16, Ordering};

static NEXT_PORT: AtomicU16 = AtomicU16::new(20000);

fn reserve_port() -> u16 {
    NEXT_PORT.fetch_add(1, Ordering::Relaxed)
}

fn write_server_port(temp: &tempfile::TempDir, port: u16) {
    let config_path = temp.path().join(".memento").join("config.toml");
    let config = fs::read_to_string(&config_path).expect("read config");
    let mut replaced = false;
    let mut lines = Vec::new();

    for line in config.lines() {
        if line.starts_with("server_port = ") {
            lines.push(format!("server_port = {port}"));
            replaced = true;
        } else {
            lines.push(line.to_string());
        }
    }

    if !replaced {
        lines.push(format!("server_port = {port}"));
    }

    fs::write(config_path, lines.join("\n") + "\n").expect("write config with server port");
}

fn wait_for_server(
    child: &mut std::process::Child,
    port: u16,
    stdout_path: &Path,
    stderr_path: &Path,
) {
    let deadline = Instant::now() + Duration::from_secs(5);

    while Instant::now() < deadline {
        if let Some(status) = child.try_wait().expect("poll server process") {
            let stdout = fs::read_to_string(stdout_path).unwrap_or_default();
            let stderr = fs::read_to_string(stderr_path).unwrap_or_default();
            panic!("server exited early with {status}\nstdout:\n{stdout}\nstderr:\n{stderr}");
        }

        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }

        std::thread::sleep(Duration::from_millis(50));
    }

    stop_server(child);
    let stdout = fs::read_to_string(stdout_path).unwrap_or_default();
    let stderr = fs::read_to_string(stderr_path).unwrap_or_default();
    panic!("server did not start on port {port}\nstdout:\n{stdout}\nstderr:\n{stderr}");
}

fn replace_index_metadata(temp: &tempfile::TempDir, key: &str, value: &str) {
    use rusqlite::Connection;

    let index_path = temp.path().join(".memento").join("index.db");
    let connection = Connection::open(index_path).expect("open index");
    connection
        .execute(
            "INSERT INTO workspace_meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            rusqlite::params![key, value],
        )
        .expect("update workspace metadata");
}

#[test]
fn doctor_reports_workspace_health_information() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["init", "--model", "bge-small-en-v1.5"])
        .assert()
        .success();

    let mut server = start_server(&temp, true);

    let output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
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
    assert!(stdout.contains("ok config_load version=4 model=bge-small-en-v1.5 dim=384"));
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
fn doctor_fails_when_workspace_embedding_does_not_match_active_profile() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["init", "--model", "bge-small-en-v1.5"])
        .assert()
        .success();

    replace_index_metadata(&temp, "embedding_dimension", "768");

    let mut server = start_server(&temp, true);

    let output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("doctor")
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let stdout = strip_ansi(&String::from_utf8(output).expect("stdout is utf-8"));
    assert!(stdout.contains("ok embedding_workspace model=bge-small-en-v1.5 dim=768"));
    assert!(stdout.contains(
        "fail embedding_match config model=bge-small-en-v1.5 dim=384, workspace model=bge-small-en-v1.5 dim=768, active model=bge-small-en-v1.5 dim=384"
    ));

    stop_server(&mut server);
}
