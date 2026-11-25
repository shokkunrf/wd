//! The client of the token socket, which issues a token for an identity of the ledger.

use std::fmt;
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::fs;
use crate::name::Name;
use crate::output::escape;

/// The issuer may call the GitHub API before it answers.
const TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RESPONSE: u64 = 8 * 1024;

/// A token, which `Debug` does not show so that it cannot reach a message.
pub struct Token(String);

impl Token {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Token(..)")
    }
}

/// `$XDG_RUNTIME_DIR/wd/token.sock`, or under `/run/user/<uid>` when XDG_RUNTIME_DIR is not an absolute path.
pub fn socket_path() -> Result<PathBuf, String> {
    let runtime_dir = match std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from) {
        Some(dir) if dir.is_absolute() => dir,
        _ => {
            let uid = fs::own_uid().map_err(|err| format!("cannot find the user ID: {err}"))?;
            PathBuf::from(format!("/run/user/{uid}"))
        }
    };
    Ok(runtime_dir.join("wd").join("token.sock"))
}

/// Asks the issuer at `socket` for a token of `identity` limited to `repo`.
pub fn issue(socket: &Path, identity: &Name, repo: &str) -> Result<Token, String> {
    let mut stream = fs::connect(socket).map_err(|err| format!("cannot connect: {err}"))?;
    let reason = |err: std::io::Error| match err.kind() {
        ErrorKind::WouldBlock | ErrorKind::TimedOut => {
            format!("no answer within {} seconds", TIMEOUT.as_secs())
        }
        _ => err.to_string(),
    };
    stream
        .set_read_timeout(Some(TIMEOUT))
        .and_then(|()| stream.set_write_timeout(Some(TIMEOUT)))
        .map_err(|err| err.to_string())?;
    stream
        .write_all(format!("token {} {repo}\n", identity.as_str()).as_bytes())
        .map_err(reason)?;
    let mut line = Vec::new();
    BufReader::new(stream.take(MAX_RESPONSE))
        .read_until(b'\n', &mut line)
        .map_err(reason)?;
    parse_response(&line)
}

/// Reads `ok <token> <expires>` or `err <reason>` with its newline.
fn parse_response(line: &[u8]) -> Result<Token, String> {
    let Some(line) = line.strip_suffix(b"\n") else {
        return Err(if line.is_empty() {
            "the issuer closed the connection without an answer".to_owned()
        } else if line.len() as u64 >= MAX_RESPONSE {
            format!("the answer has no newline within {MAX_RESPONSE} bytes")
        } else {
            "the issuer closed the connection in the middle of the answer".to_owned()
        });
    };
    let malformed = || "the answer is neither `ok <token> <expires>` nor `err <reason>`".to_owned();
    let line = std::str::from_utf8(line).map_err(|_| malformed())?;
    if let Some(reason) = line.strip_prefix("err ") {
        return Err(format!("the issuer answered: {}", escape(reason)));
    }
    let fields: Vec<&str> = line.split(' ').collect();
    match fields[..] {
        ["ok", token, expires] if is_word(token) && is_word(expires) => Ok(Token(token.to_owned())),
        _ => Err(malformed()),
    }
}

/// Non-empty printable ASCII without spaces, so that the token fits in a line of the credential protocol.
fn is_word(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_graphic())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ok_answer_gives_the_token() {
        let token = parse_response(b"ok ghs_abc 2026-01-01T00:00:00Z\n").unwrap();
        assert_eq!(token.as_str(), "ghs_abc");
        assert_eq!(format!("{token:?}"), "Token(..)");
    }

    #[test]
    fn an_err_answer_gives_its_reason_escaped() {
        assert_eq!(
            parse_response(b"err identity is not github-app type: me\x1b\n").unwrap_err(),
            "the issuer answered: identity is not github-app type: me\\u{1b}"
        );
    }

    #[test]
    fn other_answers_are_refused_without_showing_them() {
        for line in [
            &b"ok ghs_abc\n"[..],
            b"ok ghs_abc 2026 extra\n",
            b"ok  2026\n",
            b"ok gh\x01s 2026\n",
            b"ok ghs\xff 2026\n",
            b"OK ghs_abc 2026\n",
            b"\n",
        ] {
            let reason = parse_response(line).unwrap_err();
            assert_eq!(
                reason, "the answer is neither `ok <token> <expires>` nor `err <reason>`",
                "{line:?}"
            );
        }
        assert_eq!(
            parse_response(b"").unwrap_err(),
            "the issuer closed the connection without an answer"
        );
        assert_eq!(
            parse_response(b"ok ghs_abc 2026").unwrap_err(),
            "the issuer closed the connection in the middle of the answer"
        );
        assert_eq!(
            parse_response(&[b'x'; 8192]).unwrap_err(),
            "the answer has no newline within 8192 bytes"
        );
    }
}
