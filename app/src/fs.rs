//! The only place that reads files. Other modules take bytes.

use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::path::Path;

#[derive(Debug)]
pub enum ReadError {
    Io(io::Error),
    NotRegular,
    /// The size of the file in bytes.
    TooLarge(u64),
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadError::Io(err) => write!(f, "{err}"),
            ReadError::NotRegular => write!(f, "not a regular file"),
            ReadError::TooLarge(size) => write!(f, "file is {size} bytes"),
        }
    }
}

impl From<io::Error> for ReadError {
    fn from(err: io::Error) -> Self {
        ReadError::Io(err)
    }
}

/// Reads a regular file of at most `limit` bytes. Symlinks are followed.
///
/// The type is checked before opening, so that a FIFO is never opened and does not block, and again
/// on the opened file. At most `limit + 1` bytes are read.
#[allow(clippy::disallowed_methods)]
pub fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>, ReadError> {
    if !std::fs::metadata(path)?.is_file() {
        return Err(ReadError::NotRegular);
    }
    let file: File = OpenOptions::new().read(true).open(path)?;
    let meta = file.metadata()?;
    if !meta.is_file() {
        return Err(ReadError::NotRegular);
    }
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1)).read_to_end(&mut bytes)?;
    let read = bytes.len() as u64;
    if read > limit || meta.len() > limit {
        return Err(ReadError::TooLarge(read.max(meta.len())));
    }
    Ok(bytes)
}
