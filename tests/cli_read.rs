mod support;

use std::fs;

use tempfile::tempdir;

use support::*;

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
            "mem://user/preferences/style.md",
            "Prefer concise explanations",
        ])
        .assert()
        .success();

    let output = base_command(&temp)
        .args(["cat", "mem://user/preferences/style.md"])
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
            "mem://user/preferences/style.md",
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
    assert!(reindex_stdout.contains("removed notes.txt"));

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
            "mem://user/preferences/editor/rust.md",
            "Use rustfmt defaults",
        ])
        .assert()
        .success();

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args([
            "remember",
            "mem://agent/skills/rust/refactor-cli.md",
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
    assert!(agent_ls_stdout.contains("mem://agent/skills/rust/refactor-cli.md"));
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
