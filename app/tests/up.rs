// These tests run the built binary and real git as child processes, so they bypass the wrappers.
#![allow(clippy::disallowed_methods)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const WORKSPACE_YAML: &str = "name: demo
control: control
agent: claude
repos:
  - name: control
    url: https://github.com/me/control.git
    default_ref: branch:develop
";

const LEDGER: &str = "identities:
  me:
    type: github-user
    login: me
    email: me@example.com
  bot:
    type: github-app
    app_id: 1
    installation_id: 2
    pem: /nonexistent/bot.pem
    login: bot[bot]
    email: 1+bot[bot]@users.noreply.github.com
";

/// A host in a fresh directory outside any git working tree, with its own HOME, PATH and root.
struct Host {
    dir: PathBuf,
}

impl Drop for Host {
    fn drop(&mut self) {
        // A leftover in the temporary directory is harmless, so a failure is not reported.
        #[allow(clippy::let_underscore_must_use)]
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[allow(clippy::unwrap_used, clippy::panic)]
impl Host {
    fn new(test: &str) -> Host {
        let dir = std::env::temp_dir().join(format!("wd-up-{}-{test}", std::process::id()));
        let host = Host { dir };
        for sub in [
            "home",
            "fakebin",
            "origins",
            "root/controls",
            "root/workspaces",
        ] {
            fs::create_dir_all(host.dir.join(sub)).unwrap();
        }
        std::os::unix::fs::symlink(real_git(), host.fakebin().join("git")).unwrap();
        fs::create_dir_all(host.ledger().parent().unwrap()).unwrap();
        fs::write(host.ledger(), LEDGER).unwrap();
        let origins = host.origins();
        // github.com in either URL form fetches from origins/ instead.
        fs::write(
            host.home().join(".gitconfig"),
            format!(
                "[url \"{0}/\"]\n\tinsteadOf = https://github.com/\n\tinsteadOf = git@github.com:\n",
                origins.display()
            ),
        )
        .unwrap();
        host
    }

    fn home(&self) -> PathBuf {
        self.dir.join("home")
    }

    fn fakebin(&self) -> PathBuf {
        self.dir.join("fakebin")
    }

    fn origins(&self) -> PathBuf {
        self.dir.join("origins")
    }

    fn ledger(&self) -> PathBuf {
        self.home().join(".config/wd/identities.yaml")
    }

    fn git_log(&self) -> PathBuf {
        self.dir.join("git.log")
    }

    /// Puts a shell script on PATH as `name`.
    fn install(&self, name: &str, script: &str) {
        // Renamed into place, so no test thread can hold it open for writing while it runs.
        let file = self.dir.join(name);
        fs::write(&file, script).unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
        fs::rename(&file, self.fakebin().join(name)).unwrap();
    }

    /// Puts a git on PATH that logs each call and fetches github.com from origins/ even without ~/.gitconfig.
    fn log_git(&self) {
        self.install(
            "git",
            &format!(
                "#!/bin/sh\n{{\n  echo \"argv: $*\"\n  (PATH=/usr/bin:/bin; env | grep -E '^(GIT_|WD_CONTROL_)' | sort)\n  if IFS= read -r line; then echo \"stdin: $line\"; else echo 'stdin: none'; fi\n}} >>{log}\nexec {git} -c url.{origins}/.insteadOf=https://github.com/ \"$@\"\n",
                log = self.git_log().display(),
                git = real_git().display(),
                origins = self.origins().display(),
            ),
        );
    }

    /// The calls that `log_git` logged, one string per call.
    fn git_calls(&self) -> Vec<String> {
        fs::read_to_string(self.git_log())
            .unwrap()
            .split("argv: ")
            .skip(1)
            .map(str::to_owned)
            .collect()
    }

    fn root(&self) -> PathBuf {
        self.dir.join("root")
    }

    /// Where up puts the control copy of `github.com/<owner>/<repo>`.
    fn copy(&self, owner: &str, repo: &str) -> PathBuf {
        self.root()
            .join("controls/github.com")
            .join(owner)
            .join(format!("{repo}.git"))
    }

    /// Creates the bare origin `origins/<path>` whose default branch `branch` has `files`.
    fn origin(&self, path: &str, branch: &str, files: &[(&str, &str)]) -> PathBuf {
        self.origin_with_head(path, branch, branch, files)
    }

    /// Like `origin`, with origin's HEAD naming `head`, which may differ from the pushed `branch`.
    fn origin_with_head(
        &self,
        path: &str,
        head: &str,
        branch: &str,
        files: &[(&str, &str)],
    ) -> PathBuf {
        let origin = self.origins().join(path);
        let work = self.dir.join("work");
        if work.exists() {
            fs::remove_dir_all(&work).unwrap();
        }
        fs::create_dir_all(&work).unwrap();
        self.setup_git(&work, &["init", "-q", "-b", branch]);
        for (name, content) in files {
            fs::write(work.join(name), content).unwrap();
        }
        self.setup_git(&work, &["add", "-A"]);
        self.setup_git(&work, &["commit", "-q", "--allow-empty", "-m", "init"]);
        fs::create_dir_all(&origin).unwrap();
        self.setup_git(&origin, &["init", "-q", "--bare", "-b", head]);
        self.setup_git(
            &work,
            &[
                "push",
                "-q",
                origin.to_str().unwrap(),
                &format!("HEAD:{branch}"),
            ],
        );
        origin
    }

    /// Runs git for setting up a test, apart from the person's own git configuration.
    fn setup_git(&self, cwd: &Path, args: &[&str]) -> Output {
        let out = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap())
            .env("HOME", self.dir.join("setup-home"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {}", stderr(&out));
        out
    }

    /// `wd up` with only HOME, PATH and WD_ROOT set; add or remove variables on the result.
    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_wd"));
        command
            .arg("up")
            .args(args)
            .current_dir(&self.dir)
            .env_clear()
            .env("HOME", self.home())
            .env("PATH", self.fakebin())
            .env("WD_ROOT", self.root());
        command
    }

    fn up(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }

    /// Runs git on a control copy the way a test inspects it.
    fn inspect(&self, copy: &Path, args: &[&str]) -> String {
        let mut all = vec![format!("--git-dir={}", copy.display())];
        all.extend(args.iter().map(|arg| (*arg).to_owned()));
        let out = Command::new("git")
            .args(&all)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap())
            .env("HOME", self.dir.join("setup-home"))
            .output()
            .unwrap();
        assert!(out.status.success(), "git {all:?}: {}", stderr(&out));
        String::from_utf8(out.stdout).unwrap().trim_end().to_owned()
    }
}

#[allow(clippy::unwrap_used, clippy::panic)]
fn real_git() -> PathBuf {
    let path = std::env::var_os("PATH").unwrap();
    std::env::split_paths(&path)
        .map(|dir| dir.join("git"))
        .find(|git| git.is_file())
        .unwrap_or_else(|| panic!("git is not on PATH"))
}

#[allow(clippy::unwrap_used)]
fn stderr(out: &Output) -> String {
    String::from_utf8(out.stderr.clone()).unwrap()
}

fn assert_passes(out: &Output) {
    assert_eq!(out.status.code(), Some(0), "{}", stderr(out));
    assert!(out.stdout.is_empty());
    assert!(out.stderr.is_empty(), "{}", stderr(out));
}

fn assert_fails(out: &Output, expected_stderr: &str) {
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert_eq!(stderr(out), expected_stderr);
}

fn assert_no_copy(host: &Host) {
    assert!(!host.root().join("controls/github.com").exists());
}

#[test]
fn up_creates_and_fetches_the_control_copy_silently() {
    let host = Host::new("success");
    host.origin(
        "me/control.git",
        "develop",
        &[("workspace.yaml", WORKSPACE_YAML)],
    );
    let url = "https://github.com/me/control.git";
    assert_passes(&host.up(&[url, "--identity", "me"]));
    let copy = host.copy("me", "control");
    assert_eq!(
        host.inspect(&copy, &["rev-parse", "--is-bare-repository"]),
        "true"
    );
    assert_eq!(host.inspect(&copy, &["config", "remote.origin.url"]), url);
    assert_eq!(
        host.inspect(&copy, &["symbolic-ref", "refs/remotes/origin/HEAD"]),
        "refs/remotes/origin/develop"
    );
    assert!(!copy.join("hooks").exists());
}

#[test]
fn urls_differing_only_in_case_and_form_share_one_copy_and_keep_the_url_as_given() {
    let host = Host::new("case");
    host.origin("me/control.git", "develop", &[]);
    std::os::unix::fs::symlink(host.origins().join("me"), host.origins().join("Me")).unwrap();
    std::os::unix::fs::symlink(
        host.origins().join("me/control.git"),
        host.origins().join("me/Control"),
    )
    .unwrap();
    let copy = host.copy("me", "control");
    for url in [
        "https://github.com/Me/control.git",
        "git@github.com:me/Control",
    ] {
        assert_passes(&host.up(&[url, "--identity=me"]));
        assert_eq!(host.inspect(&copy, &["config", "remote.origin.url"]), url);
    }
    let owners: Vec<_> = fs::read_dir(host.root().join("controls/github.com"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(owners, ["me"]);
}

#[test]
fn a_failing_set_head_does_not_stop_up() {
    let host = Host::new("set_head_fails");
    host.origin_with_head("me/control.git", "main", "develop", &[]);
    assert_passes(&host.up(&["https://github.com/me/control.git", "--identity", "me"]));
    let copy = host.copy("me", "control");
    assert!(copy.join("refs/remotes/origin/develop").exists());
    assert!(!copy.join("refs/remotes/origin/HEAD").exists());
}

#[test]
fn a_failing_git_command_names_the_copy_and_leaves_it_for_the_next_up_to_replace() {
    let host = Host::new("git_fails");
    host.origin("me/control.git", "develop", &[]);
    host.install(
        "git",
        &format!(
            "#!/bin/sh\ncase \"$*\" in *\"remote add\"*) echo 'error: boom' >&2; exit 3;; esac\nexec {} \"$@\"\n",
            real_git().display()
        ),
    );
    let copy = host.copy("me", "control");
    let url = "https://github.com/me/control.git";
    assert_fails(
        &host.up(&[url, "--identity", "me"]),
        &format!(
            "wd: error: git remote add failed for \"{0}\" with exit status: 3; remove \"{0}\" and run wd up again\nwd: error: error: boom\n",
            copy.display()
        ),
    );
    assert_eq!(
        host.inspect(&copy, &["rev-parse", "--is-bare-repository"]),
        "true"
    );

    fs::remove_file(host.fakebin().join("git")).unwrap();
    std::os::unix::fs::symlink(real_git(), host.fakebin().join("git")).unwrap();
    assert_passes(&host.up(&[url, "--identity", "me"]));
    assert_eq!(host.inspect(&copy, &["config", "remote.origin.url"]), url);
}

#[test]
fn an_old_copy_is_replaced() {
    let host = Host::new("replace");
    host.origin("me/control.git", "develop", &[]);
    let copy = host.copy("me", "control");
    fs::create_dir_all(copy.join("stale/deep")).unwrap();
    fs::write(copy.join("stale/deep/file"), "").unwrap();
    assert_passes(&host.up(&["https://github.com/me/control", "--identity", "me"]));
    assert!(!copy.join("stale").exists());
    assert_eq!(
        host.inspect(&copy, &["config", "remote.origin.url"]),
        "https://github.com/me/control"
    );
}

#[test]
fn a_symlink_or_a_file_above_the_copy_position_stops_up() {
    let host = Host::new("above_copy");
    let url = "https://github.com/me/control.git";
    let controls = host.root().join("controls");
    fs::create_dir_all(host.dir.join("elsewhere")).unwrap();
    for dir in [controls.join("github.com"), controls.join("github.com/me")] {
        fs::create_dir_all(dir.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(host.dir.join("elsewhere"), &dir).unwrap();
        assert_fails(
            &host.up(&[url, "--identity", "me"]),
            &format!(
                "wd: error: \"{}\" is a symlink, which wd does not follow; check where it points and remove it by hand\n",
                dir.display()
            ),
        );
        fs::remove_file(&dir).unwrap();
        fs::write(&dir, "").unwrap();
        assert_fails(
            &host.up(&[url, "--identity", "me"]),
            &format!(
                "wd: error: \"{}\" is not a directory; check what it is and remove it by hand\n",
                dir.display()
            ),
        );
        fs::remove_file(&dir).unwrap();
    }
    assert_eq!(fs::read_dir(host.dir.join("elsewhere")).unwrap().count(), 0);
}

#[test]
fn a_symlink_or_a_file_at_the_copy_position_stops_up() {
    let host = Host::new("copy_position");
    host.origin("me/control.git", "develop", &[]);
    let copy = host.copy("me", "control");
    fs::create_dir_all(copy.parent().unwrap()).unwrap();
    fs::create_dir_all(host.dir.join("elsewhere")).unwrap();
    fs::write(host.dir.join("elsewhere/keep"), "").unwrap();
    std::os::unix::fs::symlink(host.dir.join("elsewhere"), &copy).unwrap();
    assert_fails(
        &host.up(&["https://github.com/me/control.git", "--identity", "me"]),
        &format!(
            "wd: error: \"{}\" is a symlink, which wd does not follow; check where it points and remove it by hand\n",
            copy.display()
        ),
    );
    assert!(host.dir.join("elsewhere/keep").exists());

    fs::remove_file(&copy).unwrap();
    fs::write(&copy, "").unwrap();
    assert_fails(
        &host.up(&["https://github.com/me/control.git", "--identity", "me"]),
        &format!(
            "wd: error: \"{}\" is not a directory; check what it is and remove it by hand\n",
            copy.display()
        ),
    );
}

#[test]
fn git_variables_in_the_environment_of_wd_do_not_reach_git() {
    let host = Host::new("git_env");
    host.origin("me/control.git", "develop", &[]);
    let out = host
        .command(&["https://github.com/me/control.git", "--identity", "me"])
        .env("GIT_DIR", "/nonexistent")
        .env(
            "GIT_CONFIG_PARAMETERS",
            "'url.https://invalid.example/.insteadof'='https://github.com/'",
        )
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    assert_passes(&out);
}

#[test]
fn an_unreachable_control_stops_up_and_leaves_the_copy() {
    let host = Host::new("unreachable");
    let out = host.up(&["https://github.com/me/missing.git", "--identity", "me"]);
    assert_eq!(out.status.code(), Some(1));
    let err = stderr(&out);
    assert!(
        err.starts_with(
            "wd: error: cannot reach the control \"https://github.com/me/missing.git\"; check the network and the credentials, and for a github-app identity that the GitHub App is installed on that repository\nwd: error: "
        ),
        "{err}"
    );
    assert_eq!(
        host.inspect(
            &host.copy("me", "missing"),
            &["config", "remote.origin.url"]
        ),
        "https://github.com/me/missing.git"
    );
}

#[test]
fn without_identity_up_stops_before_creating_the_copy() {
    let host = Host::new("no_identity");
    host.origin("me/control.git", "develop", &[]);
    assert_fails(
        &host.up(&["https://github.com/me/control.git"]),
        "wd: error: wd up \"https://github.com/me/control.git\" brings up a new workspace, so it needs --identity <key>; choose a key under `identities` in ~/.config/wd/identities.yaml\n",
    );
    assert_no_copy(&host);
}

#[test]
fn a_bad_or_repeated_identity_stops_up_before_creating_the_copy() {
    let host = Host::new("bad_identity");
    let url = "https://github.com/me/control.git";
    assert_fails(
        &host.up(&[url, "--identity", "Me"]),
        "wd: error: --identity is invalid: \"Me\" is not a canonical name; use 2 to 30 lowercase letters, digits and hyphens, starting with a letter, with no trailing or doubled hyphen\n",
    );
    assert_fails(
        &host.up(&[url, "--identity", "me", "--identity", "me"]),
        "wd: error: --identity is given more than once; give it once\n",
    );
    assert_no_copy(&host);
}

#[test]
fn a_workspace_name_is_not_supported_yet_and_creates_no_copy() {
    let host = Host::new("by_name");
    assert_fails(
        &host.up(&["demo", "--identity", "me"]),
        "wd: error: wd up \"demo\" names a workspace, which this version of wd cannot bring up yet; give the control's URL: wd up <URL> --identity <key>\n",
    );
    assert_no_copy(&host);
}

#[test]
fn an_argument_that_is_neither_a_url_nor_a_name_is_named_with_both_reasons() {
    let host = Host::new("neither");
    for (arg, url_reason) in [
        ("ssh://git@github.com/me/control.git", "is an ssh:// URL"),
        ("file:///srv/control.git", "is a file:// URL"),
        ("/srv/control.git", "is a local path"),
        (
            "https://gitlab.com/me/control",
            "is on the host \"gitlab.com\"",
        ),
        ("https://github.com:443/me/control", "has a port"),
        ("https://me@github.com/me/control", "has a user name"),
    ] {
        let out = host.up(&[arg, "--identity", "me"]);
        assert_eq!(out.status.code(), Some(1));
        let lines: Vec<String> = stderr(&out).lines().map(str::to_owned).collect();
        assert_eq!(
            lines[0],
            format!(
                "wd: error: \"{arg}\" is neither the URL of a control nor the name of a workspace"
            )
        );
        assert!(lines[1].starts_with("wd: error: as a URL: ") && lines[1].contains(url_reason));
        assert!(lines[2].starts_with("wd: error: as a name: ") && lines[2].contains("canonical"));
        assert_eq!(lines.len(), 3);
    }
    assert_no_copy(&host);
}

#[test]
fn arguments_up_does_not_accept_are_refused() {
    let host = Host::new("args");
    assert_fails(
        &host.up(&["--identity", "me"]),
        "wd: error: wd up needs the URL of a control or the name of a workspace: wd up <URL> --identity <key>\n",
    );
    assert_fails(
        &host.up(&["demo", "other"]),
        "wd: error: unexpected argument \"other\"\n",
    );
    for option in ["--check", "--name"] {
        assert_fails(
            &host.up(&["demo", option]),
            &format!("wd: error: invalid option '{option}'\n"),
        );
    }
}

#[test]
fn the_root_comes_from_wd_root_before_config_yaml() {
    let host = Host::new("root_order");
    host.origin("me/control.git", "develop", &[]);
    let config = host.home().join(".config/wd");
    fs::create_dir_all(&config).unwrap();
    fs::write(config.join("config.yaml"), "root: /nonexistent/root\n").unwrap();
    assert_passes(&host.up(&["https://github.com/me/control.git", "--identity", "me"]));
    assert!(host.copy("me", "control").is_dir());

    fs::write(
        config.join("config.yaml"),
        format!("root: {}\n", host.root().display()),
    )
    .unwrap();
    let out = host
        .command(&["https://github.com/me/other.git", "--identity", "me"])
        .env_remove("WD_ROOT")
        .output()
        .unwrap();
    assert!(
        stderr(&out).contains("cannot reach the control"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn an_undecided_missing_or_relative_root_stops_up() {
    let host = Host::new("root_errors");
    let url = "https://github.com/me/control.git";
    let config = host.home().join(".config/wd/config.yaml");
    let out = host
        .command(&[url, "--identity", "me"])
        .env_remove("WD_ROOT")
        .output()
        .unwrap();
    assert_fails(
        &out,
        &format!(
            "wd: error: the root is not decided: WD_ROOT is not set and \"{}\" does not exist; run wd init --root <dir>\n",
            config.display()
        ),
    );

    let missing = host.dir.join("missing");
    let out = host
        .command(&[url, "--identity", "me"])
        .env("WD_ROOT", &missing)
        .output()
        .unwrap();
    assert_fails(
        &out,
        &format!(
            "wd: error: the root \"{0}\" from WD_ROOT does not exist; run wd init --root \"{0}\"\n",
            missing.display()
        ),
    );

    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(&config, format!("root: {}\n", missing.display())).unwrap();
    let out = host
        .command(&[url, "--identity", "me"])
        .env_remove("WD_ROOT")
        .output()
        .unwrap();
    assert_fails(
        &out,
        &format!(
            "wd: error: the root \"{0}\" from `root` in \"{1}\" does not exist; run wd init --root \"{0}\"\n",
            missing.display(),
            config.display()
        ),
    );

    for value in ["", "root"] {
        let out = host
            .command(&[url, "--identity", "me"])
            .env("WD_ROOT", value)
            .output()
            .unwrap();
        assert_fails(
            &out,
            &format!(
                "wd: error: the root \"{value}\" from WD_ROOT is not an absolute path; give a path that starts with \"/\"\n"
            ),
        );
    }
    assert!(!missing.exists());
}

#[test]
fn a_root_that_is_a_file_is_not_a_directory() {
    let host = Host::new("root_file");
    let file = host.dir.join("file");
    fs::write(&file, "").unwrap();
    let out = host
        .command(&["https://github.com/me/control.git", "--identity", "me"])
        .env("WD_ROOT", &file)
        .output()
        .unwrap();
    assert_fails(
        &out,
        &format!(
            "wd: error: \"{}\" is not a directory; check what it is and remove it by hand\n",
            file.display()
        ),
    );
}

#[test]
fn a_root_inside_a_git_working_tree_stops_up() {
    let host = Host::new("root_in_git");
    fs::create_dir_all(host.dir.join(".git")).unwrap();
    assert_fails(
        &host.up(&["https://github.com/me/control.git", "--identity", "me"]),
        &format!(
            "wd: error: the root \"{}\" is inside a git working tree, since \"{}\" exists; remove that .git or choose a root outside any git working tree\n",
            host.root().display(),
            host.dir.join(".git").display()
        ),
    );
    assert_no_copy(&host);
}

#[test]
fn inside_a_container_up_runs_only_on_the_host() {
    let host = Host::new("container");
    let out = host
        .command(&["https://github.com/me/control.git", "--identity", "me"])
        .env("WD_WORKSPACE", "demo")
        .output()
        .unwrap();
    assert_fails(
        &out,
        "wd: error: `wd up` runs only on the host, but WD_WORKSPACE is set, which marks the inside of a container; run it on the host, or unset WD_WORKSPACE if this is the host\n",
    );
    assert_no_copy(&host);
}

#[test]
fn without_git_up_says_to_install_it() {
    let host = Host::new("no_git");
    fs::remove_file(host.fakebin().join("git")).unwrap();
    let out = host.up(&["https://github.com/me/control.git", "--identity", "me"]);
    assert_eq!(out.status.code(), Some(1));
    let err = stderr(&out);
    assert!(err.starts_with("wd: error: cannot run git: "), "{err}");
    assert!(err.ends_with("; install git 2.32 or newer\n"), "{err}");
}

#[test]
fn a_github_app_identity_fetches_without_the_gitconfig_or_the_terminal() {
    let host = Host::new("github_app");
    host.origin("me/control.git", "develop", &[]);
    // If ~/.gitconfig were read, its longer prefix would win over the logging git's.
    fs::write(
        host.home().join(".gitconfig"),
        "[url \"/nonexistent/\"]\n\tinsteadOf = https://github.com/me/\n",
    )
    .unwrap();
    host.log_git();
    let input = host.dir.join("input");
    fs::write(&input, "typed\n").unwrap();
    let out = host
        .command(&["https://github.com/me/control.git", "--identity", "bot"])
        .stdin(fs::File::open(&input).unwrap())
        .output()
        .unwrap();
    assert_passes(&out);
    let calls = host.git_calls();
    assert_eq!(calls.len(), 4, "{calls:?}");
    for call in &calls {
        for line in [
            "GIT_CONFIG_GLOBAL=/dev/null",
            "GIT_TERMINAL_PROMPT=0",
            "GIT_ASKPASS=",
            "GIT_CONFIG_KEY_0=credential.helper",
            "GIT_CONFIG_VALUE_0=",
            "stdin: none",
        ] {
            assert!(call.lines().any(|l| l == line), "{line} in {call}");
        }
    }
    assert!(
        calls[2].starts_with(&format!(
            "--git-dir={} fetch --prune origin\n",
            host.copy("me", "control").display()
        )),
        "{}",
        calls[2]
    );
}

#[test]
fn a_github_user_identity_lets_git_read_the_terminal() {
    let host = Host::new("user_stdin");
    host.origin("me/control.git", "develop", &[]);
    host.log_git();
    let input = host.dir.join("input");
    fs::write(&input, "typed\n").unwrap();
    let out = host
        .command(&["https://github.com/me/control.git", "--identity", "me"])
        .stdin(fs::File::open(&input).unwrap())
        .output()
        .unwrap();
    assert_passes(&out);
    let calls = host.git_calls();
    assert!(calls[0].lines().any(|l| l == "stdin: typed"), "{calls:?}");
    assert!(
        !calls.iter().any(|call| call.contains("GIT_CONFIG")),
        "{calls:?}"
    );
}

#[test]
fn an_identity_missing_from_the_ledger_stops_up_before_creating_the_copy() {
    let host = Host::new("ledger_key");
    host.origin("me/control.git", "develop", &[]);
    assert_fails(
        &host.up(&["https://github.com/me/control.git", "--identity", "other"]),
        &format!(
            "wd: error: {}: line 2, column 3: the identity \"other\" is not in the ledger; the keys present are \"me\", \"bot\"; pick one of them with --identity, or add an entry with that key under `identities`\n",
            host.ledger().display()
        ),
    );
    assert_no_copy(&host);
}

#[test]
fn a_missing_or_symlinked_ledger_stops_up_before_creating_the_copy() {
    let host = Host::new("ledger_file");
    let url = "https://github.com/me/control.git";
    let moved = host.dir.join("identities.yaml");
    fs::rename(host.ledger(), &moved).unwrap();
    assert_fails(
        &host.up(&[url, "--identity", "me"]),
        &format!(
            "wd: error: \"{}\" does not exist; run wd init --root <dir> to write its template, then add an entry for \"me\" under `identities`\n",
            host.ledger().display()
        ),
    );
    std::os::unix::fs::symlink(&moved, host.ledger()).unwrap();
    assert_fails(
        &host.up(&[url, "--identity", "me"]),
        &format!(
            "wd: error: \"{}\" is a symlink, which wd does not follow; check where it points and remove it by hand\n",
            host.ledger().display()
        ),
    );
    assert_no_copy(&host);
}

#[test]
fn every_problem_of_the_selected_entry_is_reported_by_line_and_column() {
    let host = Host::new("ledger_entry");
    fs::write(
        host.ledger(),
        "identities:\n  bot:\n    type: github-app\n    login: -bot\n    app_id: 0\n",
    )
    .unwrap();
    let ledger = host.ledger();
    assert_fails(
        &host.up(&["https://github.com/me/control.git", "--identity", "bot"]),
        &[
            "line 3, column 5: missing key `identities[bot].email`",
            "line 3, column 5: missing key `identities[bot].installation_id`",
            "line 3, column 5: missing key `identities[bot].pem`",
            "line 4, column 12: `identities[bot].login` is invalid: \"-bot\" is not a GitHub login; use letters, digits and hyphens, starting and ending with a letter or digit, with an optional \"[bot]\"",
            "line 5, column 13: `identities[bot].app_id` is invalid: 0 is not 1 or more; use a positive number",
        ]
        .iter()
        .map(|line| format!("wd: error: {}: {line}\n", ledger.display()))
        .collect::<String>(),
    );
    assert_no_copy(&host);
}

#[test]
fn entries_other_than_the_selected_one_are_checked_only_for_their_value_types() {
    let host = Host::new("ledger_others");
    host.origin("me/control.git", "develop", &[]);
    let url = "https://github.com/me/control.git";
    fs::write(
        host.ledger(),
        format!("{LEDGER}  odd:\n    type: robot\n    login: '!!'\n"),
    )
    .unwrap();
    assert_passes(&host.up(&[url, "--identity", "me"]));
    fs::write(
        host.ledger(),
        format!("{LEDGER}  odd:\n    type: robot\n    login: true\n"),
    )
    .unwrap();
    assert_fails(
        &host.up(&[url, "--identity", "me"]),
        &format!(
            "wd: error: {}: line 15, column 12: `identities[odd].login` must be a string or an integer, but the plain value \"true\" is read as a boolean; write it in quotes to use it as a string\n",
            host.ledger().display()
        ),
    );
}
