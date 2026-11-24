//! The only place that runs external commands.

use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::process::{Command, Output, Stdio};

const GIT_PREFIX: &[u8] = b"GIT_";

/// Runs git in `cwd` without any GIT_ variable of wd's environment, so that only what wd sets reaches it.
#[allow(clippy::disallowed_methods)]
pub fn git(cwd: &Path, args: &[&OsStr]) -> io::Result<Output> {
    let mut command = Command::new("git");
    command.args(args).current_dir(cwd);
    for (key, _) in std::env::vars_os() {
        if key.as_encoded_bytes().starts_with(GIT_PREFIX) {
            command.env_remove(key);
        }
    }
    command
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
}
