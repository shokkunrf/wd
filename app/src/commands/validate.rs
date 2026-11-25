//! `wd validate [<path>]`: checks workspace.yaml, reading that one file and nothing else.

use std::path::PathBuf;

use crate::error::Error;
use crate::fs;
use crate::location::Runs;
use crate::workspace::{self, Failure};
use crate::yaml;

const DEFAULT_PATH: &str = "workspace.yaml";

pub const RUNS: Runs = Runs::Anywhere;

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

    let bytes =
        fs::read_limited(&path, yaml::MAX_BYTES as u64).map_err(|err| Error::read(&path, err))?;
    let root = match yaml::parse(&bytes) {
        Ok(root) => root,
        Err(error) => return Err(Error::Yaml { path, error }),
    };
    match workspace::check(&root) {
        Ok(()) => Ok(()),
        Err(Failure::Yaml(error)) => Err(Error::Yaml { path, error }),
        Err(Failure::Problems(problems)) => Err(Error::Invalid { path, problems }),
    }
}
