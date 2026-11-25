//! `wd init --root <dir>`: creates the root with controls/ and workspaces/, records it in config.yaml and writes the ledger template.

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

/// Only comment lines, so it reads as a ledger without entries.
const LEDGER_TEMPLATE: &str = "\
# The identity ledger of wd. Add one entry under `identities` per identity, keyed by a canonical name,
# then pick it with wd up <URL> --identity <key>. The entries below show the two types.
#
# identities:
#   bot:
#     type: github-app
#     app_id: 123456
#     installation_id: 7890123
#     pem: /home/me/.config/wd/bot.pem
#     login: bot[bot]
#     email: 123456+bot[bot]@users.noreply.github.com
#   me:
#     type: github-user
#     login: me
#     email: me@example.com
";

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
    let root = root::check(&value, &Source::Flag)?;
    let config_dir = config::dir()?;
    let config_yaml = config_dir.join(config::CONFIG_YAML);
    let recorded = config::read_root(&config_yaml)?;
    if let Some(recorded) = &recorded {
        same_root(&config_yaml, recorded, &root, &value)?;
    }
    let ledger = config_dir.join(config::IDENTITIES_YAML);
    fs::regular_or_absent(&ledger).map_err(|err| Error::write(&ledger, err))?;

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
    fs::write_new(&ledger, LEDGER_TEMPLATE.as_bytes(), FILE_MODE)
        .map_err(|err| Error::write(&ledger, err))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::{self, Kind};
    use crate::name::Name;
    use crate::yaml;

    fn uncommented_template() -> String {
        LEDGER_TEMPLATE
            .lines()
            .skip_while(|line| *line != "# identities:")
            .map(|line| format!("{}\n", line.strip_prefix("# ").unwrap()))
            .collect()
    }

    #[test]
    fn the_ledger_template_reads_as_no_entries() {
        let root = yaml::parse(LEDGER_TEMPLATE.as_bytes()).unwrap();
        assert!(root.as_map("the document").unwrap().is_empty());
    }

    #[test]
    fn the_ledger_template_uncommented_has_both_entries() {
        let root = yaml::parse(uncommented_template().as_bytes()).unwrap();
        let entries = root.as_map("the document").unwrap();
        assert_eq!(entries[0].key, "identities");
        let identities = entries[0].value.as_map("`identities`").unwrap();
        let keys: Vec<&str> = identities.iter().map(|entry| entry.key.as_str()).collect();
        assert_eq!(keys, ["bot", "me"]);
    }

    #[test]
    fn the_entries_of_the_ledger_template_pass_the_ledger_check() {
        let text = uncommented_template();
        for (key, kind) in [("bot", Kind::App), ("me", Kind::User)] {
            let key = Name::parse(key).unwrap();
            assert_eq!(ledger::read(text.as_bytes(), &key).unwrap(), kind);
        }
    }
}
