//! `wd validate [<path>]`: checks workspace.yaml. It reads that one file and nothing else, so it
//! runs the same on the host, in the container and in CI.

use std::path::PathBuf;

use crate::error::Error;
use crate::fs::{self, ReadError};
use crate::workspace::{self, Failure};
use crate::yaml;

const DEFAULT_PATH: &str = "workspace.yaml";

/// `parser` has consumed the command name.
pub fn run(parser: &mut lexopt::Parser) -> Result<(), Error> {
    use lexopt::prelude::*;

    let mut path = None;
    while let Some(arg) = parser.next()? {
        match arg {
            Value(value) if path.is_none() => path = Some(PathBuf::from(value)),
            _ => return Err(arg.unexpected().into()),
        }
    }
    let path = path.unwrap_or_else(|| PathBuf::from(DEFAULT_PATH));

    let bytes = match fs::read_limited(&path, yaml::MAX_BYTES as u64) {
        Ok(bytes) => bytes,
        Err(ReadError::TooLarge(size)) => {
            return Err(Error::Yaml {
                path,
                error: yaml::Error::too_large(size),
            });
        }
        Err(err) => {
            return Err(Error::Unreadable {
                path,
                reason: err.to_string(),
            });
        }
    };
    let root = match yaml::parse(&bytes) {
        Ok(root) => root,
        Err(error) => return Err(Error::Yaml { path, error }),
    };
    match workspace::check(&root) {
        Ok(_) => Ok(()),
        Err(Failure::Yaml(error)) => Err(Error::Yaml { path, error }),
        Err(Failure::Problems(problems)) => Err(Error::Invalid { path, problems }),
    }
}
