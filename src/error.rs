use std::{io, path::PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("interrupted")]
    Interrupted,

    #[error("stop extraction")]
    StopExtraction,

    #[error("{} is a symlink", .0.display())]
    Symlink(PathBuf),

    #[error("{} not found", .0.display())]
    NotFound(PathBuf),

    #[error("no tokens to search for")]
    NoTokens,

    #[error("max_depth must be a positive integer")]
    InvalidMaxDepth,

    #[error("{} must be path to file or dir", .0.display())]
    NotFileOrDir(PathBuf),

    #[error("{0}")]
    InvalidSnippet(String),

    #[error(transparent)]
    InvalidExclude(#[from] pathspec::Error),

    #[error(transparent)]
    SignalHandler(#[from] ctrlc::Error),

    #[error(transparent)]
    Io(#[from] io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn is_interrupted(&self) -> bool {
        matches!(self, Self::Interrupted)
    }

    pub fn is_pipe_stop(&self) -> bool {
        match self {
            Self::StopExtraction => true,
            Self::Io(e) => e.kind() == io::ErrorKind::BrokenPipe,
            _ => false,
        }
    }

    pub fn is_invalid_data(&self) -> bool {
        matches!(self, Self::Io(e) if e.kind() == io::ErrorKind::InvalidData)
    }
}
