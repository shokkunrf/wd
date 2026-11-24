//! The prechecks of up, which run before the control copy is created and report every missing item at once.

use crate::config;
use crate::control::Auth;
use crate::error::Error;
use crate::fs;
use crate::ledger::{self, Failure, Kind};
use crate::name::Name;
use crate::output::{escape, quote};
use crate::process;
use crate::repo_url::RepoUrl;
use crate::token::{self, Token};
use crate::yaml;

/// How the control copy of `url` fetches with `identity`, or every missing item, one per line.
pub(super) fn run(identity: &Name, url: &RepoUrl) -> Result<Auth, Error> {
    let mut missing = Vec::new();
    tool("devcontainer", "the devcontainer CLI", &mut missing);
    tool("docker", "docker", &mut missing);
    let kind = ledger_entry(identity, &mut missing);
    // An identity the ledger cannot give is reported alone, without asking the issuer for it.
    let token = kind.and_then(|_| trial_issue(identity, url, &mut missing));
    linger(&mut missing);
    match (kind, token) {
        (Some(Kind::App), Some(token)) if missing.is_empty() => Ok(Auth::App(token)),
        (Some(Kind::User), Some(_)) if missing.is_empty() => Ok(Auth::User),
        _ => Err(Error::Precheck(missing)),
    }
}

/// Checks that `program --version` runs; `what` names what to install.
fn tool(program: &str, what: &str, missing: &mut Vec<String>) {
    match process::capture(program, &["--version"]) {
        Ok(out) if out.status.success() => {}
        Ok(out) => missing.push(format!(
            "cannot run {program}: {program} --version failed with {}{}; check the installation of {what}",
            out.status,
            first_line(&out.stderr)
        )),
        Err(err) => missing.push(format!("cannot run {program}: {err}; install {what}")),
    }
}

/// Checks that systemd keeps the user's services, the token issuer among them, running without a login.
fn linger(missing: &mut Vec<String>) {
    let uid = match fs::own_uid() {
        Ok(uid) => uid.to_string(),
        Err(err) => {
            missing.push(format!(
                "cannot check Linger: cannot find the user ID from /proc/self: {err}; check that /proc is mounted"
            ));
            return;
        }
    };
    let out = match process::capture("loginctl", &["show-user", &uid, "--property=Linger"]) {
        Ok(out) => out,
        Err(err) => {
            missing.push(format!("cannot check Linger: cannot run loginctl: {err}; install systemd's loginctl, then run loginctl enable-linger"));
            return;
        }
    };
    let next = "run loginctl enable-linger";
    if !out.status.success() {
        missing.push(format!(
            "cannot check Linger for user {uid}: loginctl show-user failed with {}{}; {next}",
            out.status,
            first_line(&out.stderr)
        ));
        return;
    }
    match linger_value(&out.stdout) {
        Some(true) => {}
        Some(false) => missing.push(format!("Linger is not enabled for user {uid}; {next}")),
        None => missing.push(format!(
            "cannot check Linger for user {uid}: loginctl show-user printed neither Linger=yes nor Linger=no; {next}"
        )),
    }
}

fn linger_value(stdout: &[u8]) -> Option<bool> {
    String::from_utf8_lossy(stdout)
        .lines()
        .find_map(|line| match line.trim_end() {
            "Linger=yes" => Some(true),
            "Linger=no" => Some(false),
            _ => None,
        })
}

/// The first line of a command's stderr after ": ", or nothing when it is empty.
fn first_line(stderr: &[u8]) -> String {
    match String::from_utf8_lossy(stderr)
        .lines()
        .find(|line| !line.trim().is_empty())
    {
        Some(line) => format!(": {}", escape(line.trim())),
        None => String::new(),
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
fn ledger_entry(identity: &Name, missing: &mut Vec<String>) -> Option<Kind> {
    let path = match config::dir() {
        Ok(dir) => dir.join(config::IDENTITIES_YAML),
        Err(err) => {
            missing.push(err.to_string());
            return None;
        }
    };
    let bytes = match fs::read_nofollow(&path, yaml::MAX_BYTES as u64) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => {
            missing.push(format!(
                "{} does not exist; run wd init --root <dir> to write its template, then add an entry for {} under `identities`",
                quote(&path.to_string_lossy()),
                quote(identity.as_str())
            ));
            return None;
        }
        Err(err) => {
            missing.push(Error::read(&path, err).to_string());
            return None;
        }
    };
    let err = match ledger::read(&bytes, identity) {
        Ok(kind) => return Some(kind),
        Err(Failure::Yaml(error)) => Error::Yaml { path, error },
        Err(Failure::Problems(problems)) => Error::Invalid { path, problems },
    };
    missing.extend(err.to_string().lines().map(str::to_owned));
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linger_is_read_from_its_property_line() {
        assert_eq!(linger_value(b"Linger=yes\n"), Some(true));
        assert_eq!(linger_value(b"Linger=no\n"), Some(false));
        assert_eq!(linger_value(b"Name=me\nLinger=yes\n"), Some(true));
        assert_eq!(linger_value(b"Linger=maybe\n"), None);
        assert_eq!(linger_value(b""), None);
    }

    #[test]
    fn the_first_line_of_stderr_is_escaped() {
        assert_eq!(first_line(b"\n  boom\x1b \nmore\n"), ": boom\\u{1b}");
        assert_eq!(first_line(b" \n"), "");
    }
}
