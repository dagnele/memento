#![allow(dead_code)]

use std::fs;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use assert_cmd::Command;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::OnceLock;

pub struct RunningServer {
    child: Option<std::process::Child>,
}

impl RunningServer {
    fn new(child: std::process::Child) -> Self {
        Self { child: Some(child) }
    }

    pub fn kill(&mut self) -> std::io::Result<()> {
        if let Some(child) = self.child.as_mut() {
            child.kill()
        } else {
            Ok(())
        }
    }

    pub fn wait(&mut self) -> std::io::Result<()> {
        if let Some(mut child) = self.child.take() {
            child.wait().map(|_| ())
        } else {
            Ok(())
        }
    }

    pub fn stop(&mut self) {
        let _ = self.kill();
        let _ = self.wait();
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn strip_ansi(input: &str) -> String {
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

pub fn fixture_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(relative)
}

pub fn base_command(temp: &tempfile::TempDir) -> Command {
    let mut command = Command::cargo_bin("memento").expect("binary exists");
    command
        .current_dir(temp.path())
        .env("MEMENTO_MODEL_CACHE_DIR", temp.path().join("model-cache"));
    command
}

pub fn start_server(temp: &tempfile::TempDir, test_embedding: bool) -> RunningServer {
    start_server_with_args(temp, test_embedding, &[])
}

pub fn start_server_with_args(
    temp: &tempfile::TempDir,
    test_embedding: bool,
    extra_args: &[&str],
) -> RunningServer {
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
        .args(extra_args)
        .stdout(std::fs::File::create(&stdout_path).expect("create server stdout log"))
        .stderr(std::fs::File::create(&stderr_path).expect("create server stderr log"));

    if test_embedding {
        command.env("MEMENTO_TEST_EMBEDDING", "1");
    }

    let mut child = command.spawn().expect("start server");
    wait_for_server(&mut child, port, &stdout_path, &stderr_path);
    RunningServer::new(child)
}

pub fn start_mcp_server(temp: &tempfile::TempDir) -> (RunningServer, u16) {
    use std::process::Command as ProcessCommand;

    let exe = assert_cmd::cargo::cargo_bin("memento");
    let port = reserve_port();
    let stdout_path = temp.path().join("mcp.stdout.log");
    let stderr_path = temp.path().join("mcp.stderr.log");
    let mut command = ProcessCommand::new(exe);
    command
        .current_dir(temp.path())
        .env("MEMENTO_MODEL_CACHE_DIR", temp.path().join("model-cache"))
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["mcp", "--transport", "http", "--port", &port.to_string()])
        .stdout(std::fs::File::create(&stdout_path).expect("create mcp stdout log"))
        .stderr(std::fs::File::create(&stderr_path).expect("create mcp stderr log"));

    let mut child = command.spawn().expect("start mcp server");
    wait_for_server(&mut child, port, &stdout_path, &stderr_path);
    (RunningServer::new(child), port)
}

pub fn stop_server(server: &mut RunningServer) {
    server.stop();
}

// Allocate ports from a process-specific range so separate `cargo test`
// processes do not collide with each other.
static NEXT_PORT: OnceLock<AtomicU16> = OnceLock::new();

pub fn reserve_port() -> u16 {
    let base = 10_000 + ((std::process::id() % 500) as u16) * 100;
    NEXT_PORT
        .get_or_init(|| AtomicU16::new(base))
        .fetch_add(1, Ordering::Relaxed)
}

pub fn write_server_port(temp: &tempfile::TempDir, port: u16) {
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

pub fn replace_config_line(temp: &tempfile::TempDir, prefix: &str, replacement: &str) {
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

pub fn wait_for_server(
    child: &mut std::process::Child,
    port: u16,
    stdout_path: &Path,
    stderr_path: &Path,
) {
    let deadline = Instant::now() + Duration::from_secs(15);

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

    let _ = child.kill();
    let _ = child.wait();
    let stdout = fs::read_to_string(stdout_path).unwrap_or_default();
    let stderr = fs::read_to_string(stderr_path).unwrap_or_default();
    panic!("server did not start on port {port}\nstdout:\n{stdout}\nstderr:\n{stderr}");
}
