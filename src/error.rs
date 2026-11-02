use std::{
    error,
    fmt::{self, Display, Formatter},
};

#[derive(Debug)]
pub struct StopExtractionError;

impl Display for StopExtractionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "stop extraction")
    }
}

impl error::Error for StopExtractionError {}

#[derive(Debug)]
pub struct InterruptedError;

impl Display for InterruptedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "interrupted")
    }
}

impl error::Error for InterruptedError {}
