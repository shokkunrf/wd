mod common;

use std::ffi::OsString;
use std::process::ExitCode;

use common::error::Error;
use common::output;
use common::version::VERSION;

fn main() -> ExitCode {
    match run(std::env::args_os().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            output::report(&err);
            ExitCode::FAILURE
        }
    }
}

/// `args`はプログラム名を除いた引数。
fn run(args: Vec<OsString>) -> Result<(), Error> {
    use lexopt::prelude::*;

    let mut parser = lexopt::Parser::from_args(args);
    match parser.next()? {
        Some(Long("version")) => {
            if let Some(extra) = parser.raw_args()?.next() {
                return Err(Error::ExtraArgument(extra));
            }
            output::line(VERSION)
        }
        Some(Value(command)) => Err(Error::UnknownCommand(command)),
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
                format!("余分な引数: {extra}")
            );
        }
    }

    #[test]
    fn version_rejects_a_value() {
        assert_eq!(error_of(&["--version=1"]), "--versionは値を取らない: 1");
    }
}
