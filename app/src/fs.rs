//! The only place that touches the filesystem.

use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, ErrorKind, Read};
use std::path::{Path, PathBuf};

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

/// Reads a regular file of at most `limit` bytes, following symlinks. A FIFO is never opened.
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

#[derive(Debug)]
pub enum DirError {
    Io(io::Error),
    Symlink,
    NotDir,
}

/// Whether an entry of any type, a dangling symlink included, has this path.
#[allow(clippy::disallowed_methods)]
pub fn entry_exists(path: &Path) -> io::Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        // A file in place of a parent directory means nothing can be under it.
        Err(err) if matches!(err.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => {
            Ok(false)
        }
        Err(err) => Err(err),
    }
}

/// The path with every symlink resolved. The path must exist.
#[allow(clippy::disallowed_methods)]
pub fn realpath(path: &Path) -> io::Result<PathBuf> {
    std::fs::canonicalize(path)
}

/// Creates the directory and its missing parents, following symlinks on the way.
#[allow(clippy::disallowed_methods)]
pub fn create_dir_all(path: &Path) -> io::Result<()> {
    std::fs::create_dir_all(path)
}

/// Creates the directory if it is absent, and checks that what is there is a directory and not a symlink.
#[allow(clippy::disallowed_methods)]
pub fn ensure_dir(path: &Path) -> Result<(), DirError> {
    match std::fs::create_dir(path) {
        Ok(()) => return Ok(()),
        Err(err) if err.kind() == ErrorKind::AlreadyExists => {}
        Err(err) => return Err(DirError::Io(err)),
    }
    let meta = std::fs::symlink_metadata(path).map_err(DirError::Io)?;
    if meta.file_type().is_symlink() {
        Err(DirError::Symlink)
    } else if meta.is_dir() {
        Ok(())
    } else {
        Err(DirError::NotDir)
    }
}
