//! The control copy, the bare repository under controls/ that is the only one host git touches.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::process::Output;

use crate::error::Error;
use crate::fs;
use crate::process::{self, Stdin};
use crate::repo_url::RepoUrl;
use crate::root::Root;
use crate::token::Token;
use crate::yaml;

const WORKSPACE_YAML: &str = "workspace.yaml";
const ORIGIN_REFS: &str = "refs/remotes/origin/";
const ORIGIN_HEAD: &str = "refs/remotes/origin/HEAD";
const REGULAR_MODE: &str = "100644";

/// The variable that passes the token of a github-app identity to the credential helper, so it is never on disk.
const TOKEN_VAR: &str = "WD_CONTROL_TOKEN";

/// How git gets the credentials to fetch, by the type of the selected identity.
#[derive(Debug)]
pub enum Auth {
    /// The token that the issuer gave for the control's repo.
    App(Token),
    User,
}

#[derive(Debug)]
pub struct ControlCopy {
    path: PathBuf,
    /// `<root>/controls`, where every git call runs.
    cwd: PathBuf,
    url: RepoUrl,
    auth: Auth,
}

impl ControlCopy {
    /// Creates the copy afresh, removing one that an earlier run left at the identifier's place.
    pub fn create(root: &Root, url: RepoUrl, auth: Auth) -> Result<ControlCopy, Error> {
        let cwd = root.controls();
        let host_dir = cwd.join(url.host());
        let owner_dir = host_dir.join(url.owner());
        for dir in [&cwd, &host_dir, &owner_dir] {
            fs::ensure_dir(dir).map_err(|err| Error::dir("create", dir, err))?;
        }
        let path = owner_dir.join(format!("{}.git", url.repo()));
        fs::remove_dir_tree(&path).map_err(|err| Error::dir("remove", &path, err))?;
        let copy = ControlCopy {
            path,
            cwd,
            url,
            auth,
        };
        copy.init()?;
        Ok(copy)
    }

    fn init(&self) -> Result<(), Error> {
        // An empty template leaves out the sample hooks, so the copy has no hooks at all.
        let out = self.git(
            Reach::Copy,
            &[
                OsStr::new("init"),
                OsStr::new("--bare"),
                OsStr::new("--template="),
                self.path.as_os_str(),
            ],
        )?;
        self.succeeded("init", &out)?;
        let out = self.git(
            Reach::Copy,
            &[
                &self.git_dir(),
                OsStr::new("remote"),
                OsStr::new("add"),
                OsStr::new("origin"),
                OsStr::new(self.url.as_str()),
            ],
        )?;
        self.succeeded("remote add", &out)
    }

    /// Fetches origin and points `origin/HEAD` at origin's current default branch.
    pub fn fetch(&self) -> Result<(), Error> {
        let git_dir = self.git_dir();
        let out = self.git(
            Reach::Origin,
            &[
                &git_dir,
                OsStr::new("fetch"),
                OsStr::new("--prune"),
                OsStr::new("origin"),
            ],
        )?;
        if !out.status.success() {
            return Err(Error::Unreachable {
                url: self.url.as_str().to_owned(),
                stderr: out.stderr,
            });
        }
        // Only a spawn error stops here, since origin's HEAD may name a branch that does not exist.
        self.git(
            Reach::Origin,
            &[
                &git_dir,
                OsStr::new("remote"),
                OsStr::new("set-head"),
                OsStr::new("origin"),
                OsStr::new("-a"),
            ],
        )?;
        Ok(())
    }

    pub fn url(&self) -> &RepoUrl {
        &self.url
    }

    /// The bytes of workspace.yaml in the commit that origin/HEAD points to.
    pub fn workspace_yaml(&self) -> Result<Vec<u8>, Error> {
        let url = || self.url.as_str().to_owned();
        let branches = self.origin_branches()?;
        if branches.is_empty() {
            return Err(Error::OriginEmpty { url: url() });
        }
        // Resolved once, so that every read below sees the same commit.
        let head = self.origin_head()?;
        let entry = match &head {
            Some(commit) => self.ls_tree(commit)?,
            None => None,
        };
        let Some(entry) = entry else {
            let mut having = Vec::new();
            for branch in branches {
                // Only a hint, so a branch that git cannot read counts as one without the file.
                if let Ok(Some(_)) = self.ls_tree(&format!("{ORIGIN_REFS}{branch}")) {
                    having.push(branch);
                }
            }
            return Err(Error::NoWorkspaceYaml {
                url: url(),
                head_resolved: head.is_some(),
                branches: having,
            });
        };
        if entry.mode != REGULAR_MODE {
            return Err(Error::WorkspaceYamlMode {
                url: url(),
                mode: entry.mode,
            });
        }
        if let Some(size) = entry.size
            && size > yaml::MAX_BYTES as u64
        {
            return Err(Error::ControlWorkspace {
                url: url(),
                problems: vec![yaml::Error::too_large(size)],
            });
        }
        let out = self.git(
            Reach::Copy,
            &[
                &self.git_dir(),
                OsStr::new("cat-file"),
                OsStr::new("blob"),
                OsStr::new(&entry.object),
            ],
        )?;
        self.succeeded("cat-file", &out)?;
        Ok(out.stdout)
    }

    /// The names of the branches under refs/remotes/origin/.
    fn origin_branches(&self) -> Result<Vec<String>, Error> {
        let out = self.git(
            Reach::Copy,
            &[
                &self.git_dir(),
                OsStr::new("for-each-ref"),
                OsStr::new("--format=%(refname)"),
                OsStr::new(ORIGIN_REFS),
            ],
        )?;
        self.succeeded("for-each-ref", &out)?;
        Ok(String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|line| *line != ORIGIN_HEAD)
            .filter_map(|line| line.strip_prefix(ORIGIN_REFS))
            .map(str::to_owned)
            .collect())
    }

    /// The commit that origin/HEAD points to, or None when it does not lead to one.
    fn origin_head(&self) -> Result<Option<String>, Error> {
        let out = self.git(
            Reach::Copy,
            &[
                &self.git_dir(),
                OsStr::new("rev-parse"),
                OsStr::new("--verify"),
                OsStr::new("--quiet"),
                OsStr::new(&format!("{ORIGIN_HEAD}^{{commit}}")),
            ],
        )?;
        if !out.status.success() {
            return Ok(None);
        }
        Ok(Some(String::from_utf8_lossy(&out.stdout).trim().to_owned()))
    }

    /// The entry of workspace.yaml at the top of `tree_ish`, or None when it has none.
    fn ls_tree(&self, tree_ish: &str) -> Result<Option<TreeEntry>, Error> {
        let out = self.git(
            Reach::Copy,
            &[
                &self.git_dir(),
                OsStr::new("ls-tree"),
                OsStr::new("-l"),
                OsStr::new(tree_ish),
                OsStr::new("--"),
                OsStr::new(WORKSPACE_YAML),
            ],
        )?;
        self.succeeded("ls-tree", &out)?;
        Ok(TreeEntry::parse(&out.stdout))
    }

    fn git_dir(&self) -> OsString {
        let mut arg = OsString::from("--git-dir=");
        arg.push(&self.path);
        arg
    }

    fn git(&self, reach: Reach, args: &[&OsStr]) -> Result<Output, Error> {
        let (env, stdin) = match &self.auth {
            // A github-app identity runs unattended, so git must not wait for the terminal.
            Auth::App(token) => {
                let mut env = app_env();
                if let Reach::Origin = reach {
                    env.push((TOKEN_VAR.to_owned(), token.as_str().to_owned()));
                }
                (env, Stdin::Null)
            }
            Auth::User => (Vec::new(), Stdin::Inherit),
        };
        process::git(&self.cwd, args, &env, stdin).map_err(|err| Error::GitSpawn {
            reason: err.to_string(),
        })
    }

    fn succeeded(&self, command: &'static str, out: &Output) -> Result<(), Error> {
        if out.status.success() {
            Ok(())
        } else {
            Err(Error::GitFailed {
                command,
                path: self.path.clone(),
                status: out.status.to_string(),
                stderr: out.stderr.clone(),
            })
        }
    }
}

/// One line of `git ls-tree -l`.
#[derive(Debug, PartialEq, Eq)]
struct TreeEntry {
    mode: String,
    object: String,
    /// None for an entry that is not a blob.
    size: Option<u64>,
}

impl TreeEntry {
    /// Reads `<mode> <type> <object> <size>\t<path>` of the first line, or None when there is no line.
    fn parse(stdout: &[u8]) -> Option<TreeEntry> {
        let text = String::from_utf8_lossy(stdout);
        let (meta, _) = text.lines().next()?.split_once('\t')?;
        let fields: Vec<&str> = meta.split_whitespace().collect();
        let [mode, _, object, size] = fields[..] else {
            return None;
        };
        Some(TreeEntry {
            mode: mode.to_owned(),
            object: object.to_owned(),
            size: size.parse().ok(),
        })
    }
}

/// Whether a git call talks to origin, which is the only kind that gets the token.
#[derive(Debug, Clone, Copy)]
enum Reach {
    Copy,
    Origin,
}

/// Answers only https on github.com, the only host a control can be on after the rewrite of git@github.com:.
fn credential_helper() -> String {
    format!(
        "!f() {{ test \"$1\" = get || return 0; p= h=; while IFS= read -r line && test -n \"$line\"; do case \"$line\" in protocol=https) p=1 ;; host=github.com) h=1 ;; esac; done; if test \"$p$h\" = 11; then printf 'username=x-access-token\\npassword=%s\\n' \"${TOKEN_VAR}\"; fi; }}; f"
    )
}

/// The variables for a github-app identity, with the configuration passed through GIT_CONFIG_COUNT.
fn app_env() -> Vec<(String, String)> {
    let config = [
        // An empty value drops the helpers of the system configuration.
        ("credential.helper", String::new()),
        ("credential.helper", credential_helper()),
        (
            "url.https://github.com/.insteadOf",
            "git@github.com:".to_owned(),
        ),
    ];
    let mut env = vec![
        ("GIT_TERMINAL_PROMPT".to_owned(), "0".to_owned()),
        // An empty value keeps git from falling back to core.askPass or SSH_ASKPASS.
        ("GIT_ASKPASS".to_owned(), String::new()),
        ("GIT_CONFIG_GLOBAL".to_owned(), "/dev/null".to_owned()),
        ("GIT_CONFIG_COUNT".to_owned(), config.len().to_string()),
    ];
    for (index, (key, value)) in config.into_iter().enumerate() {
        env.push((format!("GIT_CONFIG_KEY_{index}"), key.to_owned()));
        env.push((format!("GIT_CONFIG_VALUE_{index}"), value));
    }
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tree_entry_is_read_from_ls_tree() {
        assert_eq!(
            TreeEntry::parse(b"100644 blob 0123abcd     149\tworkspace.yaml\n"),
            Some(TreeEntry {
                mode: "100644".to_owned(),
                object: "0123abcd".to_owned(),
                size: Some(149),
            })
        );
        assert_eq!(
            TreeEntry::parse(b"040000 tree 0123abcd       -\tworkspace.yaml\n"),
            Some(TreeEntry {
                mode: "040000".to_owned(),
                object: "0123abcd".to_owned(),
                size: None,
            })
        );
        assert_eq!(TreeEntry::parse(b""), None);
    }
}
