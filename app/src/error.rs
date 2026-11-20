use std::ffi::OsString;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum Error {
    NoCommand,
    UnknownCommand(OsString),
    Args(lexopt::Error),
    Stdout(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoCommand => write!(f, "missing command"),
            Error::UnknownCommand(command) => write!(f, "unknown command {command:?}"),
            Error::Args(err) => write!(f, "{err}"),
            Error::Stdout(err) => write!(f, "cannot write to stdout: {err}"),
        }
    }
}

impl From<lexopt::Error> for Error {
    fn from(err: lexopt::Error) -> Self {
        Error::Args(err)
    }
}
