//! Error type for fallible sysfs operations.

use std::fmt;
use std::path::PathBuf;

/// Errors that can be surfaced by the sysfs backend.
///
/// Most enumeration is intentionally infallible: missing files yield empty
/// results because that's the natural sysfs idiom (the kernel may have
/// published or unpublished an attribute between two reads). [`Error`] is
/// reserved for cases where the configuration itself is wrong — e.g., a
/// caller pointed [`crate::Sysfs`] at a path that doesn't exist.
#[derive(Debug)]
pub enum Error {
    /// The supplied sysfs root does not exist or is not a directory.
    InvalidRoot(PathBuf),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidRoot(p) => {
                write!(f, "sysfs root {} is not a directory", p.display())
            }
        }
    }
}

impl std::error::Error for Error {}

/// Result alias for [`Error`].
pub type Result<T> = std::result::Result<T, Error>;
