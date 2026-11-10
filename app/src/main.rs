mod error;
mod fs;
mod output;
mod validate;
mod workspace;
mod yaml;

use std::ffi::OsString;
use std::io::ErrorKind;
use std::process::ExitCode;

use error::Error;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    match run(std::env::args_os().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        // The reader closed stdout early, as in `wd --version | head -c0`.
        Err(Error::Stdout(err)) if err.kind() == ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(err) => {
            output::report(&err);
            ExitCode::FAILURE
        }
    }
}

/// `args` excludes the program name.
fn run(args: Vec<OsString>) -> Result<(), Error> {
    use lexopt::prelude::*;

    let mut parser = lexopt::Parser::from_args(args);
    match parser.next()? {
        Some(Long("version")) => {
            if let Some(extra) = parser.raw_args()?.next() {
                return Err(Value(extra).unexpected().into());
            }
            output::line(VERSION)
        }
        Some(Value(command)) => match command.to_str() {
            Some("validate") => validate::run(&mut parser),
            _ => Err(Error::UnknownCommand(command)),
        },
        Some(arg) => Err(arg.unexpected().into()),
        None => Err(Error::NoCommand),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn error_of(args: &[&str]) -> String {
        run(args.iter().map(OsString::from).collect())
            .unwrap_err()
            .to_string()
    }

    #[test]
    fn version_rejects_extra_arguments() {
        for extra in ["extra", "--version", "-v", "-vx"] {
            assert_eq!(
                error_of(&["--version", extra]),
                format!("unexpected argument \"{extra}\"")
            );
        }
    }

    #[test]
    fn version_rejects_a_value() {
        assert_eq!(
            error_of(&["--version=1"]),
            "unexpected argument for option '--version': \"1\""
        );
    }
}
