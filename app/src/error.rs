use std::ffi::OsString;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use crate::fs::DirError;
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
}

impl Error {
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
        }
    }
}

impl From<lexopt::Error> for Error {
    fn from(err: lexopt::Error) -> Self {
        Error::Args(err)
    }
}
