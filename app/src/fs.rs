//! The only place that touches the filesystem.

use std::fmt;
use std::fs::{DirBuilder, File, OpenOptions};
use std::io::{self, ErrorKind, Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ReadError {
    Io(io::Error),
    Symlink,
    NotRegular,
    /// The size of the file in bytes.
    TooLarge(u64),
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadError::Io(err) => write!(f, "{err}"),
            ReadError::Symlink => write!(f, "a symlink"),
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
    read_open(file, limit)
}

/// Reads a regular file of at most `limit` bytes without following a symlink at the path, or None when it is absent.
#[allow(clippy::disallowed_methods)]
pub fn read_nofollow(path: &Path, limit: u64) -> Result<Option<Vec<u8>>, ReadError> {
    // O_NONBLOCK keeps a FIFO from blocking the open; the type check below then rejects it.
    let opened = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path);
    let file = match opened {
        Ok(file) => file,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) if err.raw_os_error() == Some(libc::ELOOP) => return Err(ReadError::Symlink),
        Err(err) => return Err(ReadError::Io(err)),
    };
    read_open(file, limit).map(Some)
}

fn read_open(file: File, limit: u64) -> Result<Vec<u8>, ReadError> {
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
pub enum WriteError {
    Io(io::Error),
    Symlink,
    NotRegular,
}

/// Writes a new file, or returns false when a regular file is already there and leaves it as it is.
#[allow(clippy::disallowed_methods)]
pub fn write_new(path: &Path, bytes: &[u8], mode: u32) -> Result<bool, WriteError> {
    let opened = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path);
    match opened {
        Ok(mut file) => file.write_all(bytes).map(|()| true).map_err(WriteError::Io),
        Err(err) if err.kind() == ErrorKind::AlreadyExists => {
            regular_or_absent(path).map(|()| false)
        }
        Err(err) => Err(WriteError::Io(err)),
    }
}

/// Checks that `write_new` would either write the file or find a regular file there.
#[allow(clippy::disallowed_methods)]
pub fn regular_or_absent(path: &Path) -> Result<(), WriteError> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(WriteError::Io(err)),
    };
    if meta.file_type().is_symlink() {
        Err(WriteError::Symlink)
    } else if meta.is_file() {
        Ok(())
    } else {
        Err(WriteError::NotRegular)
    }
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

/// Whether the path leads to a directory, following symlinks.
#[allow(clippy::disallowed_methods)]
pub fn is_dir(path: &Path) -> io::Result<bool> {
    match std::fs::metadata(path) {
        Ok(meta) => Ok(meta.is_dir()),
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

/// Creates the directory and its missing parents with `mode`, following symlinks on the way.
#[allow(clippy::disallowed_methods)]
pub fn create_dir_all(path: &Path, mode: u32) -> io::Result<()> {
    DirBuilder::new().recursive(true).mode(mode).create(path)
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

/// Removes a directory tree that is absent or a real directory; a symlink or a file is left in place.
#[allow(clippy::disallowed_methods)]
pub fn remove_dir_tree(path: &Path) -> Result<(), DirError> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(DirError::Io(err)),
    };
    if meta.file_type().is_symlink() {
        Err(DirError::Symlink)
    } else if meta.is_dir() {
        std::fs::remove_dir_all(path).map_err(DirError::Io)
    } else {
        Err(DirError::NotDir)
    }
}

/// Follows a symlink at `path`, as connecting to a socket always does.
#[allow(clippy::disallowed_methods)]
pub fn connect(path: &Path) -> io::Result<UnixStream> {
    UnixStream::connect(path)
}

/// The user ID that wd runs as, which owns /proc/self.
#[allow(clippy::disallowed_methods)]
pub fn own_uid() -> io::Result<u32> {
    std::fs::metadata("/proc/self").map(|meta| meta.uid())
}
