pub mod counter;
pub mod hash;
pub mod internet_checksum;
pub mod meter;
pub mod register;

pub use counter::Counter;
pub use hash::HashExtern;
pub use internet_checksum::InternetChecksum;
pub use meter::{Color, Meter};
pub use register::Register;

use crate::runner::ExternError;

fn repeat<Value: Clone>(value: Value, len: i64) -> Result<Vec<Value>, ExternError> {
    let len = usize::try_from(len)
        .map_err(|_| ExternError::Failure("negative object size".to_owned()))?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(len)
        .map_err(|error| ExternError::Failure(error.to_string()))?;
    values.resize(len, value);
    Ok(values)
}
