use std::ffi::OsString;
use std::fmt;
use std::io;
use std::path::PathBuf;

use crate::output::escape;
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
        problems: Vec<String>,
    },
}

fn show(path: &std::path::Path) -> String {
    escape(&path.to_string_lossy())
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
        }
    }
}

impl From<lexopt::Error> for Error {
    fn from(err: lexopt::Error) -> Self {
        Error::Args(err)
    }
}
