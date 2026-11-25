//! Where a command may run, decided only by whether WD_WORKSPACE is set.

use crate::error::Error;

const WORKSPACE_VAR: &str = "WD_WORKSPACE";

#[derive(Debug, Clone, Copy)]
pub enum Runs {
    Host,
    Anywhere,
}

pub fn check(command: &'static str, runs: Runs) -> Result<(), Error> {
    match runs {
        Runs::Host if std::env::var_os(WORKSPACE_VAR).is_some() => {
            Err(Error::WrongPlace { command })
        }
        Runs::Host | Runs::Anywhere => Ok(()),
    }
}
