//! The two forms of a repo URL that GitHub shows, and the identifier derived from them.

use crate::output::quote;

const HOST: &str = "github.com";

/// A `url` in one of the two accepted forms. The identifier is `github.com/<owner>/<repo>` in
/// lowercase without the trailing `.git`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoUrl {
    url: String,
    identifier: String,
}

impl RepoUrl {
    /// The error is the reason, which starts with the quoted value.
    pub fn parse(text: &str) -> Result<RepoUrl, String> {
        let q = quote(text);
        let path = host_and_path(text, &q)?;
        let parts: Vec<&str> = path.split('/').collect();
        if parts.iter().any(|part| part.contains("..")) {
            return Err(format!("{q} has a component with \"..\""));
        }
        let [owner, repo_with_git] = parts[..] else {
            return Err(format!("{q} must end with <owner>/<repo>"));
        };
        if !is_component(owner) {
            return Err(format!(
                "{q} has the invalid owner {}; an owner starts with a letter, digit or underscore and has only letters, digits, \".\", \"_\" and \"-\"",
                quote(owner)
            ));
        }
        // The suffix is removed whatever its case, since the identifier is lowercase.
        let repo = match repo_with_git.len().checked_sub(".git".len()) {
            Some(at)
                if repo_with_git
                    .get(at..)
                    .is_some_and(|s| s.eq_ignore_ascii_case(".git")) =>
            {
                &repo_with_git[..at]
            }
            _ => repo_with_git,
        };
        if repo.is_empty() {
            return Err(format!("{q} has no repo name before \".git\""));
        }
        if !is_component(repo) {
            return Err(format!(
                "{q} has the invalid repo {}; a repo starts with a letter, digit or underscore and has only letters, digits, \".\", \"_\" and \"-\"",
                quote(repo)
            ));
        }
        Ok(RepoUrl {
            url: text.to_owned(),
            identifier: format!("{HOST}/{}/{}", owner.to_lowercase(), repo.to_lowercase()),
        })
    }

    pub fn identifier(&self) -> &str {
        &self.identifier
    }

    pub fn as_str(&self) -> &str {
        &self.url
    }
}

/// Checks the part of the URL before the path and returns the path.
fn host_and_path<'a>(text: &'a str, q: &str) -> Result<&'a str, String> {
    let forms =
        "use https://github.com/<owner>/<repo>[.git] or git@github.com:<owner>/<repo>[.git]";
    if let Some(rest) = text.strip_prefix("https://") {
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        if authority.contains('@') {
            return Err(format!("{q} has a user name; {forms}"));
        }
        let (host, port) = match authority.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        };
        check_host(host, q)?;
        if port.is_some() {
            return Err(format!("{q} has a port; {forms}"));
        }
        if text.contains(['?', '#']) {
            return Err(format!("{q} has a query or a fragment; {forms}"));
        }
        Ok(path)
    } else if let Some(rest) = text.strip_prefix("git@") {
        let Some((host, path)) = rest.split_once(':') else {
            return Err(format!("{q} has no \":\" after the host; {forms}"));
        };
        check_host(host, q)?;
        if text.contains(['?', '#']) {
            return Err(format!("{q} has a query or a fragment; {forms}"));
        }
        Ok(path)
    } else if text.starts_with("ssh://") {
        Err(format!("{q} is an ssh:// URL; {forms}"))
    } else if text.starts_with("file://") {
        Err(format!("{q} is a file:// URL; {forms}"))
    } else if text.starts_with(['/', '.']) {
        Err(format!("{q} is a local path; {forms}"))
    } else if text.contains("://") {
        Err(format!("{q} has a scheme that is not accepted; {forms}"))
    } else if text
        .split_once('@')
        .is_some_and(|(user, _)| !user.contains('/'))
    {
        Err(format!("{q} has a user name; {forms}"))
    } else {
        Err(format!("{q} has no scheme; {forms}"))
    }
}

fn check_host(host: &str, q: &str) -> Result<(), String> {
    if host == HOST {
        Ok(())
    } else {
        Err(format!(
            "{q} is on the host {}, but only {HOST} is accepted",
            quote(host)
        ))
    }
}

fn is_component(text: &str) -> bool {
    text.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identifier(text: &str) -> String {
        RepoUrl::parse(text).unwrap().identifier().to_owned()
    }

    #[test]
    fn accepts_the_two_forms_with_and_without_git() {
        for text in [
            "https://github.com/o/r",
            "https://github.com/o/r.git",
            "git@github.com:o/r",
            "git@github.com:o/r.git",
        ] {
            assert_eq!(identifier(text), "github.com/o/r", "{text}");
            assert_eq!(RepoUrl::parse(text).unwrap().as_str(), text);
        }
    }

    #[test]
    fn the_identifier_is_lowercase_without_one_git() {
        assert_eq!(
            identifier("https://github.com/Me/Repo.GIT"),
            "github.com/me/repo"
        );
        assert_eq!(
            identifier("git@github.com:Me/Repo.git"),
            "github.com/me/repo"
        );
        assert_eq!(
            identifier("git@github.com:_o.x-y/r_1.git.git"),
            "github.com/_o.x-y/r_1.git"
        );
    }

    #[test]
    fn a_host_other_than_github_is_named() {
        assert_eq!(
            RepoUrl::parse("https://gitlab.com/o/r").unwrap_err(),
            "\"https://gitlab.com/o/r\" is on the host \"gitlab.com\", but only github.com is accepted"
        );
        assert!(
            RepoUrl::parse("git@ghe.example.com:o/r")
                .unwrap_err()
                .contains("\"ghe.example.com\"")
        );
    }

    #[test]
    fn rejects_the_forms_of_the_table() {
        for (text, reason) in [
            ("https://me@github.com/o/r", "has a user name"),
            ("me@github.com:o/r", "has a user name"),
            ("https://github.com:443/o/r", "has a port"),
            ("https://github.com/o/r?x=1", "has a query or a fragment"),
            ("https://github.com/o/r#x", "has a query or a fragment"),
            ("git@github.com:o/r#x", "has a query or a fragment"),
            ("ssh://git@github.com/o/r.git", "is an ssh:// URL"),
            ("github.com/o/r", "has no scheme"),
            ("/srv/r.git", "is a local path"),
            ("./r", "is a local path"),
            ("../r", "is a local path"),
            ("file:///srv/r.git", "is a file:// URL"),
            ("http://github.com/o/r", "has a scheme that is not accepted"),
            ("https://github.com/o/../r", "has a component with \"..\""),
            ("https://github.com/-o/r", "has the invalid owner \"-o\""),
            ("https://github.com/o/-r", "has the invalid repo \"-r\""),
            ("https://github.com/o/.git", "has no repo name"),
            ("https://github.com/o/.GIT", "has no repo name"),
            ("https://github.com/o", "must end with <owner>/<repo>"),
            ("https://github.com/o/r/x", "must end with <owner>/<repo>"),
            ("https://github.com/o/r/", "must end with <owner>/<repo>"),
            ("https://github.com//r", "has the invalid owner \"\""),
            ("https://github.com", "must end with <owner>/<repo>"),
            ("git@github.com", "has no \":\" after the host"),
            ("git@github.com:o/r x", "has the invalid repo"),
            ("", "has no scheme"),
        ] {
            let err = RepoUrl::parse(text).unwrap_err();
            assert!(err.contains(reason), "{text:?}: {err}");
        }
    }

    #[test]
    fn a_newline_is_rejected_and_escaped() {
        let err = RepoUrl::parse("https://github.com/o/r\nx").unwrap_err();
        assert!(err.contains("\\n") && !err.contains('\n'), "{err}");
        assert!(RepoUrl::parse("git@github.com:o/r\n").is_err());
    }
}
