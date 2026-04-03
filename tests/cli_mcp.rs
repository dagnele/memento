mod support;

use std::fs;
use std::net::TcpStream;

use reqwest::blocking::Client;
use tempfile::tempdir;

use support::*;

#[test]
fn mcp_http_is_reachable_on_its_port() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("init")
        .assert()
        .success();

    let (mut mcp_server, mcp_port) = start_mcp_server(&temp);

    assert!(TcpStream::connect(("127.0.0.1", mcp_port)).is_ok());

    stop_server(&mut mcp_server);
}

#[test]
fn mcp_tool_list_is_available_on_server_port() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("init")
        .assert()
        .success();

    let (mut mcp_server, mcp_port) = start_mcp_server(&temp);

    let client = Client::builder().build().expect("build reqwest client");
    let response = client
        .post(format!("http://127.0.0.1:{}", mcp_port))
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
                        "version": "0.1.1"
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
        .post(format!("http://127.0.0.1:{}", mcp_port))
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

    stop_server(&mut mcp_server);
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

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["add", "notes.txt"])
        .assert()
        .success();

    stop_server(&mut server);

    let (mut mcp_server, mcp_port) = start_mcp_server(&temp);
    let client = Client::builder().build().expect("build reqwest client");

    let initialize = client
        .post(format!("http://127.0.0.1:{}", mcp_port))
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
                        "version": "0.1.1"
                    }
                }
            })
            .to_string(),
        )
        .send()
        .expect("send initialize request");

    assert!(initialize.status().is_success());

    let response = client
        .post(format!("http://127.0.0.1:{}", mcp_port))
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
        .post(format!("http://127.0.0.1:{}", mcp_port))
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

    stop_server(&mut mcp_server);
}

#[test]
fn mcp_resource_templates_expose_mem_namespace_patterns() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("init")
        .assert()
        .success();

    let (mut mcp_server, mcp_port) = start_mcp_server(&temp);
    let client = Client::builder().build().expect("build reqwest client");

    let initialize = client
        .post(format!("http://127.0.0.1:{}", mcp_port))
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
                        "version": "0.1.1"
                    }
                }
            })
            .to_string(),
        )
        .send()
        .expect("send initialize request");

    assert!(initialize.status().is_success());

    let response = client
        .post(format!("http://127.0.0.1:{}", mcp_port))
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

    stop_server(&mut mcp_server);
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

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["add", "notes.txt"])
        .assert()
        .success();

    stop_server(&mut server);

    let (mut mcp_server, mcp_port) = start_mcp_server(&temp);
    let client = Client::builder().build().expect("build reqwest client");

    let initialize = client
        .post(format!("http://127.0.0.1:{}", mcp_port))
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
                        "version": "0.1.1"
                    }
                }
            })
            .to_string(),
        )
        .send()
        .expect("send initialize request");

    assert!(initialize.status().is_success());

    let response = client
        .post(format!("http://127.0.0.1:{}", mcp_port))
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

    stop_server(&mut mcp_server);
}

#[test]
fn mcp_dir_flag_overrides_working_directory() {
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
    let stdout_path = workspace.path().join("mcp.stdout.log");
    let stderr_path = workspace.path().join("mcp.stderr.log");

    let mut mcp_server = ProcessCommand::new(exe)
        .current_dir(wrong_cwd.path())
        .env(
            "MEMENTO_MODEL_CACHE_DIR",
            workspace.path().join("model-cache"),
        )
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args([
            "mcp",
            "--transport",
            "http",
            "--port",
            &port.to_string(),
            "--dir",
            workspace.path().to_str().expect("workspace path is utf-8"),
        ])
        .stdout(std::fs::File::create(&stdout_path).expect("create mcp stdout log"))
        .stderr(std::fs::File::create(&stderr_path).expect("create mcp stderr log"))
        .spawn()
        .expect("start mcp server");

    wait_for_server(&mut mcp_server, port, &stdout_path, &stderr_path);

    assert!(TcpStream::connect(("127.0.0.1", port)).is_ok());

    let _ = mcp_server.kill();
    let _ = mcp_server.wait();
}
