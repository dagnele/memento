mod support;

use std::fs;

use tempfile::tempdir;

use support::*;

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
            .join("memento")
            .join("SKILL.md")
            .exists()
    );
}

#[test]
fn root_help_lists_command_descriptions_and_quick_start() {
    let temp = tempdir().expect("create temp dir");

    let output = base_command(&temp)
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = strip_ansi(&String::from_utf8(output).expect("stdout is utf-8"));
    assert!(stdout.contains("Index local project files, store durable user or agent notes"));
    assert!(stdout.contains("init      Create a .memento workspace in the current directory"));
    assert!(stdout.contains("serve     Run the local server used by indexing and search commands"));
    assert!(stdout.contains("add       Index local text files into mem://resources"));
    assert!(
        stdout.contains("find      Search indexed resources and memory by semantic similarity")
    );
    assert!(stdout.contains("Quick start:"));
    assert!(stdout.contains("memento init"));
    assert!(stdout.contains("memento serve"));
    assert!(stdout.contains("memento add \"docs/**/*.md\""));
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
