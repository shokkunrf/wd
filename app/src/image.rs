//! The OCI reference of `image`: `<name>[:<tag>][@<digest>]`.

use crate::output::quote;

const MAX_NAME_LEN: usize = 255;
const MAX_TAG_LEN: usize = 128;
const MIN_HEX_LEN: usize = 32;

/// A reference with a tag, a digest or both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image(String);

impl Image {
    /// The error is the reason, which starts with the quoted value.
    pub fn parse(text: &str) -> Result<Image, String> {
        check(text).map_err(|why| format!("{} is not an OCI reference; {why}", quote(text)))?;
        Ok(Image(text.to_owned()))
    }

    #[cfg_attr(not(test), expect(dead_code, reason = "up reads the image (#17)"))]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn check(text: &str) -> Result<(), String> {
    let (named, digest) = match text.split_once('@') {
        Some((named, digest)) => (named, Some(digest)),
        None => (text, None),
    };
    // A colon before the last slash belongs to the port of the domain.
    let (name, tag) = match named.rsplit_once(':') {
        Some((name, tag)) if !tag.contains('/') => (name, Some(tag)),
        _ => (named, None),
    };
    if tag.is_none() && digest.is_none() {
        return Err("a tag or a digest is required".to_owned());
    }
    check_name(name)?;
    if let Some(tag) = tag {
        check_tag(tag)?;
    }
    if let Some(digest) = digest {
        check_digest(digest)?;
    }
    Ok(())
}

fn check_name(name: &str) -> Result<(), String> {
    if name.len() > MAX_NAME_LEN {
        return Err(format!("the name is over {MAX_NAME_LEN} characters"));
    }
    let mut parts: Vec<&str> = name.split('/').collect();
    // With more than one part, the first is a domain unless it is a path component.
    if parts.len() > 1 && parts.first().is_some_and(|first| !is_path(first)) {
        let domain = parts.remove(0);
        check_domain(domain)?;
    }
    for part in parts {
        if !is_path(part) {
            return Err(format!(
                "{} is not a path component; use lowercase letters and digits separated by \".\", \"_\", \"__\" or hyphens",
                quote(part)
            ));
        }
    }
    Ok(())
}

fn check_domain(domain: &str) -> Result<(), String> {
    let (host, port) = match domain.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (domain, None),
    };
    let edge = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric());
    let host_ok = edge(host.chars().next())
        && edge(host.chars().next_back())
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
        && !host.contains("..");
    if !host_ok {
        return Err(format!("{} is not a valid host", quote(host)));
    }
    if let Some(port) = port
        && (port.is_empty() || !port.chars().all(|c| c.is_ascii_digit()))
    {
        return Err(format!("{} is not a valid port", quote(port)));
    }
    Ok(())
}

/// `[a-z0-9]+((\.|_|__|-+)[a-z0-9]+)*`
fn is_path(part: &str) -> bool {
    let alnum = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit();
    part.starts_with(alnum)
        && part.ends_with(alnum)
        && part
            .split(alnum)
            .filter(|run| !run.is_empty())
            .all(valid_separator)
}

fn valid_separator(run: &str) -> bool {
    matches!(run, "." | "_" | "__") || run.chars().all(|c| c == '-')
}

fn check_tag(tag: &str) -> Result<(), String> {
    let first_ok = tag.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_');
    let rest_ok = tag
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'));
    if first_ok && rest_ok && tag.len() <= MAX_TAG_LEN {
        Ok(())
    } else {
        Err(format!(
            "{} is not a tag; a tag has 1 to {MAX_TAG_LEN} letters, digits, \"_\", \".\" and \"-\" and does not start with \".\" or \"-\"",
            quote(tag)
        ))
    }
}

fn check_digest(digest: &str) -> Result<(), String> {
    let invalid = || {
        format!(
            "{} is not a digest; use <algorithm>:<hex> with at least {MIN_HEX_LEN} hex digits",
            quote(digest)
        )
    };
    let Some((algorithm, hex)) = digest.split_once(':') else {
        return Err(invalid());
    };
    let algorithm_ok = algorithm.split(['+', '.', '_', '-']).all(|piece| {
        piece.starts_with(|c: char| c.is_ascii_alphabetic())
            && piece.chars().all(|c| c.is_ascii_alphanumeric())
    });
    let hex_ok = hex.len() >= MIN_HEX_LEN && hex.chars().all(|c| c.is_ascii_hexdigit());
    if algorithm_ok && hex_ok {
        Ok(())
    } else {
        Err(invalid())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEX: &str = "0123456789abcdef0123456789ABCDEF";

    fn ok(text: &str) {
        assert_eq!(Image::parse(text).unwrap().as_str(), text);
    }

    fn err(text: &str) -> String {
        Image::parse(text).unwrap_err()
    }

    #[test]
    fn accepts_a_tag_a_digest_or_both() {
        ok("ghcr.io/shokkunrf/devcontainer-images/base:4.1.0");
        ok("ubuntu:latest");
        ok("localhost:5000/team/app:v1");
        ok("host:5000/a/b_c__d-e--f.g:_t.-1");
        ok(&format!("example.com/o/img@sha256:{HEX}"));
        ok(&format!("img:1@sha256:{HEX}"));
        ok(&format!("img@a+b.c_d-e:{HEX}"));
        ok("LOCALHOST.example/a:1");
        ok("Foo/bar:1");
        ok("a_b.io/x:1");
        ok("Foo-1.io:5000/bar:1");
    }

    #[test]
    fn a_tag_or_a_digest_is_required() {
        assert_eq!(
            err("ghcr.io/o/img"),
            "\"ghcr.io/o/img\" is not an OCI reference; a tag or a digest is required"
        );
        assert!(err("localhost:5000/img").contains("a tag or a digest is required"));
    }

    #[test]
    fn rejects_a_malformed_reference() {
        let long = format!("{}:1", "a/".repeat(128));
        for text in [
            "",
            ":1",
            "img:",
            "img:.1",
            "img:-1",
            "img:a b",
            "Img:1",
            "a//b:1",
            "/a:1",
            "a/:1",
            "a/-b:1",
            "a/b-:1",
            "a/b___c:1",
            "a/b._c:1",
            "a/b.:1",
            "-host.io/a:1",
            "host.io-/a:1",
            "ho..st.io/a:1",
            "host.io:/a:1",
            "host.io:x/a:1",
            "host.io:80x/a:1",
            "img@sha256:abc",
            "img@sha256",
            "img@:0123456789abcdef0123456789abcdef",
            "img@1a:0123456789abcdef0123456789abcdef",
            "img@a--b:0123456789abcdef0123456789abcdef",
            "img@sha256:0123456789abcdef0123456789abcdeg",
            &long,
        ] {
            assert!(Image::parse(text).is_err(), "{text:?}");
        }
        let tag = format!("img:{}", "a".repeat(129));
        assert!(Image::parse(&tag).is_err());
        ok(&format!("img:{}", "a".repeat(128)));
    }

    #[test]
    fn a_newline_is_rejected_and_escaped() {
        for text in [
            "img:1\n",
            "img:1\nx",
            "im\ng:1",
            "img@sha256:0123456789abcdef0123456789abcdef\n",
        ] {
            let reason = err(text);
            assert!(reason.contains("\\n") && !reason.contains('\n'), "{reason}");
        }
    }
}
