mod support;

use std::fs;

use tempfile::tempdir;

use support::*;

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
            "mem://agent/skills/sample-copy",
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
        .args(["show", "mem://agent/skills/sample-copy.txt"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = strip_ansi(&String::from_utf8(output).expect("stdout is utf-8"));
    assert!(stdout.contains("mem://agent/skills/sample-copy.txt"));
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
            "mem://user/preferences/writing-style.md",
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
        .args(["show", "mem://user/preferences/writing-style.md"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = strip_ansi(&String::from_utf8(output).expect("stdout is utf-8"));
    assert!(stdout.contains("mem://user/preferences/writing-style.md"));
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
            "mem://agent/skills/binary-copy",
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
fn remember_inline_text_requires_markdown_uri() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp).arg("init").assert().success();

    let mut server = start_server(&temp, true);

    let output = base_command(&temp)
        .args([
            "remember",
            "mem://user/preferences/writing-style",
            "plain text",
        ])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();

    let stderr = strip_ansi(&String::from_utf8(output).expect("stderr is utf-8"));
    assert!(stderr.contains("inline text requires a Markdown destination URI ending in `.md`"));
    assert!(stderr.contains("mem://user/notes/todo.md"));

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
            "mem://agent/skills/to-remove.md",
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
        .args(["forget", "mem://agent/skills/to-remove.md"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let forget_stdout = strip_ansi(&String::from_utf8(forget_output).expect("stdout is utf-8"));
    assert!(forget_stdout.contains("item removed"));
    assert!(forget_stdout.contains("mem://agent/skills/to-remove.md"));
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
fn forget_removes_empty_agent_directory() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("init")
        .assert()
        .success();

    let dir_path = temp
        .path()
        .join(".memento")
        .join("agent")
        .join("playbook")
        .join("test");
    fs::create_dir_all(&dir_path).expect("create empty agent directory");

    let mut server = start_server(&temp, true);

    let forget_output = base_command(&temp)
        .args(["forget", "mem://agent/playbook/test"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let forget_stdout = strip_ansi(&String::from_utf8(forget_output).expect("stdout is utf-8"));
    assert!(forget_stdout.contains("item removed"));
    assert!(forget_stdout.contains("mem://agent/playbook/test"));
    assert!(!dir_path.exists());

    stop_server(&mut server);
}

#[test]
fn forget_rejects_agent_directory_that_contains_files() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("init")
        .assert()
        .success();

    let file_path = temp
        .path()
        .join(".memento")
        .join("agent")
        .join("playbook")
        .join("test")
        .join("note.md");
    fs::create_dir_all(file_path.parent().expect("file parent exists"))
        .expect("create agent directory tree");
    fs::write(&file_path, "keep me").expect("write agent file");

    let mut server = start_server(&temp, true);

    let output = base_command(&temp)
        .args(["forget", "mem://agent/playbook/test"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();

    let stderr = strip_ansi(&String::from_utf8(output).expect("stderr is utf-8"));
    assert!(stderr.contains("directory contains files; forget files individually first"));
    assert!(file_path.exists());

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
            "mem://agent/skills/update-me.md",
            "first version",
        ])
        .assert()
        .success();

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args([
            "remember",
            "mem://agent/skills/update-me.md",
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
    assert!(stdout.contains("mem://agent/skills/update-me.md"));
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
            "mem://user/notes/imported-item",
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
            "mem://user/notes/imported-item",
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
    assert!(stdout.contains("mem://user/notes/imported-item.txt"));
    assert!(stdout.contains("preview second imported version"));
    assert!(!stdout.contains("preview first imported version"));

    stop_server(&mut server);
}
