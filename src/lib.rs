//! Extract TODO-style tokens from a file tree.

mod cli;
mod error;
mod exclude;
mod extractor;
mod pager;
mod writer;

pub use cli::Cli;
pub use error::{Error, Result};
pub use extractor::{Extractor, ExtractorConfig};
pub use writer::{OutputDest, Writer};
