//! Errors of notation operations

use std::{error::Error, fmt};

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
