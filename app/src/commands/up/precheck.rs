//! The prechecks of up, which run before the control copy is created and report every missing item at once.

use std::ffi::OsStr;
use std::path::Path;

use crate::config;
use crate::control::Auth;
use crate::error::Error;
use crate::fs;
use crate::ledger::{self, Failure, Kind};
use crate::name::Name;
use crate::output::{escape, quote};
use crate::process::{self, Stdin};
use crate::repo_url::RepoUrl;
use crate::token::{self, Token};
use crate::yaml;

/// The oldest git that takes GIT_CONFIG_COUNT and GIT_CONFIG_GLOBAL, as (major, minor).
const MIN_GIT: (u32, u32) = (2, 32);

/// How the control copy of `url` fetches with `identity`, or every missing item, one per line.
pub(super) fn run(identity: &Name, url: &RepoUrl) -> Result<Auth, Error> {
    let mut missing = Vec::new();
    tool("devcontainer", "the devcontainer CLI", &mut missing);
    tool("docker", "docker", &mut missing);
    git_version(&mut missing);
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

/// Checks that the git on PATH is at least `MIN_GIT`.
fn git_version(missing: &mut Vec<String>) {
    let need = format!("install git {}.{} or newer", MIN_GIT.0, MIN_GIT.1);
    // The version does not depend on the directory, so any that exists will do.
    let out = match process::git(Path::new("/"), &[OsStr::new("--version")], &[], Stdin::Null) {
        Ok(out) => out,
        Err(err) => {
            missing.push(format!("cannot run git: {err}; {need}"));
            return;
        }
    };
    if !out.status.success() {
        missing.push(format!(
            "cannot run git: git --version failed with {}{}; check the installation of git, which must be {}.{} or newer",
            out.status,
            first_line(&out.stderr),
            MIN_GIT.0,
            MIN_GIT.1
        ));
        return;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().next().unwrap_or_default().trim();
    match parse_git_version(line) {
        Some(version) if version >= MIN_GIT => {}
        Some(_) => missing.push(format!(
            "git --version printed {}, which is older than {}.{}; {need}",
            quote(line),
            MIN_GIT.0,
            MIN_GIT.1
        )),
        None => missing.push(format!(
            "cannot read the version of git: git --version printed {}; {need}",
            quote(line)
        )),
    }
}

/// The (major, minor) of `git version <major>.<minor>[.…]`, whatever follows the minor number.
fn parse_git_version(line: &str) -> Option<(u32, u32)> {
    let (major, rest) = line.strip_prefix("git version ")?.split_once('.')?;
    let minor_len = rest.bytes().take_while(u8::is_ascii_digit).count();
    if major.is_empty() || !major.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((major.parse().ok()?, rest[..minor_len].parse().ok()?))
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
    fn the_git_version_is_read_around_the_minimum() {
        for (line, expected) in [
            ("git version 2.31.9", Some((2, 31))),
            ("git version 2.32.0", Some((2, 32))),
            ("git version 3.0.0", Some((3, 0))),
            ("git version 2.56.0", Some((2, 56))),
            ("git version 10.1", Some((10, 1))),
        ] {
            assert_eq!(parse_git_version(line), expected, "{line}");
        }
        assert!(parse_git_version("git version 2.31.9").is_some_and(|v| v < MIN_GIT));
        assert!(parse_git_version("git version 2.32.0").is_some_and(|v| v >= MIN_GIT));
        assert!(parse_git_version("git version 3.0.0").is_some_and(|v| v >= MIN_GIT));
    }

    #[test]
    fn a_suffix_after_the_version_is_ignored() {
        for line in [
            "git version 2.39.5 (Apple Git-154)",
            "git version 2.39.0.windows.1",
            "git version 2.39.rc1",
        ] {
            assert_eq!(parse_git_version(line), Some((2, 39)), "{line}");
        }
        assert_eq!(parse_git_version("git version 2.32.0.rc1"), Some((2, 32)));
    }

    #[test]
    fn output_that_is_not_a_git_version_is_not_read() {
        for line in [
            "",
            "git",
            "git version",
            "git version 2",
            "git version 2.",
            "git version two.thirty",
            "git version .32",
            "git version -2.40.0",
            "git version 2.x",
            "version 2.40.0",
            "git version 99999999999.1",
        ] {
            assert_eq!(parse_git_version(line), None, "{line}");
        }
    }

    #[test]
    fn the_first_line_of_stderr_is_escaped() {
        assert_eq!(first_line(b"\n  boom\x1b \nmore\n"), ": boom\\u{1b}");
        assert_eq!(first_line(b" \n"), "");
    }
}
