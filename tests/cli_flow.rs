use std::fs;
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::time::{Duration, Instant};

use assert_cmd::Command;
use reqwest::blocking::Client;
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

fn fixture_path(relative: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(relative)
}

fn base_command(temp: &tempfile::TempDir) -> Command {
    let mut command = Command::cargo_bin("memento").expect("binary exists");
    command
        .current_dir(temp.path())
        .env("MEMENTO_MODEL_CACHE_DIR", temp.path().join("model-cache"));
    command
}

fn start_server(temp: &tempfile::TempDir, test_embedding: bool) -> std::process::Child {
    start_server_with_args(temp, test_embedding, &[])
}

fn start_server_with_args(
    temp: &tempfile::TempDir,
    test_embedding: bool,
    extra_args: &[&str],
) -> std::process::Child {
    use std::process::Command as ProcessCommand;

    let exe = assert_cmd::cargo::cargo_bin("memento");
    let port = reserve_port_pair_start();
    write_server_port(temp, port);
    let stdout_path = temp.path().join("server.stdout.log");
    let stderr_path = temp.path().join("server.stderr.log");
    let mut command = ProcessCommand::new(exe);
    command
        .current_dir(temp.path())
        .env("MEMENTO_MODEL_CACHE_DIR", temp.path().join("model-cache"))
        .arg("serve")
        .args(extra_args)
        .stdout(std::fs::File::create(&stdout_path).expect("create server stdout log"))
        .stderr(std::fs::File::create(&stderr_path).expect("create server stderr log"));

    if test_embedding {
        command.env("MEMENTO_TEST_EMBEDDING", "1");
    }

    let mut child = command.spawn().expect("start server");
    wait_for_server(&mut child, port, &stdout_path, &stderr_path);
    wait_for_server(&mut child, port + 1, &stdout_path, &stderr_path);
    child
}

fn stop_server(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn reserve_port_pair_start() -> u16 {
    loop {
        let first = TcpListener::bind(("127.0.0.1", 0)).expect("bind ephemeral port");
        let port = first.local_addr().expect("read local address").port();

        if port < u16::MAX && TcpListener::bind(("127.0.0.1", port + 1)).is_ok() {
            return port;
        }
    }
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

fn replace_config_line(temp: &tempfile::TempDir, prefix: &str, replacement: &str) {
    let config_path = temp.path().join(".memento").join("config.toml");
    let config = fs::read_to_string(&config_path).expect("read config");
    let updated = config
        .lines()
        .map(|line| {
            if line.starts_with(prefix) {
                replacement.to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";

    fs::write(config_path, updated).expect("write updated config");
}

fn read_config_server_port(temp: &tempfile::TempDir) -> u16 {
    let config_path = temp.path().join(".memento").join("config.toml");
    let config = fs::read_to_string(config_path).expect("read config");

    config
        .lines()
        .find_map(|line| line.strip_prefix("server_port = "))
        .expect("server_port line")
        .parse()
        .expect("server_port is a valid u16")
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

#[test]
fn init_writes_default_config() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp).arg("init").assert().success();

    let config = fs::read_to_string(temp.path().join(".memento").join("config.toml"))
        .expect("config exists");

    assert!(config.contains("workspace_version = 4"));
    assert!(config.contains("embedding_model = \"bge-base-en-v1.5\""));
    assert!(config.contains("embedding_dimension = 768"));
    assert!(config.contains("segment_line_count = 40"));
    assert!(config.contains("segment_line_overlap = 10"));
    assert!(config.contains("server_port = 4000"));
    assert!(
        temp.path()
            .join(".memento")
            .join("agent")
            .join("skills")
            .join("memento.md")
            .exists()
    );
}

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
        .args(["show", "mem://agent/skills/memento"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = strip_ansi(&String::from_utf8(output).expect("stdout is utf-8"));
    assert!(stdout.contains("mem://agent/skills/memento"));
    assert!(stdout.contains(".memento/agent/skills/memento.md"));

    let output = base_command(&temp)
        .args(["cat", "mem://agent/skills/memento"])
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
    assert!(stdout.contains("The MCP endpoint listens on `http://127.0.0.1:<server_port + 1>`"));
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
    assert!(stdout.contains("mem://agent/skills/debugging"));
    assert!(stdout.contains("mem://agent/skills/rust"));

    let server_stdout =
        fs::read_to_string(temp.path().join("server.stdout.log")).expect("read server stdout log");
    assert!(server_stdout.contains(
        "memento serve auto-indexing agent skill mem://agent/skills/memento from .memento/agent/skills/memento.md"
    ));
    assert!(server_stdout.contains(
        "memento serve auto-indexing agent skill mem://agent/skills/debugging from .memento/agent/skills/debugging.md"
    ));
    assert!(server_stdout.contains(
        "memento serve auto-indexing agent skill mem://agent/skills/rust/refactor from .memento/agent/skills/rust/refactor.md"
    ));

    let output = base_command(&temp)
        .args(["show", "mem://agent/skills/rust/refactor"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = strip_ansi(&String::from_utf8(output).expect("stdout is utf-8"));
    assert!(stdout.contains("mem://agent/skills/rust/refactor"));
    assert!(stdout.contains(".memento/agent/skills/rust/refactor.md"));

    stop_server(&mut server);
}

#[test]
fn doctor_reports_workspace_and_cache_information() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp)
        .args(["init", "--model", "bge-small-en-v1.5"])
        .assert()
        .success();

    let mut server = start_server(&temp, false);

    let output = base_command(&temp)
        .arg("doctor")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = strip_ansi(&String::from_utf8(output).expect("stdout is utf-8"));
    assert!(stdout.contains("memento doctor environment report"));
    assert!(stdout.contains("workspace ok .memento"));
    assert!(stdout.contains("config ok .memento/config.toml"));
    assert!(stdout.contains("index ok .memento/index.db"));
    assert!(stdout.contains("user_dir ok .memento/user"));
    assert!(stdout.contains("agent_dir ok .memento/agent"));
    assert!(stdout.contains("model_cache missing"));
    assert!(stdout.contains("config_load ok version=4 model=bge-small-en-v1.5 dim=384"));
    assert!(stdout.contains("item_count 0"));
    assert!(stdout.contains("workspace_embedding model=bge-small-en-v1.5 dim=384"));
    assert!(stdout.contains("active_embedding bge-small-en-v1.5 dim=384"));
    assert!(stdout.contains("test_embedding disabled"));

    stop_server(&mut server);
}

#[test]
fn remote_commands_fail_cleanly_when_server_is_not_running() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp).arg("init").assert().success();
    write_server_port(&temp, reserve_port_pair_start());

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
    assert!(stderr.contains("command=find phase=request_read latency_ms="));
    assert!(stderr.contains("command=find phase=request_parse latency_ms="));
    assert!(stderr.contains("command=find phase=execute latency_ms="));
    assert!(stderr.contains("command=find status=ok total_latency_ms="));
    assert!(stderr.contains("[memento:timing] find_total="));
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

#[test]
fn serve_rejects_server_port_without_mcp_sibling_port() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp).arg("init").assert().success();
    replace_config_line(&temp, "server_port = ", "server_port = 65535");

    let output = base_command(&temp)
        .arg("doctor")
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();

    let stderr = strip_ansi(&String::from_utf8(output).expect("stderr is utf-8"));
    assert!(stderr.contains("server_port must be less than 65535"));
}

#[test]
fn serve_starts_mcp_listener_on_server_port_plus_one() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("init")
        .assert()
        .success();

    let mut server = start_server(&temp, true);
    let port = read_config_server_port(&temp);

    assert!(TcpStream::connect(("127.0.0.1", port + 1)).is_ok());

    stop_server(&mut server);
}

#[test]
fn mcp_tool_list_is_available_on_server_port_plus_one() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("init")
        .assert()
        .success();

    let mut server = start_server(&temp, true);
    let port = read_config_server_port(&temp);

    let client = Client::builder().build().expect("build reqwest client");
    let response = client
        .post(format!("http://127.0.0.1:{}", port + 1))
        .header("Accept", "application/json, text/event-stream")
        .header("Content-Type", "application/json")
        .body(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "cli-flow-test",
                        "version": "0.1.0"
                    }
                }
            })
            .to_string(),
        )
        .send()
        .expect("send initialize request");

    let status = response.status();
    let body = response.text().expect("read initialize response body");
    assert!(
        status.is_success(),
        "initialize failed with {status}: {body}"
    );
    assert!(body.contains("\"protocolVersion\":\"2025-06-18\""));

    let response = client
        .post(format!("http://127.0.0.1:{}", port + 1))
        .header("Accept", "application/json, text/event-stream")
        .header("Content-Type", "application/json")
        .header("MCP-Protocol-Version", "2025-06-18")
        .body(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/list",
                "params": {}
            })
            .to_string(),
        )
        .send()
        .expect("send tools/list request");

    assert!(response.status().is_success());
    let body = response.text().expect("read tools/list response");
    assert!(body.contains("\"name\":\"add\""));
    assert!(body.contains("\"name\":\"find\""));
    assert!(body.contains("\"name\":\"show\""));

    stop_server(&mut server);
}

#[test]
fn mcp_resources_list_and_read_mem_uri() {
    let temp = tempdir().expect("create temp dir");
    let notes = temp.path().join("notes.txt");
    fs::write(&notes, "alpha note\nsecond line\n").expect("write notes");

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("init")
        .assert()
        .success();

    let mut server = start_server(&temp, true);
    let port = read_config_server_port(&temp);

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["add", "notes.txt"])
        .assert()
        .success();

    let client = Client::builder().build().expect("build reqwest client");

    let initialize = client
        .post(format!("http://127.0.0.1:{}", port + 1))
        .header("Accept", "application/json, text/event-stream")
        .header("Content-Type", "application/json")
        .body(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "cli-flow-test",
                        "version": "0.1.0"
                    }
                }
            })
            .to_string(),
        )
        .send()
        .expect("send initialize request");

    assert!(initialize.status().is_success());

    let response = client
        .post(format!("http://127.0.0.1:{}", port + 1))
        .header("Accept", "application/json, text/event-stream")
        .header("Content-Type", "application/json")
        .header("MCP-Protocol-Version", "2025-06-18")
        .body(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "resources/list",
                "params": {}
            })
            .to_string(),
        )
        .send()
        .expect("send resources/list request");

    assert!(response.status().is_success());
    let body = response.text().expect("read resources/list response");
    assert!(
        body.contains("mem://resources/notes.txt"),
        "resources/list body was: {body}"
    );

    let response = client
        .post(format!("http://127.0.0.1:{}", port + 1))
        .header("Accept", "application/json, text/event-stream")
        .header("Content-Type", "application/json")
        .header("MCP-Protocol-Version", "2025-06-18")
        .body(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "resources/read",
                "params": {
                    "uri": "mem://resources/notes.txt"
                }
            })
            .to_string(),
        )
        .send()
        .expect("send resources/read request");

    assert!(response.status().is_success());
    let body = response.text().expect("read resources/read response");
    assert!(
        body.contains("alpha note\\nsecond line\\n"),
        "resources/read body was: {body}"
    );

    stop_server(&mut server);
}

#[test]
fn mcp_resource_templates_expose_mem_namespace_patterns() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("init")
        .assert()
        .success();

    let mut server = start_server(&temp, true);
    let port = read_config_server_port(&temp);
    let client = Client::builder().build().expect("build reqwest client");

    let initialize = client
        .post(format!("http://127.0.0.1:{}", port + 1))
        .header("Accept", "application/json, text/event-stream")
        .header("Content-Type", "application/json")
        .body(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "cli-flow-test",
                        "version": "0.1.0"
                    }
                }
            })
            .to_string(),
        )
        .send()
        .expect("send initialize request");

    assert!(initialize.status().is_success());

    let response = client
        .post(format!("http://127.0.0.1:{}", port + 1))
        .header("Accept", "application/json, text/event-stream")
        .header("Content-Type", "application/json")
        .header("MCP-Protocol-Version", "2025-06-18")
        .body(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "resources/templates/list",
                "params": {}
            })
            .to_string(),
        )
        .send()
        .expect("send resources/templates/list request");

    assert!(response.status().is_success());
    let body = response
        .text()
        .expect("read resources/templates/list response");
    assert!(
        body.contains("mem://resources/{path}"),
        "template body was: {body}"
    );
    assert!(
        body.contains("mem://user/{path}"),
        "template body was: {body}"
    );
    assert!(
        body.contains("mem://agent/{path}"),
        "template body was: {body}"
    );

    stop_server(&mut server);
}

#[test]
fn mcp_tools_call_show_returns_structured_result() {
    let temp = tempdir().expect("create temp dir");
    let notes = temp.path().join("notes.txt");
    fs::write(&notes, "alpha note\nsecond line\n").expect("write notes");

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("init")
        .assert()
        .success();

    let mut server = start_server(&temp, true);
    let port = read_config_server_port(&temp);

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["add", "notes.txt"])
        .assert()
        .success();

    let client = Client::builder().build().expect("build reqwest client");

    let initialize = client
        .post(format!("http://127.0.0.1:{}", port + 1))
        .header("Accept", "application/json, text/event-stream")
        .header("Content-Type", "application/json")
        .body(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "cli-flow-test",
                        "version": "0.1.0"
                    }
                }
            })
            .to_string(),
        )
        .send()
        .expect("send initialize request");

    assert!(initialize.status().is_success());

    let response = client
        .post(format!("http://127.0.0.1:{}", port + 1))
        .header("Accept", "application/json, text/event-stream")
        .header("Content-Type", "application/json")
        .header("MCP-Protocol-Version", "2025-06-18")
        .body(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {
                    "name": "show",
                    "arguments": {
                        "uri": "mem://resources/notes.txt"
                    }
                }
            })
            .to_string(),
        )
        .send()
        .expect("send tools/call request");

    assert!(response.status().is_success());
    let body = response.text().expect("read tools/call response");
    assert!(
        body.contains("\"type\":\"text\""),
        "tools/call body was: {body}"
    );
    assert!(
        body.contains("\\\"uri\\\":\\\"mem://resources/notes.txt\\\""),
        "tools/call body was: {body}"
    );
    assert!(
        body.contains("\\\"namespace\\\":\\\"resources\\\""),
        "tools/call body was: {body}"
    );
    assert!(
        body.contains("\\\"source_path\\\":\\\"notes.txt\\\""),
        "tools/call body was: {body}"
    );

    stop_server(&mut server);
}

#[test]
fn models_lists_supported_embedding_models() {
    let temp = tempdir().expect("create temp dir");

    let output = base_command(&temp)
        .arg("models")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = strip_ansi(&String::from_utf8(output).expect("stdout is utf-8"));
    assert!(stdout.contains("memento models supported embedding models"));
    assert!(stdout.contains(
        "bge-base-en-v1.5 dim=768 recommended use=balanced default for English notes and docs"
    ));
    assert!(
        stdout
            .contains("bge-small-en-v1.5 dim=384 use=fast lightweight indexing on local machines")
    );
    assert!(stdout.contains("bge-large-en-v1.5 dim=1024 use=highest-quality English retrieval"));
    assert!(stdout.contains(
        "jina-embeddings-v2-base-code dim=768 use=code-heavy repositories and source search"
    ));
    assert!(stdout.contains(
        "nomic-embed-text-v1.5 dim=768 use=longer English notes and general semantic search"
    ));
    assert!(stdout.contains("bge-m3 dim=1024 use=multilingual content across mixed repositories"));
}

#[test]
fn init_writes_selected_embedding_model() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp)
        .args(["init", "--model", "bge-base-en-v1.5"])
        .assert()
        .success();

    let config = fs::read_to_string(temp.path().join(".memento").join("config.toml"))
        .expect("config exists");

    assert!(config.contains("embedding_model = \"bge-base-en-v1.5\""));
    assert!(config.contains("embedding_dimension = 768"));
}

#[test]
fn init_accepts_newly_added_embedding_model() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp)
        .args(["init", "--model", "jina-embeddings-v2-base-code"])
        .assert()
        .success();

    let config = fs::read_to_string(temp.path().join(".memento").join("config.toml"))
        .expect("config exists");

    assert!(config.contains("embedding_model = \"jina-embeddings-v2-base-code\""));
    assert!(config.contains("embedding_dimension = 768"));
}

#[test]
fn init_writes_selected_server_port() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp)
        .args(["init", "--port", "4012"])
        .assert()
        .success();

    let config = fs::read_to_string(temp.path().join(".memento").join("config.toml"))
        .expect("config exists");

    assert!(config.contains("server_port = 4012"));
}

#[test]
fn find_returns_segment_preview_for_indexed_text() {
    let temp = tempdir().expect("create temp dir");
    let notes = temp.path().join("notes.txt");

    fs::write(
        &notes,
        [
            "alpha topic intro",
            "alpha topic details",
            "beta topic note",
            "gamma topic note",
        ]
        .join("\n"),
    )
    .expect("write notes");

    let mut init = base_command(&temp);
    init.env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["init", "--model", "bge-small-en-v1.5"])
        .assert()
        .success();

    let config_path = temp.path().join(".memento").join("config.toml");
    fs::write(
        &config_path,
        [
            "workspace_version = 4",
            "embedding_model = \"bge-small-en-v1.5\"",
            "embedding_dimension = 384",
            "segment_line_count = 2",
            "segment_line_overlap = 1",
            "server_port = 4000",
        ]
        .join("\n"),
    )
    .expect("rewrite config");

    let mut server = start_server(&temp, true);

    let mut add = base_command(&temp);
    add.env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["add", "notes.txt"])
        .assert()
        .success();

    let output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["find", "alpha details"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = strip_ansi(&String::from_utf8(output).expect("stdout is utf-8"));

    assert!(stdout.contains("mem://resources/notes.txt"));
    assert!(stdout.contains("lines=1-2"));
    assert!(stdout.contains("preview alpha topic intro | alpha topic details"));

    stop_server(&mut server);
}

#[test]
fn find_reports_no_matches_for_empty_workspace() {
    let temp = tempdir().expect("create temp dir");

    let mut init = base_command(&temp);
    init.env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("init")
        .assert()
        .success();

    let mut server = start_server(&temp, true);

    let output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["find", "nothing indexed yet"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = strip_ansi(&String::from_utf8(output).expect("stdout is utf-8"));
    assert!(stdout.contains("memento find searching for nothing indexed yet"));
    assert!(stdout.contains("status no matches found"));

    stop_server(&mut server);
}

#[test]
fn remember_file_preserves_text_extension_and_indexes_item() {
    let temp = tempdir().expect("create temp dir");
    let source = fixture_path("text/sample.txt");

    base_command(&temp).arg("init").assert().success();

    let mut server = start_server(&temp, true);

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args([
            "remember",
            "--namespace",
            "agent",
            "--path",
            "skills/sample-copy",
            "--file",
            source.to_str().expect("fixture path is utf-8"),
        ])
        .assert()
        .success();

    assert!(
        temp.path()
            .join(".memento")
            .join("agent")
            .join("skills")
            .join("sample-copy.txt")
            .exists()
    );

    let output = base_command(&temp)
        .args(["show", "mem://agent/skills/sample-copy"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = strip_ansi(&String::from_utf8(output).expect("stdout is utf-8"));
    assert!(stdout.contains("mem://agent/skills/sample-copy"));
    assert!(stdout.contains(".memento/agent/skills/sample-copy.txt"));

    stop_server(&mut server);
}

#[test]
fn remember_inline_text_creates_user_item() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp).arg("init").assert().success();

    let mut server = start_server(&temp, true);

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args([
            "remember",
            "--namespace",
            "user",
            "--path",
            "preferences/writing-style",
            "Prefer concise technical explanations",
        ])
        .assert()
        .success();

    let stored = temp
        .path()
        .join(".memento")
        .join("user")
        .join("preferences")
        .join("writing-style.md");

    assert!(stored.exists());
    assert_eq!(
        fs::read_to_string(&stored).expect("stored item is readable"),
        "Prefer concise technical explanations"
    );

    let output = base_command(&temp)
        .args(["show", "mem://user/preferences/writing-style"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = strip_ansi(&String::from_utf8(output).expect("stdout is utf-8"));
    assert!(stdout.contains("mem://user/preferences/writing-style"));
    assert!(stdout.contains("user_item"));
    assert!(stdout.contains(".memento/user/preferences/writing-style.md"));

    stop_server(&mut server);
}

#[test]
fn remember_file_rejects_binary_input() {
    let temp = tempdir().expect("create temp dir");
    let source = fixture_path("binary/sample.bin");

    base_command(&temp).arg("init").assert().success();

    let mut server = start_server(&temp, true);

    let output = base_command(&temp)
        .args([
            "remember",
            "--namespace",
            "agent",
            "--path",
            "skills/binary-copy",
            "--file",
            source.to_str().expect("fixture path is utf-8"),
        ])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();

    let stderr = strip_ansi(&String::from_utf8(output).expect("stderr is utf-8"));
    assert!(stderr.contains("only text-based UTF-8 files are supported"));

    stop_server(&mut server);
}

#[test]
fn add_rejects_binary_input() {
    let temp = tempdir().expect("create temp dir");
    let source = fixture_path("binary/sample.bin");
    let copied = temp.path().join("sample.bin");

    fs::copy(&source, &copied).expect("copy fixture");

    base_command(&temp).arg("init").assert().success();

    let mut server = start_server(&temp, true);

    let output = base_command(&temp)
        .args(["add", "sample.bin"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();

    let stderr = strip_ansi(&String::from_utf8(output).expect("stderr is utf-8"));
    assert!(stderr.contains("only text-based UTF-8 files are supported"));

    stop_server(&mut server);
}

#[test]
fn add_supports_glob_patterns_for_text_files() {
    let temp = tempdir().expect("create temp dir");

    fs::write(temp.path().join("one.md"), "alpha note").expect("write first markdown file");
    fs::write(temp.path().join("two.md"), "beta note").expect("write second markdown file");
    fs::write(temp.path().join("three.txt"), "plain text").expect("write text file");

    base_command(&temp).arg("init").assert().success();

    let mut server = start_server(&temp, true);

    let output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["add", "*.md"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = strip_ansi(&String::from_utf8(output).expect("stdout is utf-8"));
    assert!(stdout.contains("indexed 2 resource(s)"));
    assert!(stdout.contains("added one.md"));
    assert!(stdout.contains("added two.md"));
    assert!(!stdout.contains("three.txt"));

    stop_server(&mut server);
}

#[test]
fn add_glob_skips_matches_inside_memento_workspace_dir() {
    let temp = tempdir().expect("create temp dir");

    fs::write(temp.path().join("one.md"), "alpha note").expect("write markdown file");

    base_command(&temp).arg("init").assert().success();

    fs::write(
        temp.path().join(".memento").join("ignored.md"),
        "internal workspace note",
    )
    .expect("write internal markdown file");

    let mut server = start_server(&temp, true);

    let output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["add", "**/*.md"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = strip_ansi(&String::from_utf8(output).expect("stdout is utf-8"));
    assert!(stdout.contains("indexed 1 resource(s)"));
    assert!(stdout.contains("added one.md"));
    assert!(!stdout.contains("ignored.md"));

    let ls_output = base_command(&temp)
        .args(["ls", "mem://resources"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let ls_stdout = strip_ansi(&String::from_utf8(ls_output).expect("stdout is utf-8"));
    assert!(ls_stdout.contains("mem://resources/one.md"));
    assert!(!ls_stdout.contains("ignored.md"));

    stop_server(&mut server);
}

#[test]
fn add_rejects_explicit_paths_inside_memento_workspace_dir() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp).arg("init").assert().success();

    let nested = temp.path().join(".memento").join("user").join("nested");
    fs::create_dir_all(&nested).expect("create nested workspace directory");
    fs::write(nested.join("ignored.md"), "internal workspace note")
        .expect("write internal markdown file");

    let mut server = start_server(&temp, true);

    let output = base_command(&temp)
        .args(["add", ".memento/user/nested/ignored.md"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();

    let stderr = strip_ansi(&String::from_utf8(output).expect("stderr is utf-8"));
    assert!(stderr.contains("cannot add paths inside `.memento`"));
    assert!(stderr.contains(".memento/user/nested/ignored.md"));

    let ls_output = base_command(&temp)
        .args(["ls", "mem://resources"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let ls_stdout = strip_ansi(&String::from_utf8(ls_output).expect("stdout is utf-8"));
    assert!(!ls_stdout.contains("ignored.md"));

    stop_server(&mut server);
}

#[test]
fn add_rejects_glob_patterns_targeting_memento_workspace_dir() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp).arg("init").assert().success();

    fs::write(
        temp.path().join(".memento").join("ignored.md"),
        "internal workspace note",
    )
    .expect("write internal markdown file");

    let mut server = start_server(&temp, true);

    let output = base_command(&temp)
        .args(["add", ".memento/**/*.md"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();

    let stderr = strip_ansi(&String::from_utf8(output).expect("stderr is utf-8"));
    assert!(stderr.contains("cannot add paths inside `.memento`"));
    assert!(stderr.contains(".memento/**/*.md"));

    stop_server(&mut server);
}

#[test]
fn add_skips_already_indexed_resources_without_force() {
    let temp = tempdir().expect("create temp dir");
    let notes = temp.path().join("notes.txt");

    fs::write(&notes, "first version\nalpha").expect("write notes");

    let mut init = base_command(&temp);
    init.env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("init")
        .assert()
        .success();

    let mut server = start_server(&temp, true);

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["add", "notes.txt"])
        .assert()
        .success();

    fs::write(&notes, "second version\nalpha").expect("rewrite notes");

    let add_output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["add", "notes.txt"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let add_stdout = strip_ansi(&String::from_utf8(add_output).expect("stdout is utf-8"));
    assert!(add_stdout.contains("indexed 0 resource(s)"));
    assert!(add_stdout.contains("skipped notes.txt already indexed; use --force to re-add"));

    let find_output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["find", "first version"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let find_stdout = strip_ansi(&String::from_utf8(find_output).expect("stdout is utf-8"));
    assert!(find_stdout.contains("mem://resources/notes.txt"));
    assert!(find_stdout.contains("preview second version | alpha"));
    assert!(find_stdout.contains("state modified"));
    assert!(find_stdout.contains("run `memento reindex notes.txt`"));

    stop_server(&mut server);
}

#[test]
fn add_force_readds_already_indexed_resource() {
    let temp = tempdir().expect("create temp dir");
    let notes = temp.path().join("notes.txt");

    fs::write(&notes, "first version\nalpha").expect("write notes");

    let mut init = base_command(&temp);
    init.env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("init")
        .assert()
        .success();

    let mut server = start_server(&temp, true);

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["add", "notes.txt"])
        .assert()
        .success();

    fs::write(&notes, "second version\nalpha").expect("rewrite notes");

    let add_output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["add", "--force", "notes.txt"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let add_stdout = strip_ansi(&String::from_utf8(add_output).expect("stdout is utf-8"));
    assert!(add_stdout.contains("indexed 1 resource(s)"));
    assert!(add_stdout.contains("added notes.txt"));

    let find_output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["find", "second version"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let find_stdout = strip_ansi(&String::from_utf8(find_output).expect("stdout is utf-8"));
    assert!(find_stdout.contains("preview second version | alpha"));
    assert!(!find_stdout.contains("preview first version"));

    stop_server(&mut server);
}

#[test]
fn rm_untracks_resource_without_deleting_source_file() {
    let temp = tempdir().expect("create temp dir");
    let notes = temp.path().join("notes.txt");

    fs::write(&notes, "tracked content\nalpha").expect("write notes");

    let mut init = base_command(&temp);
    init.env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("init")
        .assert()
        .success();

    let mut server = start_server(&temp, true);

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["add", "notes.txt"])
        .assert()
        .success();

    let rm_output = base_command(&temp)
        .args(["rm", "notes.txt"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let rm_stdout = strip_ansi(&String::from_utf8(rm_output).expect("stdout is utf-8"));
    assert!(rm_stdout.contains("resource untracked"));
    assert!(rm_stdout.contains("mem://resources/notes.txt"));
    assert!(notes.exists());

    let find_output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["find", "tracked content"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let find_stdout = strip_ansi(&String::from_utf8(find_output).expect("stdout is utf-8"));
    assert!(find_stdout.contains("status no matches found"));

    stop_server(&mut server);
}

#[test]
fn forget_removes_memory_item_and_backing_file() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp).arg("init").assert().success();

    let mut server = start_server(&temp, true);

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args([
            "remember",
            "--namespace",
            "agent",
            "--path",
            "skills/to-remove",
            "temporary memory",
        ])
        .assert()
        .success();

    let stored = temp
        .path()
        .join(".memento")
        .join("agent")
        .join("skills")
        .join("to-remove.md");
    assert!(stored.exists());

    let forget_output = base_command(&temp)
        .args(["forget", "mem://agent/skills/to-remove"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let forget_stdout = strip_ansi(&String::from_utf8(forget_output).expect("stdout is utf-8"));
    assert!(forget_stdout.contains("item removed"));
    assert!(forget_stdout.contains("mem://agent/skills/to-remove"));
    assert!(!stored.exists());

    let find_output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["find", "temporary memory"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let find_stdout = strip_ansi(&String::from_utf8(find_output).expect("stdout is utf-8"));
    assert!(find_stdout.contains("status no matches found"));

    stop_server(&mut server);
}

#[test]
fn add_rejects_unmatched_glob_patterns() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp).arg("init").assert().success();

    let mut server = start_server(&temp, true);

    let output = base_command(&temp)
        .args(["add", "*.md"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();

    let stderr = strip_ansi(&String::from_utf8(output).expect("stderr is utf-8"));
    assert!(stderr.contains("glob matched no paths: `*.md`"));

    stop_server(&mut server);
}

#[test]
fn reindex_refreshes_modified_resource_via_server() {
    let temp = tempdir().expect("create temp dir");
    let notes = temp.path().join("notes.txt");

    fs::write(&notes, "first version\nalpha").expect("write notes");

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["init", "--model", "bge-small-en-v1.5"])
        .assert()
        .success();

    let mut server = start_server(&temp, true);

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["add", "notes.txt"])
        .assert()
        .success();

    fs::write(&notes, "second version\nalpha").expect("rewrite notes");

    let reindex_output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["reindex", "notes.txt"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let reindex_stdout = strip_ansi(&String::from_utf8(reindex_output).expect("stdout is utf-8"));
    assert!(reindex_stdout.contains("refreshed 1 resource(s)"));
    assert!(reindex_stdout.contains("reindexed notes.txt"));

    let find_output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["find", "second version"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let find_stdout = strip_ansi(&String::from_utf8(find_output).expect("stdout is utf-8"));
    assert!(find_stdout.contains("mem://resources/notes.txt"));
    assert!(find_stdout.contains("preview second version | alpha"));

    stop_server(&mut server);
}

#[test]
fn show_returns_resource_metadata_without_dumping_content() {
    let temp = tempdir().expect("create temp dir");
    let notes = temp.path().join("notes.txt");

    fs::write(&notes, "line one\nline two").expect("write notes");

    let mut init = base_command(&temp);
    init.env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("init")
        .assert()
        .success();

    let mut server = start_server(&temp, true);

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["add", "notes.txt"])
        .assert()
        .success();

    let output = base_command(&temp)
        .args(["show", "mem://resources/notes.txt"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = strip_ansi(&String::from_utf8(output).expect("stdout is utf-8"));
    assert!(stdout.contains("mem://resources/notes.txt"));
    assert!(stdout.contains("resource_file"));
    assert!(stdout.contains("layers"));
    assert!(!stdout.contains("line one"));
    assert!(!stdout.contains("line two"));

    stop_server(&mut server);
}

#[test]
fn cat_returns_resource_file_contents() {
    let temp = tempdir().expect("create temp dir");
    let notes = temp.path().join("notes.txt");

    fs::write(&notes, "alpha\nbeta").expect("write notes");

    let mut init = base_command(&temp);
    init.env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("init")
        .assert()
        .success();

    let mut server = start_server(&temp, true);

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["add", "notes.txt"])
        .assert()
        .success();

    let output = base_command(&temp)
        .args(["cat", "mem://resources/notes.txt"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = strip_ansi(&String::from_utf8(output).expect("stdout is utf-8"));
    assert_eq!(stdout.trim(), "alpha\nbeta");

    stop_server(&mut server);
}

#[test]
fn cat_returns_memory_item_contents() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp).arg("init").assert().success();

    let mut server = start_server(&temp, true);

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args([
            "remember",
            "--namespace",
            "user",
            "--path",
            "preferences/style",
            "Prefer concise explanations",
        ])
        .assert()
        .success();

    let output = base_command(&temp)
        .args(["cat", "mem://user/preferences/style"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = strip_ansi(&String::from_utf8(output).expect("stdout is utf-8"));
    assert_eq!(stdout.trim(), "Prefer concise explanations");

    stop_server(&mut server);
}

#[test]
fn cat_rejects_namespace_and_directory_uris() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp).arg("init").assert().success();

    let mut server = start_server(&temp, true);

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args([
            "remember",
            "--namespace",
            "user",
            "--path",
            "preferences/style",
            "Prefer concise explanations",
        ])
        .assert()
        .success();

    let namespace_output = base_command(&temp)
        .args(["cat", "mem://resources"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let namespace_stderr =
        strip_ansi(&String::from_utf8(namespace_output).expect("stderr is utf-8"));
    assert!(namespace_stderr.contains("refers to a virtual namespace"));
    assert!(namespace_stderr.contains("use `memento ls` or `memento show` instead"));

    let directory_output = base_command(&temp)
        .args(["cat", "mem://user/preferences"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let directory_stderr =
        strip_ansi(&String::from_utf8(directory_output).expect("stderr is utf-8"));
    assert!(directory_stderr.contains("refers to a virtual directory"));
    assert!(directory_stderr.contains("use `memento ls` or `memento show` instead"));

    stop_server(&mut server);
}

#[test]
fn ls_show_and_find_mark_modified_resources_as_needing_reindex() {
    let temp = tempdir().expect("create temp dir");
    let notes = temp.path().join("notes.txt");

    fs::write(&notes, "first version\nalpha").expect("write notes");

    let mut init = base_command(&temp);
    init.env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("init")
        .assert()
        .success();

    let mut server = start_server(&temp, true);

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["add", "notes.txt"])
        .assert()
        .success();

    fs::write(&notes, "second version\nalpha").expect("rewrite notes");

    let ls_output = base_command(&temp)
        .args(["ls", "mem://resources"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let ls_stdout = strip_ansi(&String::from_utf8(ls_output).expect("stdout is utf-8"));
    assert!(ls_stdout.contains("mem://resources/notes.txt"));
    assert!(ls_stdout.contains("modified"));
    assert!(ls_stdout.contains("run `memento reindex notes.txt`"));

    let show_output = base_command(&temp)
        .args(["show", "mem://resources/notes.txt"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let show_stdout = strip_ansi(&String::from_utf8(show_output).expect("stdout is utf-8"));
    assert!(show_stdout.contains("live_state"));
    assert!(show_stdout.contains("modified"));
    assert!(show_stdout.contains("run `memento reindex notes.txt`"));
    assert!(!show_stdout.contains("second version"));

    let find_output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["find", "first version"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let find_stdout = strip_ansi(&String::from_utf8(find_output).expect("stdout is utf-8"));
    assert!(find_stdout.contains("mem://resources/notes.txt"));
    assert!(find_stdout.contains("state modified"));
    assert!(find_stdout.contains("run `memento reindex notes.txt`"));

    stop_server(&mut server);
}

#[test]
fn reindex_marks_missing_resource_deleted_and_removes_it_from_search() {
    let temp = tempdir().expect("create temp dir");
    let notes = temp.path().join("notes.txt");

    fs::write(&notes, "ephemeral content\nalpha").expect("write notes");

    let mut init = base_command(&temp);
    init.env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("init")
        .assert()
        .success();

    let mut server = start_server(&temp, true);

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["add", "notes.txt"])
        .assert()
        .success();

    fs::remove_file(&notes).expect("remove tracked note");

    let reindex_output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["reindex", "notes.txt"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let reindex_stdout = strip_ansi(&String::from_utf8(reindex_output).expect("stdout is utf-8"));
    assert!(reindex_stdout.contains("refreshed 0 resource(s)"));
    assert!(reindex_stdout.contains("missing notes.txt"));

    let find_output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["find", "ephemeral content"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let find_stdout = strip_ansi(&String::from_utf8(find_output).expect("stdout is utf-8"));
    assert!(find_stdout.contains("status no matches found"));
    assert!(!find_stdout.contains("mem://resources/notes.txt"));

    stop_server(&mut server);
}

#[test]
fn ls_and_show_support_nested_user_and_agent_paths() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp).arg("init").assert().success();

    let mut server = start_server(&temp, true);

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args([
            "remember",
            "--namespace",
            "user",
            "--path",
            "preferences/editor/rust",
            "Use rustfmt defaults",
        ])
        .assert()
        .success();

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args([
            "remember",
            "--namespace",
            "agent",
            "--path",
            "skills/rust/refactor-cli",
            "Run cargo fmt and cargo build after CLI refactors",
        ])
        .assert()
        .success();

    let user_ls = base_command(&temp)
        .args(["ls", "mem://user/preferences"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let user_ls_stdout = strip_ansi(&String::from_utf8(user_ls).expect("stdout is utf-8"));
    assert!(user_ls_stdout.contains("mem://user/preferences/editor"));

    let agent_ls = base_command(&temp)
        .args(["ls", "mem://agent/skills/rust"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let agent_ls_stdout = strip_ansi(&String::from_utf8(agent_ls).expect("stdout is utf-8"));
    assert!(agent_ls_stdout.contains("mem://agent/skills/rust/refactor-cli"));
    assert!(agent_ls_stdout.contains("file"));

    let nested_show = base_command(&temp)
        .args(["show", "mem://agent/skills/rust"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let nested_show_stdout = strip_ansi(&String::from_utf8(nested_show).expect("stdout is utf-8"));
    assert!(nested_show_stdout.contains("mem://agent/skills/rust"));
    assert!(nested_show_stdout.contains("namespace_dir"));

    stop_server(&mut server);
}

#[test]
fn remember_inline_text_updates_existing_item_at_same_uri() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp).arg("init").assert().success();

    let mut server = start_server(&temp, true);

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args([
            "remember",
            "--namespace",
            "agent",
            "--path",
            "skills/update-me",
            "first version",
        ])
        .assert()
        .success();

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args([
            "remember",
            "--namespace",
            "agent",
            "--path",
            "skills/update-me",
            "second version",
        ])
        .assert()
        .success();

    let stored = temp
        .path()
        .join(".memento")
        .join("agent")
        .join("skills")
        .join("update-me.md");
    assert_eq!(
        fs::read_to_string(&stored).expect("stored item is readable"),
        "second version"
    );

    let output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["find", "second version"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = strip_ansi(&String::from_utf8(output).expect("stdout is utf-8"));
    assert!(stdout.contains("mem://agent/skills/update-me"));
    assert!(stdout.contains("preview second version"));
    assert!(!stdout.contains("preview first version"));

    stop_server(&mut server);
}

#[test]
fn remember_file_updates_existing_item_and_keeps_same_uri() {
    let temp = tempdir().expect("create temp dir");
    let source = temp.path().join("source.txt");

    base_command(&temp).arg("init").assert().success();

    let mut server = start_server(&temp, true);

    fs::write(&source, "first imported version").expect("write source file");

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args([
            "remember",
            "--namespace",
            "user",
            "--path",
            "notes/imported-item",
            "--file",
            "source.txt",
        ])
        .assert()
        .success();

    fs::write(&source, "second imported version").expect("rewrite source file");

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args([
            "remember",
            "--namespace",
            "user",
            "--path",
            "notes/imported-item",
            "--file",
            "source.txt",
        ])
        .assert()
        .success();

    let stored = temp
        .path()
        .join(".memento")
        .join("user")
        .join("notes")
        .join("imported-item.txt");
    assert_eq!(
        fs::read_to_string(&stored).expect("stored item is readable"),
        "second imported version"
    );

    let output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["find", "second imported version"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = strip_ansi(&String::from_utf8(output).expect("stdout is utf-8"));
    assert!(stdout.contains("mem://user/notes/imported-item"));
    assert!(stdout.contains("preview second imported version"));
    assert!(!stdout.contains("preview first imported version"));

    stop_server(&mut server);
}
