//! The root, `$WD_ROOT`, checked to be absolute and outside any git working tree.

use std::fmt;
use std::path::{Component, Path, PathBuf};

use crate::config;
use crate::error::Error;
use crate::fs;
use crate::output::quote;

const ROOT_VAR: &str = "WD_ROOT";

/// Where the value of the root came from, as messages name it.
#[derive(Debug, Clone)]
pub enum Source {
    Flag,
    Env,
    Config(PathBuf),
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Source::Flag => write!(f, "--root"),
            Source::Env => write!(f, "{ROOT_VAR}"),
            Source::Config(path) => write!(f, "`root` in {}", quote(&path.to_string_lossy())),
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

/// The root from WD_ROOT, or else from config.yaml, which must exist as a directory.
pub fn resolve() -> Result<Root, Error> {
    let (value, from) = match std::env::var_os(ROOT_VAR) {
        Some(value) => {
            let value = value.into_string().map_err(|value| Error::NotUtf8 {
                what: ROOT_VAR,
                value,
            })?;
            (value, Source::Env)
        }
        None => {
            let config_yaml = config::dir()?.join(config::CONFIG_YAML);
            match config::read_root(&config_yaml)? {
                Some(value) => (value, Source::Config(config_yaml)),
                None => {
                    return Err(Error::RootUndecided {
                        config: config_yaml,
                    });
                }
            }
        }
    };
    let root = check(&value, &from)?;
    match fs::is_dir(root.path()) {
        Ok(true) => Ok(root),
        Ok(false) => match fs::entry_exists(root.path()) {
            Ok(true) => Err(Error::NotDir { path: root.0 }),
            Ok(false) => Err(Error::RootMissing { root: root.0, from }),
            Err(err) => Err(Error::Filesystem {
                action: "check",
                path: root.0,
                reason: err.to_string(),
            }),
        },
        Err(err) => Err(Error::Filesystem {
            action: "check",
            path: root.0,
            reason: err.to_string(),
        }),
    }
}

/// Checks a root that may not exist yet, without calling git.
pub fn check(value: &str, from: &Source) -> Result<Root, Error> {
    if !value.starts_with('/') {
        return Err(Error::RootNotAbsolute {
            from: from.clone(),
            value: value.to_owned(),
        });
    }
    if value.chars().any(char::is_control) {
        return Err(Error::RootControlChar {
            from: from.clone(),
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
