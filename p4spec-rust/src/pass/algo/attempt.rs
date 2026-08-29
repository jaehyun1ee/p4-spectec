//! Controlled failures used while trying algorithmic conversions

use super::AlgoError;

pub(super) type Attempt<T> = Result<T, AlgoError>;

pub(super) fn fail<T>(error: AlgoError) -> Attempt<T> {
    Err(error)
}
