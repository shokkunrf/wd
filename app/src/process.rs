//! The only place that runs external commands.

use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::process::{Command, Output, Stdio};

const GIT_PREFIX: &[u8] = b"GIT_";

#[derive(Debug, Clone, Copy)]
pub enum Stdin {
    Null,
    /// Lets git and ssh ask the person at the terminal.
    Inherit,
}

/// Runs git in `cwd` with `env` and without any other GIT_ variable of wd's environment.
#[allow(clippy::disallowed_methods)]
pub fn git(
    cwd: &Path,
    args: &[&OsStr],
    env: &[(String, String)],
    stdin: Stdin,
) -> io::Result<Output> {
    let mut command = Command::new("git");
    command.args(args).current_dir(cwd);
    for (key, _) in std::env::vars_os() {
        if key.as_encoded_bytes().starts_with(GIT_PREFIX) {
            command.env_remove(key);
        }
    }
    command.envs(env.iter().map(|(key, value)| (key, value)));
    let stdin = match stdin {
        Stdin::Null => Stdio::null(),
        Stdin::Inherit => Stdio::inherit(),
    };
    command
        .stdin(stdin)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
}
