// These tests run the built binary as a child process, so they bypass the wrappers.
#![allow(clippy::disallowed_methods)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const SPEC_EXAMPLE: &str = "name: autopilot
control: sandbox
agent: claude
image: ghcr.io/shokkunrf/devcontainer-images/base:4.1.0
repos:
  - name: sandbox
    url: git@github.com:shokkunrf/autopilot-sandbox.git
    default_ref: branch:develop
  - name: wd
    url: git@github.com:shokkunrf/wd.git
    default_ref: branch:develop
";

/// A fresh directory under the target directory of the tests.
#[allow(clippy::unwrap_used, clippy::panic)]
fn dir(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("validate")
        .join(test);
    match fs::remove_dir_all(&dir) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => panic!("cannot clean {}: {err}", dir.display()),
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[allow(clippy::unwrap_used)]
fn write(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, content).unwrap();
    path
}

/// Runs `wd validate` in `cwd` with an empty environment.
#[allow(clippy::unwrap_used)]
fn validate(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_wd"))
        .arg("validate")
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .env("HOME", "/nonexistent/home")
        .env("WD_ROOT", "/nonexistent/root")
        .output()
        .unwrap()
}

#[allow(clippy::unwrap_used)]
fn stderr(out: &Output) -> String {
    String::from_utf8(out.stderr.clone()).unwrap()
}

#[allow(clippy::unwrap_used)]
fn assert_passes(out: &Output) {
    assert_eq!(out.status.code(), Some(0), "{}", stderr(out));
    assert!(out.stdout.is_empty());
    assert!(out.stderr.is_empty());
}

fn assert_fails(out: &Output, expected_stderr: &str) {
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert_eq!(stderr(out), expected_stderr);
}

#[test]
fn the_example_of_the_spec_passes_without_output() {
    let dir = dir("example");
    write(&dir, "workspace.yaml", SPEC_EXAMPLE.as_bytes());
    assert_passes(&validate(&dir, &["workspace.yaml"]));
}

#[test]
fn without_a_path_it_reads_workspace_yaml_in_the_current_directory() {
    let dir = dir("default_path");
    assert_eq!(validate(&dir, &[]).status.code(), Some(1));
    write(&dir, "workspace.yaml", SPEC_EXAMPLE.as_bytes());
    assert_passes(&validate(&dir, &[]));
}

#[test]
#[allow(clippy::unwrap_used)]
fn it_reads_only_the_file_and_writes_nothing() {
    let dir = dir("read_only");
    let path = write(&dir, "other.yaml", SPEC_EXAMPLE.as_bytes());
    let before = fs::read_dir(&dir).unwrap().count();
    assert_passes(&validate(&dir, &["other.yaml"]));
    assert_eq!(fs::read_dir(&dir).unwrap().count(), before);
    assert_eq!(fs::read(&path).unwrap(), SPEC_EXAMPLE.as_bytes());
}

#[test]
fn unknown_keys_including_identity_and_features_are_named() {
    let dir = dir("unknown_keys");
    let text = format!("identity: me\nfeatures:\n  - x\n{SPEC_EXAMPLE}    extra: 1\n");
    write(&dir, "workspace.yaml", text.as_bytes());
    assert_fails(
        &validate(&dir, &[]),
        "wd: error: workspace.yaml: unknown key \"identity\"\n\
         wd: error: workspace.yaml: unknown key \"features\"\n\
         wd: error: workspace.yaml: repos[wd]: unknown key \"extra\"\n",
    );
}

#[test]
fn missing_keys_are_listed_with_unknown_keys_one_per_line() {
    let dir = dir("missing_keys");
    write(
        &dir,
        "workspace.yaml",
        b"name: a\nidentity: me\nrepos:\n  - name: x\n    url: \"git@github.com:o/r.git\"\n",
    );
    assert_fails(
        &validate(&dir, &[]),
        "wd: error: workspace.yaml: unknown key \"identity\"\n\
         wd: error: workspace.yaml: missing key \"control\"\n\
         wd: error: workspace.yaml: missing key \"agent\"\n\
         wd: error: workspace.yaml: missing key \"image\"\n\
         wd: error: workspace.yaml: repos[x]: missing key \"default_ref\"\n",
    );
}

#[test]
fn yaml_errors_show_line_column_and_reason() {
    let cases: [(&str, &str); 9] = [
        ("a: &x 1\n", "line 1, column 4: anchors (&) are not allowed"),
        ("a: *x\n", "line 1, column 4: aliases (*) are not allowed"),
        (
            "a: !x 1\n",
            "line 1, column 4: the tag !x is not in the YAML 1.2 core schema",
        ),
        (
            "a: !!binary 1\n",
            "line 1, column 4: the tag !!binary is not in the YAML 1.2 core schema",
        ),
        (
            "a: 1\n---\nb: 2\n",
            "line 2, column 1: a second document is not allowed",
        ),
        ("a: 1\n\"a\": 2\n", "line 2, column 1: duplicate key \"a\""),
        (
            "a:\n b:\n  c:\n   d:\n    e:\n     f:\n      g:\n       h:\n        i: 1\n",
            "line 9, column 9: mappings and sequences are nested deeper than 8 levels",
        ),
        (
            "name: true\n",
            "line 1, column 7: `name` must be a string, but the plain value \"true\" is read as a boolean; write it in quotes to use it as a string",
        ),
        (
            "name: 010\n",
            "line 1, column 7: `name` must be a string, but the plain value \"010\" is read as an integer; write it in quotes to use it as a string",
        ),
    ];
    for (index, (text, reason)) in cases.iter().enumerate() {
        let dir = dir(&format!("yaml_error_{index}"));
        write(&dir, "workspace.yaml", text.as_bytes());
        assert_fails(
            &validate(&dir, &[]),
            &format!("wd: error: workspace.yaml: {reason}\n"),
        );
    }
}

#[test]
fn quotes_flow_style_crlf_and_str_tags_are_accepted() {
    let dir = dir("accepted_styles");
    let quoted = SPEC_EXAMPLE
        .replace("name: autopilot", "name: \"autopilot\"")
        .replace("agent: claude", "agent: !!str claude");
    write(&dir, "quoted.yaml", quoted.as_bytes());
    assert_passes(&validate(&dir, &["quoted.yaml"]));

    let crlf = SPEC_EXAMPLE.replace('\n', "\r\n");
    write(&dir, "crlf.yaml", crlf.as_bytes());
    assert_passes(&validate(&dir, &["crlf.yaml"]));

    let flow = "{name: autopilot, control: sandbox, agent: claude, \
        image: 'ghcr.io/shokkunrf/devcontainer-images/base:4.1.0', \
        repos: [{name: sandbox, url: 'git@github.com:shokkunrf/autopilot-sandbox.git', \
        default_ref: 'branch:develop'}]}\n";
    write(&dir, "flow.yaml", flow.as_bytes());
    assert_passes(&validate(&dir, &["flow.yaml"]));
}

#[test]
fn a_missing_file_a_directory_and_a_big_file_fail() {
    let dir = dir("unreadable");
    let out = validate(&dir, &["missing.yaml"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).starts_with("wd: error: cannot read missing.yaml: "));

    let out = validate(&dir, &["."]);
    assert_fails(&out, "wd: error: cannot read .: not a regular file\n");

    let big = format!("name: {}\n", "x".repeat(1 << 20));
    write(&dir, "big.yaml", big.as_bytes());
    assert_fails(
        &validate(&dir, &["big.yaml"]),
        &format!(
            "wd: error: big.yaml: the file is {} bytes, over the limit of 1048576 bytes (1 MiB)\n",
            big.len()
        ),
    );
}

#[test]
fn invalid_utf8_fails_with_the_position() {
    let dir = dir("utf8");
    write(&dir, "workspace.yaml", b"name: a\ncontrol: \x82\xa0\n");
    assert_fails(
        &validate(&dir, &[]),
        "wd: error: workspace.yaml: line 2, column 10: the file is not valid UTF-8\n",
    );
}

#[test]
fn extra_arguments_and_options_fail() {
    let dir = dir("args");
    write(&dir, "workspace.yaml", SPEC_EXAMPLE.as_bytes());
    assert_fails(
        &validate(&dir, &["workspace.yaml", "extra"]),
        "wd: error: unexpected argument \"extra\"\n",
    );
    assert_fails(
        &validate(&dir, &["--frobnicate"]),
        "wd: error: invalid option '--frobnicate'\n",
    );
}
