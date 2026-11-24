// These tests run the built binary as a child process, so they bypass the wrappers.
#![allow(clippy::disallowed_methods)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A fresh directory outside any git working tree, since the root must not be inside one.
struct Dir(PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        // A leftover in the temporary directory is harmless, so a failure is not reported.
        #[allow(clippy::let_underscore_must_use)]
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl std::ops::Deref for Dir {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

#[allow(clippy::unwrap_used)]
fn dir(test: &str) -> Dir {
    let dir = std::env::temp_dir().join(format!("wd-init-{}-{test}", std::process::id()));
    fs::create_dir_all(dir.join("home")).unwrap();
    Dir(dir)
}

fn command(dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_wd"));
    command
        .arg("init")
        .args(args)
        .env_clear()
        .env("HOME", dir.join("home"));
    command
}

#[allow(clippy::unwrap_used)]
fn init(dir: &Path, args: &[&str]) -> Output {
    command(dir, args).output().unwrap()
}

#[allow(clippy::unwrap_used)]
fn stderr(out: &Output) -> String {
    String::from_utf8(out.stderr.clone()).unwrap()
}

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

#[allow(clippy::unwrap_used)]
fn text(path: &Path) -> &str {
    path.to_str().unwrap()
}

#[test]
fn it_creates_the_root_controls_and_workspaces() {
    let dir = dir("creates");
    let root = dir.join("a/b/root");
    assert_passes(&init(&dir, &["--root", text(&root)]));
    assert!(root.join("controls").is_dir());
    assert!(root.join("workspaces").is_dir());
}

#[test]
fn a_second_run_with_the_same_root_passes() {
    let dir = dir("again");
    let root = dir.join("root");
    assert_passes(&init(&dir, &["--root", text(&root)]));
    fs::write(root.join("workspaces/marker"), "").unwrap();
    assert_passes(&init(&dir, &[&format!("--root={}", text(&root))]));
    assert!(root.join("workspaces/marker").is_file());
}

#[test]
fn a_relative_root_is_refused_and_nothing_is_created() {
    let dir = dir("relative");
    let out = command(&dir, &["--root", "root"])
        .current_dir(&*dir)
        .output()
        .unwrap();
    assert_fails(
        &out,
        "wd: error: the root \"root\" from --root is not an absolute path; give a path that starts with \"/\"\n",
    );
    assert!(!dir.join("root").exists());
}

#[test]
fn a_root_with_a_control_character_is_refused() {
    let dir = dir("control_char");
    let root = format!("{}/a\nb", text(&dir));
    let out = init(&dir, &["--root", &root]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("has a control character"));
    assert_eq!(fs::read_dir(&*dir).unwrap().count(), 1);
}

#[test]
fn a_git_directory_or_file_above_the_root_is_refused_and_nothing_is_created() {
    let dir = dir("git_above");
    fs::create_dir_all(dir.join("repo/.git")).unwrap();
    fs::create_dir_all(dir.join("worktree")).unwrap();
    fs::write(dir.join("worktree/.git"), "gitdir: /elsewhere\n").unwrap();
    for parent in ["repo", "worktree"] {
        let root = dir.join(parent).join("sub/root");
        let out = init(&dir, &["--root", text(&root)]);
        assert_fails(
            &out,
            &format!(
                "wd: error: the root \"{}\" is inside a git working tree, since \"{}/{parent}/.git\" exists; remove that .git or choose a root outside any git working tree\n",
                text(&root),
                text(&dir)
            ),
        );
        assert!(!dir.join(parent).join("sub").exists());
    }
}

#[test]
fn a_git_directory_above_the_realpath_of_the_root_is_refused() {
    let dir = dir("git_above_realpath");
    fs::create_dir_all(dir.join("repo/.git")).unwrap();
    fs::create_dir_all(dir.join("repo/deep/target")).unwrap();
    std::os::unix::fs::symlink(dir.join("repo/deep/target"), dir.join("link")).unwrap();
    let root = dir.join("link/root");
    let out = init(&dir, &["--root", text(&root)]);
    let found = fs::canonicalize(dir.join("repo")).unwrap().join(".git");
    assert_fails(
        &out,
        &format!(
            "wd: error: the root \"{}\" is inside a git working tree, since \"{}\" exists; remove that .git or choose a root outside any git working tree\n",
            text(&root),
            text(&found)
        ),
    );
    assert!(!dir.join("repo/deep/target/root").exists());
}

#[test]
fn a_git_directory_reached_through_a_missing_component_and_dot_dot_is_refused() {
    let dir = dir("git_after_dot_dot");
    fs::create_dir_all(dir.join("g/.git")).unwrap();
    let root = format!("{}/nonexist/../g", text(&dir));
    let out = init(&dir, &["--root", &root]);
    let found = fs::canonicalize(dir.join("g")).unwrap().join(".git");
    assert_fails(
        &out,
        &format!(
            "wd: error: the root \"{root}\" is inside a git working tree, since \"{}\" exists; remove that .git or choose a root outside any git working tree\n",
            text(&found)
        ),
    );
    assert!(!dir.join("nonexist").exists());
    assert!(!dir.join("g/controls").exists());
}

#[test]
fn a_symlink_in_place_of_controls_is_refused() {
    let dir = dir("controls_symlink");
    let root = dir.join("root");
    fs::create_dir_all(dir.join("elsewhere")).unwrap();
    fs::create_dir_all(&root).unwrap();
    std::os::unix::fs::symlink(dir.join("elsewhere"), root.join("controls")).unwrap();
    assert_fails(
        &init(&dir, &["--root", text(&root)]),
        &format!(
            "wd: error: \"{}/controls\" is a symlink, which wd does not follow; check where it points and remove it by hand\n",
            text(&root)
        ),
    );
}

#[test]
fn a_file_in_place_of_workspaces_is_refused() {
    let dir = dir("workspaces_file");
    let root = dir.join("root");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("workspaces"), "").unwrap();
    assert_fails(
        &init(&dir, &["--root", text(&root)]),
        &format!(
            "wd: error: \"{}/workspaces\" is not a directory; check what it is and remove it by hand\n",
            text(&root)
        ),
    );
}

#[test]
fn the_root_flag_is_required_once_and_nothing_else_is_accepted() {
    let dir = dir("args");
    let root = dir.join("root");
    assert_fails(
        &init(&dir, &[]),
        "wd: error: wd init needs --root <dir>; give the absolute path of the directory where wd keeps controls/ and workspaces/\n",
    );
    assert_fails(
        &init(&dir, &["--root", text(&root), "--root", text(&root)]),
        "wd: error: --root is given more than once; give it once\n",
    );
    assert_fails(
        &init(&dir, &["extra"]),
        "wd: error: unexpected argument \"extra\"\n",
    );
    assert_fails(
        &init(&dir, &["--name", "x"]),
        "wd: error: invalid option '--name'\n",
    );
    assert!(!root.exists());
}

#[test]
fn inside_a_container_it_runs_only_on_the_host() {
    let dir = dir("container");
    let root = dir.join("root");
    let out = command(&dir, &["--root", text(&root)])
        .env("WD_WORKSPACE", "x")
        .output()
        .unwrap();
    assert_fails(
        &out,
        "wd: error: `wd init` runs only on the host, but WD_WORKSPACE is set, which marks the inside of a container; run it on the host, or unset WD_WORKSPACE if this is the host\n",
    );
    assert!(!root.exists());
}
