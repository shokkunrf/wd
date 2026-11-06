use std::ffi::OsString;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum Error {
    NoCommand,
    UnknownCommand(OsString),
    ExtraArgument(OsString),
    Args(lexopt::Error),
    Stdout(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoCommand => write!(f, "コマンドが無い"),
            Error::UnknownCommand(command) => {
                write!(f, "知らないコマンド: {}", command.display())
            }
            Error::ExtraArgument(arg) => write!(f, "余分な引数: {}", arg.display()),
            Error::Args(err) => write_args_error(f, err),
            Error::Stdout(err) => write!(f, "標準出力に書けない: {err}"),
        }
    }
}

fn write_args_error(f: &mut fmt::Formatter<'_>, err: &lexopt::Error) -> fmt::Result {
    match err {
        lexopt::Error::MissingValue {
            option: Some(option),
        } => write!(f, "{option}に値が無い"),
        lexopt::Error::MissingValue { option: None } => write!(f, "値が無い"),
        lexopt::Error::UnexpectedOption(option) => write!(f, "知らないオプション: {option}"),
        lexopt::Error::UnexpectedArgument(arg) => write!(f, "余分な引数: {}", arg.display()),
        lexopt::Error::UnexpectedValue { option, value } => {
            write!(f, "{option}は値を取らない: {}", value.display())
        }
        lexopt::Error::ParsingFailed { value, error } => {
            write!(f, "値を解釈できない: {value}: {error}")
        }
        lexopt::Error::NonUnicodeValue(value) => {
            write!(f, "UTF-8でない値: {}", value.display())
        }
        lexopt::Error::Custom(error) => write!(f, "{error}"),
    }
}

impl From<lexopt::Error> for Error {
    fn from(err: lexopt::Error) -> Self {
        Error::Args(err)
    }
}
