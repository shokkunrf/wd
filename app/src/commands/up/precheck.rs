//! The prechecks of up, which run before the control copy is created and report every missing item at once.

use crate::config;
use crate::error::Error;
use crate::fs;
use crate::ledger::{self, Failure, Kind};
use crate::name::Name;
use crate::output::quote;
use crate::yaml;

/// The type of the selected identity, or every missing item, one per line.
pub(super) fn run(identity: &Name) -> Result<Kind, Error> {
    let mut missing = Vec::new();
    let kind = ledger_entry(identity, &mut missing)?;
    match kind {
        Some(kind) if missing.is_empty() => Ok(kind),
        _ => Err(Error::Precheck(missing)),
    }
}

/// The type of the entry of `identity` in identities.yaml, or None after adding why it cannot be used.
fn ledger_entry(identity: &Name, missing: &mut Vec<String>) -> Result<Option<Kind>, Error> {
    let path = config::dir()?.join(config::IDENTITIES_YAML);
    let bytes = match fs::read_nofollow(&path, yaml::MAX_BYTES as u64) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => {
            missing.push(format!(
                "{} does not exist; run wd init --root <dir> to write its template, then add an entry for {} under `identities`",
                quote(&path.to_string_lossy()),
                quote(identity.as_str())
            ));
            return Ok(None);
        }
        Err(err) => {
            missing.push(Error::read(&path, err).to_string());
            return Ok(None);
        }
    };
    let err = match ledger::read(&bytes, identity) {
        Ok(kind) => return Ok(Some(kind)),
        Err(Failure::Yaml(error)) => Error::Yaml { path, error },
        Err(Failure::Problems(problems)) => Error::Invalid { path, problems },
    };
    missing.extend(err.to_string().lines().map(str::to_owned));
    Ok(None)
}
