mod support;

use std::fs;

use tempfile::tempdir;

use support::*;

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
