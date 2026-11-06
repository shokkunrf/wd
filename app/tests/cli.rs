// 実行ファイルを外から呼ぶので、外部コマンドとファイルシステムのラッパを通さない
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
    let expected = option_env!("WD_VERSION").unwrap_or("0.0.0-dev");
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        format!("{expected}\n")
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
        "wd: コマンドが無い\n"
    );
}

#[test]
fn unknown_option_exits_1_naming_the_option() {
    let out = wd(&["--frobnicate"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert_eq!(
        String::from_utf8(out.stderr).unwrap(),
        "wd: 知らないオプション: --frobnicate\n"
    );
}

#[test]
fn unknown_command_exits_1_naming_the_command() {
    let out = wd(&["up"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert_eq!(
        String::from_utf8(out.stderr).unwrap(),
        "wd: 知らないコマンド: up\n"
    );
}

#[test]
fn unwritable_stdout_exits_1_instead_of_panicking() {
    // 閉じたfdへの書き込みはstdが成功として扱うので、書くたびにENOSPCを返す/dev/fullを使う
    let full = OpenOptions::new().write(true).open("/dev/full").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_wd"))
        .arg("--version")
        .stdout(Stdio::from(full))
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.starts_with("wd: 標準出力に書けない: "), "{stderr}");
}
