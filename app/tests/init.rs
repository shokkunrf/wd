// These tests run the built binary as a child process, so they bypass the wrappers.
#![allow(clippy::disallowed_methods)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
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

fn config_dir(dir: &Path) -> PathBuf {
    dir.join("home/.config/wd")
}

fn config_yaml(dir: &Path) -> PathBuf {
    config_dir(dir).join("config.yaml")
}

#[allow(clippy::unwrap_used)]
fn mode(path: &Path) -> u32 {
    fs::symlink_metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn it_creates_the_root_its_directories_and_config_yaml() {
    let dir = dir("creates");
    let root = dir.join("a/b/root");
    assert_passes(&init(&dir, &["--root", text(&root)]));
    assert!(root.join("controls").is_dir());
    assert!(root.join("workspaces").is_dir());
    assert_eq!(
        fs::read_to_string(config_yaml(&dir)).unwrap(),
        format!("root: {}\n", text(&root))
    );
    assert_eq!(mode(&config_dir(&dir)), 0o700);
    assert_eq!(mode(&config_yaml(&dir)), 0o600);
}

#[test]
fn a_second_run_with_the_same_root_passes_and_changes_nothing() {
    let dir = dir("again");
    let root = dir.join("root");
    assert_passes(&init(&dir, &["--root", text(&root)]));
    fs::write(root.join("workspaces/marker"), "").unwrap();
    let before = fs::read(config_yaml(&dir)).unwrap();
    assert_passes(&init(&dir, &[&format!("--root={}", text(&root))]));
    assert_passes(&init(&dir, &["--root", &format!("{}/", text(&root))]));
    assert!(root.join("workspaces/marker").is_file());
    assert_eq!(fs::read(config_yaml(&dir)).unwrap(), before);
}

#[test]
fn a_different_root_is_refused_showing_both_and_nothing_changes() {
    let dir = dir("conflict");
    let root = dir.join("root");
    let other = dir.join("other");
    assert_passes(&init(&dir, &["--root", text(&root)]));
    let before = fs::read(config_yaml(&dir)).unwrap();
    let config = text(&config_yaml(&dir)).to_owned();
    assert_fails(
        &init(&dir, &["--root", text(&other)]),
        &format!(
            "wd: error: \"{config}\" records the root \"{}\", but --root gives \"{}\"; nothing was written; to change the root, edit \"{config}\" yourself\n",
            text(&root),
            text(&other)
        ),
    );
    assert!(!other.exists());
    assert_eq!(fs::read(config_yaml(&dir)).unwrap(), before);
}

#[test]
fn a_root_that_needs_quotes_is_written_quoted() {
    let dir = dir("quoted");
    let root = dir.join("a #b");
    assert_passes(&init(&dir, &["--root", text(&root)]));
    assert_eq!(
        fs::read_to_string(config_yaml(&dir)).unwrap(),
        format!("root: \"{}\"\n", text(&root))
    );
    assert_passes(&init(&dir, &["--root", text(&root)]));
}

#[test]
fn a_symlink_in_place_of_config_yaml_is_refused_and_nothing_is_written() {
    let dir = dir("config_symlink");
    let root = dir.join("root");
    fs::create_dir_all(config_dir(&dir)).unwrap();
    fs::write(dir.join("target.yaml"), format!("root: {}\n", text(&root))).unwrap();
    std::os::unix::fs::symlink(dir.join("target.yaml"), config_yaml(&dir)).unwrap();
    assert_fails(
        &init(&dir, &["--root", text(&root)]),
        &format!(
            "wd: error: \"{}\" is a symlink, which wd does not follow; check where it points and remove it by hand\n",
            text(&config_yaml(&dir))
        ),
    );
    assert!(!root.exists());
}

#[test]
fn an_invalid_config_yaml_is_reported_by_line_and_column() {
    let dir = dir("config_invalid");
    let root = dir.join("root");
    fs::create_dir_all(config_dir(&dir)).unwrap();
    fs::write(config_yaml(&dir), "rot: /a\nroot: true\n").unwrap();
    let config = text(&config_yaml(&dir)).to_owned();
    assert_fails(
        &init(&dir, &["--root", text(&root)]),
        &format!(
            "wd: error: {config}: line 1, column 1: unknown key `rot`\n\
             wd: error: {config}: line 2, column 7: `root` must be a string, but the plain value \"true\" is read as a boolean; write it in quotes to use it as a string\n"
        ),
    );
    assert!(!root.exists());
}

#[test]
fn without_home_nothing_is_created() {
    let dir = dir("no_home");
    let root = dir.join("root");
    for home in [None, Some(""), Some("home")] {
        let mut command = command(&dir, &["--root", text(&root)]);
        command.env_remove("HOME");
        if let Some(home) = home {
            command.env("HOME", home);
        }
        assert_fails(
            &command.output().unwrap(),
            "wd: error: HOME is not set to an absolute path, so wd cannot find ~/.config/wd; set HOME\n",
        );
        assert!(!root.exists());
    }
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
    assert!(!config_dir(&dir).exists());
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
        assert!(!config_dir(&dir).exists());
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
    assert!(!config_dir(&dir).exists());
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
