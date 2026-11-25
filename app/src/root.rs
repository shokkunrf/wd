//! The root, `$WD_ROOT`, checked to be absolute and outside any git working tree.

use std::fmt;
use std::path::{Component, Path, PathBuf};

use crate::error::Error;
use crate::fs;

/// Where the value of the root came from, as messages name it.
#[derive(Debug)]
pub enum Source {
    Flag,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Source::Flag => write!(f, "--root"),
        }
    }
}

#[derive(Debug)]
pub struct Root(PathBuf);

impl Root {
    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn controls(&self) -> PathBuf {
        self.0.join("controls")
    }

    pub fn workspaces(&self) -> PathBuf {
        self.0.join("workspaces")
    }
}

/// Checks a root that may not exist yet, without calling git.
pub fn check(value: &str, from: Source) -> Result<Root, Error> {
    if !value.starts_with('/') {
        return Err(Error::RootNotAbsolute {
            from,
            value: value.to_owned(),
        });
    }
    if value.chars().any(char::is_control) {
        return Err(Error::RootControlChar {
            from,
            value: value.to_owned(),
        });
    }
    let path = PathBuf::from(value);
    no_git_above(&path, &path)?;
    let real = nearest_realpath(&path)?;
    no_git_above(&path, &real)?;
    Ok(Root(path))
}

/// Probes `.git` in `start` and each of its ancestors up to `/`.
fn no_git_above(root: &Path, start: &Path) -> Result<(), Error> {
    for dir in start.ancestors() {
        let git = dir.join(".git");
        match fs::entry_exists(&git) {
            Ok(false) => {}
            Ok(true) => {
                return Err(Error::RootInGit {
                    root: root.to_owned(),
                    found: git,
                });
            }
            Err(err) => {
                return Err(Error::Filesystem {
                    action: "check",
                    path: git,
                    reason: err.to_string(),
                });
            }
        }
    }
    Ok(())
}

/// The path the root resolves to once it exists: the realpath of its nearest existing ancestor with the rest applied lexically, since a component that does not exist yet cannot be a symlink.
fn nearest_realpath(root: &Path) -> Result<PathBuf, Error> {
    for dir in root.ancestors() {
        match fs::realpath(dir) {
            Ok(real) => {
                let rest = root.strip_prefix(dir).unwrap_or(root);
                return Ok(lexically(real, rest));
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(Error::Filesystem {
                    action: "resolve",
                    path: dir.to_owned(),
                    reason: err.to_string(),
                });
            }
        }
    }
    Ok(lexically(PathBuf::from("/"), root))
}

/// `base` with `rest` applied as the kernel would, `..` going up and `.` staying.
fn lexically(mut base: PathBuf, rest: &Path) -> PathBuf {
    for component in rest.components() {
        match component {
            Component::Normal(name) => base.push(name),
            Component::ParentDir => {
                base.pop();
            }
            Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
        }
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_components_are_applied_lexically() {
        for (base, rest, expected) in [
            ("/s", "nonexist/../g", "/s/g"),
            ("/s", "a/./b", "/s/a/b"),
            ("/s", "../../..", "/"),
            ("/s", "", "/s"),
        ] {
            assert_eq!(
                lexically(PathBuf::from(base), Path::new(rest)),
                PathBuf::from(expected),
                "{rest}"
            );
        }
    }
}
