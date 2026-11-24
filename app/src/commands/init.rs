//! `wd init --root <dir>`: creates the root with controls/ and workspaces/ and records it in config.yaml.

use std::path::Path;

use crate::config;
use crate::error::Error;
use crate::fs;
use crate::location::Runs;
use crate::root::{self, Root, Source};

pub const RUNS: Runs = Runs::Host;

/// The umask narrows it, as for mkdir.
const ROOT_MODE: u32 = 0o777;
const CONFIG_DIR_MODE: u32 = 0o700;
const FILE_MODE: u32 = 0o600;

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
    let config_dir = config::dir()?;
    let config_yaml = config_dir.join(config::CONFIG_YAML);
    let recorded = config::read_root(&config_yaml)?;
    if let Some(recorded) = &recorded {
        same_root(&config_yaml, recorded, &root, &value)?;
    }

    fs::create_dir_all(root.path(), ROOT_MODE).map_err(|err| Error::Filesystem {
        action: "create",
        path: root.path().to_owned(),
        reason: err.to_string(),
    })?;
    for dir in [root.controls(), root.workspaces()] {
        fs::ensure_dir(&dir).map_err(|err| Error::dir("create", &dir, err))?;
    }
    fs::create_dir_all(&config_dir, CONFIG_DIR_MODE).map_err(|err| Error::Filesystem {
        action: "create",
        path: config_dir.clone(),
        reason: err.to_string(),
    })?;
    if recorded.is_none() {
        let written = fs::write_new(&config_yaml, config::render(&value).as_bytes(), FILE_MODE)
            .map_err(|err| Error::write(&config_yaml, err))?;
        // Another process wrote config.yaml after it was read, so its value is checked again.
        if !written && let Some(recorded) = config::read_root(&config_yaml)? {
            same_root(&config_yaml, &recorded, &root, &value)?;
        }
    }
    Ok(())
}

/// Paths compare by components, so `/a/` and `/a` are the same root.
fn same_root(config_yaml: &Path, recorded: &str, root: &Root, given: &str) -> Result<(), Error> {
    if Path::new(recorded) == root.path() {
        Ok(())
    } else {
        Err(Error::RootConflict {
            config: config_yaml.to_owned(),
            recorded: recorded.to_owned(),
            given: given.to_owned(),
        })
    }
}
