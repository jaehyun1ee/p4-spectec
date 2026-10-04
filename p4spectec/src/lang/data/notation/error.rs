//! Errors of notation operations

use std::{error::Error, fmt, num::TryFromIntError};

use thiserror::Error as ThisError;

/// An error caused by a mismatch between mixop arity and supplied arguments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArityMismatch {
    /// Fewer arguments were supplied than the mixop requires.
    ArgumentCountTooFew,
    /// More arguments were supplied than the mixop requires.
    ArgumentCountTooMany,
}

impl fmt::Display for ArityMismatch {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArgumentCountTooFew => fmt.write_str("Mixop.fill: too few arguments"),
            Self::ArgumentCountTooMany => fmt.write_str("Mixop.fill: too many arguments"),
        }
    }
}

impl Error for ArityMismatch {}

/// A failure interning a shape.
#[derive(Clone, Copy, Debug, ThisError, Eq, PartialEq)]
pub enum ShapeError {
    /// The arena ran out of 32-bit handles.
    #[error("shape arena index overflow")]
    IndexOverflow,
}

// - Index overflow

impl From<TryFromIntError> for ShapeError {
    fn from(_: TryFromIntError) -> Self {
        Self::IndexOverflow
    }
}
