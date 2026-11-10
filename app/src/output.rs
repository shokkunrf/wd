use std::io::{self, Write};

use crate::error::Error;

pub fn line(text: &str) -> Result<(), Error> {
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "{text}")
        .and_then(|()| stdout.flush())
        .map_err(Error::Stdout)
}

/// Escapes `text` so that it fits in one line and shows control characters visibly.
pub fn escape(text: &str) -> String {
    text.escape_debug().to_string()
}

/// Escapes `text` and puts it in double quotes.
pub fn quote(text: &str) -> String {
    format!("\"{}\"", escape(text))
}

/// Writes one `wd: error: ` line per line of the message.
pub fn report(err: &Error) {
    let mut stderr = io::stderr().lock();
    for line in err.to_string().lines() {
        // If stderr is not writable either, the exit code is the only report left.
        #[allow(clippy::let_underscore_must_use)]
        let _ = writeln!(stderr, "wd: error: {line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_keeps_one_line() {
        assert_eq!(escape("a\nb\r\tc\"d\\"), "a\\nb\\r\\tc\\\"d\\\\");
        assert_eq!(escape("日本語"), "日本語");
        assert_eq!(quote("a\nb"), "\"a\\nb\"");
    }
}
