//! The prechecks of up, which run before the control copy is created and report every missing item at once.

use crate::config;
use crate::control::Auth;
use crate::error::Error;
use crate::fs;
use crate::ledger::{self, Failure, Kind};
use crate::name::Name;
use crate::output::quote;
use crate::repo_url::RepoUrl;
use crate::token::{self, Token};
use crate::yaml;

/// How the control copy of `url` fetches with `identity`, or every missing item, one per line.
pub(super) fn run(identity: &Name, url: &RepoUrl) -> Result<Auth, Error> {
    let mut missing = Vec::new();
    let kind = ledger_entry(identity, &mut missing)?;
    // An identity the ledger cannot give is reported alone, without asking the issuer for it.
    let token = kind.and_then(|_| trial_issue(identity, url, &mut missing));
    match (kind, token) {
        (Some(Kind::App), Some(token)) if missing.is_empty() => Ok(Auth::App(token)),
        (Some(Kind::User), Some(_)) if missing.is_empty() => Ok(Auth::User),
        _ => Err(Error::Precheck(missing)),
    }
}

/// A token for `identity` limited to the control's repo, which proves that the issuer works for it.
fn trial_issue(identity: &Name, url: &RepoUrl, missing: &mut Vec<String>) -> Option<Token> {
    let socket = match token::socket_path() {
        Ok(socket) => socket,
        Err(reason) => {
            missing.push(format!("cannot find the token socket: {reason}"));
            return None;
        }
    };
    let repo = url.repo_as_written();
    match token::issue(&socket, identity, repo) {
        Ok(token) => Some(token),
        Err(reason) => {
            missing.push(format!(
                "the token socket {} did not issue a token for {} and the repo {}: {reason}; check that service/install was run and the issuer is running, the permissions of the PEM file and the host clock",
                quote(&socket.to_string_lossy()),
                quote(identity.as_str()),
                quote(repo)
            ));
            None
        }
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
