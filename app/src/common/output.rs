use std::io::{self, Write};

use super::error::Error;

pub fn line(text: &str) -> Result<(), Error> {
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "{text}")
        .and_then(|()| stdout.flush())
        .map_err(Error::Stdout)
}

pub fn report(err: &Error) {
    let mut stderr = io::stderr().lock();
    // 標準エラーにも書けなければ、終了コードだけで伝える
    #[allow(clippy::let_underscore_must_use)]
    let _ = writeln!(stderr, "wd: {err}");
}
