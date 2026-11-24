//! The control copy, the bare repository under controls/ that is the only one host git touches.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::process::Output;

use crate::error::Error;
use crate::fs;
use crate::process;
use crate::repo_url::RepoUrl;
use crate::root::Root;

#[derive(Debug)]
pub struct ControlCopy {
    path: PathBuf,
    /// `<root>/controls`, where every git call runs.
    cwd: PathBuf,
    url: RepoUrl,
}

impl ControlCopy {
    /// Creates the copy afresh, removing one that an earlier run left at the identifier's place.
    pub fn create(root: &Root, url: RepoUrl) -> Result<ControlCopy, Error> {
        let cwd = root.controls();
        let host_dir = cwd.join(url.host());
        let owner_dir = host_dir.join(url.owner());
        for dir in [&cwd, &host_dir, &owner_dir] {
            fs::ensure_dir(dir).map_err(|err| Error::dir("create", dir, err))?;
        }
        let path = owner_dir.join(format!("{}.git", url.repo()));
        fs::remove_dir_tree(&path).map_err(|err| Error::dir("remove", &path, err))?;
        let copy = ControlCopy { path, cwd, url };
        copy.init()?;
        Ok(copy)
    }

    fn init(&self) -> Result<(), Error> {
        // An empty template leaves out the sample hooks, so the copy has no hooks at all.
        let out = self.git(&[
            OsStr::new("init"),
            OsStr::new("--bare"),
            OsStr::new("--template="),
            self.path.as_os_str(),
        ])?;
        self.succeeded("init", &out)?;
        let out = self.git(&[
            &self.git_dir(),
            OsStr::new("remote"),
            OsStr::new("add"),
            OsStr::new("origin"),
            OsStr::new(self.url.as_str()),
        ])?;
        self.succeeded("remote add", &out)
    }

    /// Fetches origin and points `origin/HEAD` at origin's current default branch.
    pub fn fetch(&self) -> Result<(), Error> {
        let git_dir = self.git_dir();
        let out = self.git(&[
            &git_dir,
            OsStr::new("fetch"),
            OsStr::new("--prune"),
            OsStr::new("origin"),
        ])?;
        if !out.status.success() {
            return Err(Error::Unreachable {
                url: self.url.as_str().to_owned(),
                stderr: out.stderr,
            });
        }
        // Only a spawn error stops here, since origin's HEAD may name a branch that does not exist.
        self.git(&[
            &git_dir,
            OsStr::new("remote"),
            OsStr::new("set-head"),
            OsStr::new("origin"),
            OsStr::new("-a"),
        ])?;
        Ok(())
    }

    fn git_dir(&self) -> OsString {
        let mut arg = OsString::from("--git-dir=");
        arg.push(&self.path);
        arg
    }

    fn git(&self, args: &[&OsStr]) -> Result<Output, Error> {
        process::git(&self.cwd, args).map_err(|err| Error::GitSpawn {
            reason: err.to_string(),
        })
    }

    fn succeeded(&self, command: &'static str, out: &Output) -> Result<(), Error> {
        if out.status.success() {
            Ok(())
        } else {
            Err(Error::GitFailed {
                command,
                path: self.path.clone(),
                status: out.status.to_string(),
                stderr: out.stderr.clone(),
            })
        }
    }
}
