//! Errors of building and projecting values

use std::num::TryFromIntError;

use thiserror::Error;

use crate::lang::data::notation::MixopError;

use super::ValueTag;

/// A failure building or projecting a value.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ValueError {
    /// The arena ran out of 32-bit handles.
    #[error("value arena index overflow")]
    IndexOverflow,
    /// A projection met a value of another kind.
    #[error("expected {expected:?} value, got {actual:?}")]
    KindMismatch { expected: ValueTag, actual: ValueTag },
    /// An element index past the end.
    #[error("value index {index} is out of bounds for length {len}")]
    IndexOutOfBounds { index: usize, len: usize },
    /// A fixed-arity projection met another count.
    #[error("expected exactly {expected} values, got {actual}")]
    CountMismatch { expected: usize, actual: usize },
}

// - Index overflow

impl From<TryFromIntError> for ValueError {
    fn from(_: TryFromIntError) -> Self {
        Self::IndexOverflow
    }
}

impl From<MixopError> for ValueError {
    fn from(error: MixopError) -> Self {
        match error {
            MixopError::IndexOverflow => Self::IndexOverflow,
        }
    }
}
