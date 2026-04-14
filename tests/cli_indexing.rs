mod support;

use std::fs;

use tempfile::tempdir;

use support::*;

#[test]
fn add_indexes_binary_input_as_metadata_only() {
    let temp = tempdir().expect("create temp dir");
    let source = fixture_path("binary/sample.bin");
    let copied = temp.path().join("sample.bin");

    fs::copy(&source, &copied).expect("copy fixture");

    base_command(&temp).arg("init").assert().success();

    let mut server = start_server(&temp, true);

    let output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["add", "sample.bin"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = strip_ansi(&String::from_utf8(output).expect("stdout is utf-8"));
    assert!(stdout.contains("indexed 1 resource(s)"));
    assert!(stdout.contains("added sample.bin metadata only"));

    let show_output = base_command(&temp)
        .args(["show", "mem://resources/sample.bin"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let show_stdout = strip_ansi(&String::from_utf8(show_output).expect("stdout is utf-8"));
    assert!(show_stdout.contains("mem://resources/sample.bin"));
    assert!(show_stdout.contains("layers"));
    assert!(show_stdout.contains("detail:metadata"));

    let find_output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["find", "binary file sample.bin"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let find_stdout = strip_ansi(&String::from_utf8(find_output).expect("stdout is utf-8"));
    assert!(find_stdout.contains("mem://resources/sample.bin"));
    assert!(find_stdout.contains("preview File name: sample.bin | File type: Binary file"));

    stop_server(&mut server);
}

#[test]
fn reindex_keeps_binary_resource_as_metadata_only() {
    let temp = tempdir().expect("create temp dir");
    let source = fixture_path("binary/sample.bin");
    let copied = temp.path().join("sample.bin");

    fs::copy(&source, &copied).expect("copy fixture");

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("init")
        .assert()
        .success();

    let mut server = start_server(&temp, true);

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["add", "sample.bin"])
        .assert()
        .success();

    let output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["reindex", "sample.bin"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = strip_ansi(&String::from_utf8(output).expect("stdout is utf-8"));
    assert!(stdout.contains("refreshed 1 resource(s)"));
    assert!(stdout.contains("reindexed sample.bin metadata only"));

    stop_server(&mut server);
}

#[test]
fn reindex_promotes_binary_resource_when_file_becomes_text() {
    let temp = tempdir().expect("create temp dir");
    let source = fixture_path("binary/sample.bin");
    let copied = temp.path().join("sample.bin");

    fs::copy(&source, &copied).expect("copy fixture");

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("init")
        .assert()
        .success();

    let mut server = start_server(&temp, true);

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["add", "sample.bin"])
        .assert()
        .success();

    fs::write(&copied, "converted to text\nhello world").expect("rewrite file as text");

    let reindex_output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["reindex", "sample.bin"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let reindex_stdout = strip_ansi(&String::from_utf8(reindex_output).expect("stdout is utf-8"));
    assert!(reindex_stdout.contains("reindexed sample.bin"));
    assert!(!reindex_stdout.contains("metadata only"));

    let find_output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["find", "converted to text"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let find_stdout = strip_ansi(&String::from_utf8(find_output).expect("stdout is utf-8"));
    assert!(find_stdout.contains("mem://resources/sample.bin"));
    assert!(find_stdout.contains("preview converted to text | hello world"));

    let show_output = base_command(&temp)
        .args(["show", "mem://resources/sample.bin"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let show_stdout = strip_ansi(&String::from_utf8(show_output).expect("stdout is utf-8"));
    assert!(show_stdout.contains("detail:disk"));
    assert!(!show_stdout.contains("detail:metadata"));

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
fn server_startup_reindexes_modified_agent_skill_file() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("init")
        .assert()
        .success();

    let skill_path = temp
        .path()
        .join(".memento")
        .join("agent")
        .join("skills")
        .join("PLAY.md");

    fs::write(&skill_path, "startup version one").expect("write skill file");

    let mut server = start_server(&temp, true);
    stop_server(&mut server);

    fs::write(&skill_path, "startup version two").expect("rewrite skill file");

    let mut server = start_server(&temp, true);

    let find_output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["find", "startup version two"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let find_stdout = strip_ansi(&String::from_utf8(find_output).expect("stdout is utf-8"));
    assert!(find_stdout.contains("mem://agent/skills/PLAY.md"));
    assert!(find_stdout.contains("preview startup version two"));

    stop_server(&mut server);
}

#[test]
fn reindex_refreshes_modified_agent_skill_file_via_server() {
    let temp = tempdir().expect("create temp dir");

    base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .arg("init")
        .assert()
        .success();

    let skill_path = temp
        .path()
        .join(".memento")
        .join("agent")
        .join("skills")
        .join("memento.md");

    let mut server = start_server(&temp, true);

    fs::write(&skill_path, "# Memento\n\nManual reindex skill text").expect("rewrite skill file");

    let reindex_output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["reindex", ".memento/agent/skills/memento.md"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let reindex_stdout = strip_ansi(&String::from_utf8(reindex_output).expect("stdout is utf-8"));
    assert!(reindex_stdout.contains("refreshed 1 resource(s)"));
    assert!(reindex_stdout.contains("reindexed .memento/agent/skills/memento.md"));

    let find_output = base_command(&temp)
        .env("MEMENTO_TEST_EMBEDDING", "1")
        .args(["find", "Manual reindex skill text"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let find_stdout = strip_ansi(&String::from_utf8(find_output).expect("stdout is utf-8"));
    assert!(find_stdout.contains("mem://agent/skills/memento.md"));
    assert!(find_stdout.contains("preview # Memento | Manual reindex skill text"));

    stop_server(&mut server);
}
