//! `wd init --root <dir>`: creates the root with controls/ and workspaces/.

use crate::error::Error;
use crate::fs;
use crate::location::Runs;
use crate::root::{self, Source};

pub const RUNS: Runs = Runs::Host;

/// `parser` has consumed the command name.
pub fn run(parser: &mut lexopt::Parser) -> Result<(), Error> {
    use lexopt::prelude::*;

    let mut value = None;
    while let Some(arg) = parser.next()? {
        match arg {
            Long("root") if value.is_some() => return Err(Error::Repeated { option: "--root" }),
            Long("root") => value = Some(parser.value()?),
            _ => return Err(arg.unexpected().into()),
        }
    }
    let value = value.ok_or(Error::MissingRootFlag)?;
    let value = value.into_string().map_err(|value| Error::NotUtf8 {
        what: "the value of --root",
        value,
    })?;
    let root = root::check(&value, Source::Flag)?;

    fs::create_dir_all(root.path()).map_err(|err| Error::Filesystem {
        action: "create",
        path: root.path().to_owned(),
        reason: err.to_string(),
    })?;
    for dir in [root.controls(), root.workspaces()] {
        fs::ensure_dir(&dir).map_err(|err| Error::dir("create", &dir, err))?;
    }
    Ok(())
}
