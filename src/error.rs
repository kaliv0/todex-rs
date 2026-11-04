//! Library and CLI error kinds.

use std::{io, path::PathBuf};

use thiserror::Error;

/// Failures from setup, walk, IO, and control flow.
#[derive(Debug, Error)]
pub enum Error {
    /// Cooperative interrupt was requested.
    #[error("interrupted")]
    Interrupted,

    /// Stdout-only broken pipe - stop the walk.
    #[error("stop extraction")]
    StopExtraction,

    /// Scan root is a symlink.
    #[error("{} is a symlink", .0.display())]
    Symlink(PathBuf),

    /// Scan root does not exist.
    #[error("{} not found", .0.display())]
    NotFound(PathBuf),

    /// Token list is empty after defaults are applied.
    #[error("no tokens to search for")]
    NoTokens,

    /// Max depth was zero or otherwise invalid.
    #[error("max_depth must be a positive integer")]
    InvalidMaxDepth,

    /// Root is neither a file nor a directory.
    #[error("{} must be path to file or dir", .0.display())]
    NotFileOrDir(PathBuf),

    /// Brace snippet count after a token is invalid.
    #[error("{0}")]
    InvalidSnippet(String),

    /// Exclude pattern failed to compile.
    #[error(transparent)]
    InvalidExclude(#[from] pathspec::Error),

    /// Ctrl-C handler could not be installed.
    #[error(transparent)]
    SignalHandler(#[from] ctrlc::Error),

    /// Underlying IO failure.
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// True for cooperative interrupt.
    pub fn is_interrupted(&self) -> bool {
        matches!(self, Self::Interrupted)
    }

    /// True for broken-pipe stop or StopExtraction.
    pub fn is_pipe_stop(&self) -> bool {
        match self {
            Self::StopExtraction => true,
            Self::Io(e) => e.kind() == io::ErrorKind::BrokenPipe,
            _ => false,
        }
    }

    /// True for invalid-data IO errors.
    pub fn is_invalid_data(&self) -> bool {
        matches!(self, Self::Io(e) if e.kind() == io::ErrorKind::InvalidData)
    }
}
