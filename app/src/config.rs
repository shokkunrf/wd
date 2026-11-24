//! `~/.config/wd/` and the root that its config.yaml records.

use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::fs;
use crate::output::escape;
use crate::yaml;

pub const CONFIG_YAML: &str = "config.yaml";
pub const IDENTITIES_YAML: &str = "identities.yaml";
const ROOT_KEY: &str = "root";

/// `$HOME/.config/wd`, which does not depend on XDG_CONFIG_HOME.
pub fn dir() -> Result<PathBuf, Error> {
    match std::env::var_os("HOME").map(PathBuf::from) {
        Some(home) if home.is_absolute() => Ok(home.join(".config").join("wd")),
        _ => Err(Error::HomeUnset),
    }
}

/// The `root:` value of config.yaml, or None when the file is absent.
pub fn read_root(path: &Path) -> Result<Option<String>, Error> {
    let bytes = match fs::read_nofollow(path, yaml::MAX_BYTES as u64) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return Ok(None),
        Err(err) => return Err(Error::read(path, err)),
    };
    root_of(&bytes)
        .map(Some)
        .map_err(|problems| Error::Invalid {
            path: path.to_owned(),
            problems,
        })
}

fn root_of(bytes: &[u8]) -> Result<String, Vec<yaml::Error>> {
    let document = yaml::parse(bytes).map_err(|err| vec![err])?;
    let entries = document.as_map("the document").map_err(|err| vec![err])?;
    let mut root = None;
    let mut problems = Vec::new();
    for entry in entries {
        if entry.key != ROOT_KEY {
            problems.push(yaml::Error::new(
                entry.key_pos,
                format!("unknown key `{}`", escape(&entry.key)),
            ));
            continue;
        }
        match entry.value.as_str("`root`") {
            Ok(value) => root = Some(value.to_owned()),
            Err(err) => problems.push(err),
        }
    }
    if !entries.iter().any(|entry| entry.key == ROOT_KEY) {
        problems.push(yaml::Error::new(document.pos(), "missing key `root`"));
    }
    match root {
        Some(root) if problems.is_empty() => Ok(root),
        _ => Err(problems),
    }
}

/// The content of config.yaml that records `root`.
pub fn render(root: &str) -> String {
    format!("{ROOT_KEY}: {}\n", yaml::write_scalar(root))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problems(text: &str) -> Vec<String> {
        root_of(text.as_bytes())
            .unwrap_err()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn the_rendered_root_reads_back() {
        for root in ["/home/me/wd", "/a #b", "/a: b", "/x\"y\\z"] {
            assert_eq!(root_of(render(root).as_bytes()).unwrap(), root);
        }
        assert_eq!(render("/home/me/wd"), "root: /home/me/wd\n");
    }

    #[test]
    fn unknown_keys_and_a_missing_root_are_listed() {
        assert_eq!(
            problems("rot: /a\nother: 1\n"),
            [
                "line 1, column 1: unknown key `rot`",
                "line 2, column 1: unknown key `other`",
                "line 1, column 1: missing key `root`",
            ]
        );
        assert_eq!(problems(""), ["line 1, column 1: missing key `root`"]);
    }

    #[test]
    fn a_root_that_is_not_a_string_is_named() {
        assert_eq!(
            problems("root: 10\n"),
            [
                "line 1, column 7: `root` must be a string, but the plain value \"10\" is read as an integer; write it in quotes to use it as a string"
            ]
        );
        assert_eq!(
            problems("- /a\n"),
            ["line 1, column 1: the document must be a mapping, but it is a sequence"]
        );
        assert!(problems("root: [\n")[0].starts_with("line "));
    }
}
