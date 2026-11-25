use std::ffi::OsString;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use crate::fs::{DirError, ReadError, WriteError};
use crate::output::{escape, quote};
use crate::root;
use crate::yaml;

#[derive(Debug)]
pub enum Error {
    NoCommand,
    UnknownCommand(OsString),
    Args(lexopt::Error),
    Stdout(io::Error),
    /// A file cannot be read.
    Unreadable {
        path: PathBuf,
        reason: String,
    },
    /// A file is not in the YAML format wd accepts.
    Yaml {
        path: PathBuf,
        error: yaml::Error,
    },
    /// A file is valid YAML but has keys or values wd does not accept. One line per problem.
    Invalid {
        path: PathBuf,
        problems: Vec<yaml::Error>,
    },
    WrongPlace {
        command: &'static str,
    },
    Repeated {
        option: &'static str,
    },
    NotUtf8 {
        what: &'static str,
        value: OsString,
    },
    MissingRootFlag,
    RootNotAbsolute {
        from: root::Source,
        value: String,
    },
    RootControlChar {
        from: root::Source,
        value: String,
    },
    RootInGit {
        root: PathBuf,
        found: PathBuf,
    },
    Filesystem {
        /// A verb such as "create" that reads "cannot create <path>".
        action: &'static str,
        path: PathBuf,
        reason: String,
    },
    Symlink {
        path: PathBuf,
    },
    NotDir {
        path: PathBuf,
    },
    HomeUnset,
    RootConflict {
        config: PathBuf,
        recorded: String,
        given: String,
    },
    RootUndecided {
        config: PathBuf,
    },
    RootMissing {
        root: PathBuf,
        from: root::Source,
    },
    MissingTarget,
    NeitherUrlNorName {
        arg: String,
        url_reason: String,
        name_reason: String,
    },
    UpByName {
        name: String,
    },
    IdentityRequired {
        url: String,
    },
    BadIdentity {
        reason: String,
    },
    /// The missing items of the prechecks, one line each.
    Precheck(Vec<String>),
    GitSpawn {
        reason: String,
    },
    GitFailed {
        command: &'static str,
        path: PathBuf,
        status: String,
        stderr: Vec<u8>,
    },
    Unreachable {
        url: String,
        stderr: Vec<u8>,
    },
    OriginEmpty {
        url: String,
    },
    NoWorkspaceYaml {
        url: String,
        /// Whether origin/HEAD led to a commit.
        head_resolved: bool,
        /// The branches of origin that have workspace.yaml.
        branches: Vec<String>,
    },
    WorkspaceYamlMode {
        url: String,
        mode: String,
    },
    /// workspace.yaml of the control does not pass validate. One line per problem.
    ControlWorkspace {
        url: String,
        problems: Vec<yaml::Error>,
    },
    /// The `url` that `control` names is not the control that wd up was given.
    ControlMismatch {
        url: String,
        url_id: String,
        control: String,
        control_url: String,
        control_id: String,
    },
}

impl Error {
    pub fn read(path: &Path, err: ReadError) -> Error {
        let path = path.to_owned();
        match err {
            ReadError::Symlink => Error::Symlink { path },
            ReadError::TooLarge(size) => Error::Yaml {
                path,
                error: yaml::Error::too_large(size),
            },
            err => Error::Unreadable {
                path,
                reason: err.to_string(),
            },
        }
    }

    pub fn write(path: &Path, err: WriteError) -> Error {
        let path = path.to_owned();
        match err {
            WriteError::Io(err) => Error::Filesystem {
                action: "write",
                path,
                reason: err.to_string(),
            },
            WriteError::Symlink => Error::Symlink { path },
            WriteError::NotRegular => Error::Filesystem {
                action: "write",
                path,
                reason: "something other than a regular file is there".to_owned(),
            },
        }
    }

    /// `action` is the verb of the operation that met `err` at `path`.
    pub fn dir(action: &'static str, path: &Path, err: DirError) -> Error {
        let path = path.to_owned();
        match err {
            DirError::Io(err) => Error::Filesystem {
                action,
                path,
                reason: err.to_string(),
            },
            DirError::Symlink => Error::Symlink { path },
            DirError::NotDir => Error::NotDir { path },
        }
    }
}

fn show(path: &Path) -> String {
    escape(&path.to_string_lossy())
}

fn quote_path(path: &Path) -> String {
    quote(&path.to_string_lossy())
}

/// The lines of git's stderr with control characters escaped, since a remote can put any byte in them.
fn git_lines(f: &mut fmt::Formatter<'_>, stderr: &[u8]) -> fmt::Result {
    for line in String::from_utf8_lossy(stderr).lines() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        writeln!(f)?;
        for c in line.chars() {
            if c.is_control() {
                write!(f, "{}", c.escape_debug())?;
            } else {
                write!(f, "{c}")?;
            }
        }
    }
    Ok(())
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoCommand => write!(f, "missing command"),
            Error::UnknownCommand(command) => write!(f, "unknown command {command:?}"),
            Error::Args(err) => write!(f, "{err}"),
            Error::Stdout(err) => write!(f, "cannot write to stdout: {err}"),
            Error::Unreadable { path, reason } => {
                write!(f, "cannot read {}: {reason}", show(path))
            }
            Error::Yaml { path, error } => write!(f, "{}: {error}", show(path)),
            Error::Invalid { path, problems } => {
                for (index, problem) in problems.iter().enumerate() {
                    if index > 0 {
                        writeln!(f)?;
                    }
                    write!(f, "{}: {problem}", show(path))?;
                }
                Ok(())
            }
            Error::WrongPlace { command } => write!(
                f,
                "`wd {command}` runs only on the host, but WD_WORKSPACE is set, which marks the inside of a container; run it on the host, or unset WD_WORKSPACE if this is the host"
            ),
            Error::Repeated { option } => {
                write!(f, "{option} is given more than once; give it once")
            }
            Error::NotUtf8 { what, value } => write!(
                f,
                "{what} {} is not valid UTF-8",
                quote(&value.to_string_lossy())
            ),
            Error::MissingRootFlag => write!(
                f,
                "wd init needs --root <dir>; give the absolute path of the directory where wd keeps controls/ and workspaces/"
            ),
            Error::RootNotAbsolute { from, value } => write!(
                f,
                "the root {} from {from} is not an absolute path; give a path that starts with \"/\"",
                quote(value)
            ),
            Error::RootControlChar { from, value } => write!(
                f,
                "the root {} from {from} has a control character; choose a path without one",
                quote(value)
            ),
            Error::RootInGit { root, found } => write!(
                f,
                "the root {} is inside a git working tree, since {} exists; remove that .git or choose a root outside any git working tree",
                quote_path(root),
                quote_path(found)
            ),
            Error::Filesystem {
                action,
                path,
                reason,
            } => write!(f, "cannot {action} {}: {reason}", quote_path(path)),
            Error::Symlink { path } => write!(
                f,
                "{} is a symlink, which wd does not follow; check where it points and remove it by hand",
                quote_path(path)
            ),
            Error::NotDir { path } => write!(
                f,
                "{} is not a directory; check what it is and remove it by hand",
                quote_path(path)
            ),
            Error::HomeUnset => write!(
                f,
                "HOME is not set to an absolute path, so wd cannot find ~/.config/wd; set HOME"
            ),
            Error::RootConflict {
                config,
                recorded,
                given,
            } => write!(
                f,
                "{} records the root {}, but --root gives {}; nothing was written; to change the root, edit {} yourself",
                quote_path(config),
                quote(recorded),
                quote(given),
                quote_path(config)
            ),
            Error::RootUndecided { config } => write!(
                f,
                "the root is not decided: WD_ROOT is not set and {} does not exist; run wd init --root <dir>",
                quote_path(config)
            ),
            Error::RootMissing { root, from } => write!(
                f,
                "the root {} from {from} does not exist; run wd init --root {}",
                quote_path(root),
                quote_path(root)
            ),
            Error::MissingTarget => write!(
                f,
                "wd up needs the URL of a control or the name of a workspace: wd up <URL> --identity <key>"
            ),
            Error::NeitherUrlNorName {
                arg,
                url_reason,
                name_reason,
            } => write!(
                f,
                "{} is neither the URL of a control nor the name of a workspace\nas a URL: {url_reason}\nas a name: {name_reason}",
                quote(arg)
            ),
            Error::UpByName { name } => write!(
                f,
                "wd up {} names a workspace, which this version of wd cannot bring up yet; give the control's URL: wd up <URL> --identity <key>",
                quote(name)
            ),
            Error::IdentityRequired { url } => write!(
                f,
                "wd up {} brings up a new workspace, so it needs --identity <key>; choose a key under `identities` in ~/.config/wd/identities.yaml",
                quote(url)
            ),
            Error::BadIdentity { reason } => write!(f, "--identity is invalid: {reason}"),
            Error::Precheck(lines) => write!(f, "{}", lines.join("\n")),
            Error::GitSpawn { reason } => {
                write!(f, "cannot run git: {reason}; install git 2.32 or newer")
            }
            Error::GitFailed {
                command,
                path,
                status,
                stderr,
            } => {
                write!(
                    f,
                    "git {command} failed for {} with {status}; remove {} and run wd up again",
                    quote_path(path),
                    quote_path(path)
                )?;
                git_lines(f, stderr)
            }
            Error::Unreachable { url, stderr } => {
                write!(
                    f,
                    "cannot reach the control {}; check the network and the credentials, and for a github-app identity that the GitHub App is installed on that repository",
                    quote(url)
                )?;
                git_lines(f, stderr)
            }
            Error::OriginEmpty { url } => write!(
                f,
                "origin of the control {} has no branches; in your clone of the control, write workspace.yaml with wd set, then commit and push it",
                quote(url)
            ),
            Error::NoWorkspaceYaml {
                url,
                head_resolved,
                branches,
            } => {
                if *head_resolved {
                    write!(
                        f,
                        "workspace.yaml is not at origin/HEAD of the control {}",
                        quote(url)
                    )?;
                } else {
                    write!(
                        f,
                        "origin/HEAD of the control {} does not lead to a commit, so workspace.yaml cannot be read from the default branch",
                        quote(url)
                    )?;
                }
                if branches.is_empty() {
                    write!(
                        f,
                        ", and no branch has it; in your clone of the control, write workspace.yaml with wd set, then commit and push it to the default branch"
                    )
                } else {
                    let names: Vec<String> = branches.iter().map(|b| quote(b)).collect();
                    write!(
                        f,
                        ", but the branches {} have it; merge the pull request that brings it to the default branch",
                        names.join(", ")
                    )
                }
            }
            Error::WorkspaceYamlMode { url, mode } => write!(
                f,
                "workspace.yaml at origin/HEAD of the control {} does not pass validate, since it is not a regular file but has the mode {}; in your clone of the control, make it a regular file that is not executable, then commit and push it",
                quote(url),
                escape(mode)
            ),
            Error::ControlWorkspace { url, problems } => {
                for problem in problems {
                    writeln!(
                        f,
                        "workspace.yaml at origin/HEAD of the control {}: {problem}",
                        quote(url)
                    )?;
                }
                write!(
                    f,
                    "fix workspace.yaml in your clone of the control, then commit and push it"
                )
            }
            Error::ControlMismatch {
                url,
                url_id,
                control,
                control_url,
                control_id,
            } => write!(
                f,
                "`control` of workspace.yaml at origin/HEAD of {} names {}, whose url {} gives the identifier {}, but the URL given to wd up gives {}; fix `control` or that url in your clone of the control, or give wd up the URL of the control",
                quote(url),
                quote(control),
                quote(control_url),
                quote(control_id),
                quote(url_id)
            ),
        }
    }
}

impl From<lexopt::Error> for Error {
    fn from(err: lexopt::Error) -> Self {
        Error::Args(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_lines_keep_quotes_and_escape_control_characters() {
        let err = Error::Unreachable {
            url: "https://github.com/o/r".to_owned(),
            stderr: b"fatal: 'x' \x1b[31mred\n\nremote: done  \n".to_vec(),
        };
        let text = err.to_string();
        let lines: Vec<&str> = text.lines().skip(1).collect();
        assert_eq!(lines, ["fatal: 'x' \\u{1b}[31mred", "remote: done"]);
    }
}
