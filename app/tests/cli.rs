// These tests run the built binary as a child process, so they bypass the wrappers.
#![allow(clippy::disallowed_methods)]

use std::fs::OpenOptions;
use std::process::{Command, Output, Stdio};

#[allow(clippy::unwrap_used)]
fn wd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_wd"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn version_prints_only_the_version_to_stdout() {
    let out = wd(&["--version"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        concat!(env!("CARGO_PKG_VERSION"), "\n")
    );
    assert!(out.stderr.is_empty());
}

#[test]
fn no_command_exits_1_with_the_reason_on_stderr() {
    let out = wd(&[]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert_eq!(
        String::from_utf8(out.stderr).unwrap(),
        "wd: error: missing command\n"
    );
}

#[test]
fn unknown_option_exits_1_naming_the_option() {
    let out = wd(&["--frobnicate"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert_eq!(
        String::from_utf8(out.stderr).unwrap(),
        "wd: error: invalid option '--frobnicate'\n"
    );
}

#[test]
fn unknown_command_exits_1_naming_the_command() {
    let out = wd(&["up"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert_eq!(
        String::from_utf8(out.stderr).unwrap(),
        "wd: error: unknown command \"up\"\n"
    );
}

#[test]
fn closed_stdout_pipe_exits_0_quietly() {
    let (reader, writer) = std::io::pipe().unwrap();
    drop(reader);
    let out = Command::new(env!("CARGO_BIN_EXE_wd"))
        .arg("--version")
        .stdout(writer)
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stderr.is_empty());
}

#[test]
fn unwritable_stdout_exits_1_instead_of_panicking() {
    // std treats writes to a closed fd as success, so use /dev/full, which fails every write with ENOSPC.
    let full = OpenOptions::new().write(true).open("/dev/full").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_wd"))
        .arg("--version")
        .stdout(Stdio::from(full))
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.starts_with("wd: error: cannot write to stdout: "),
        "{stderr}"
    );
}
