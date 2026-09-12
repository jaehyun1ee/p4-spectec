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

use crate::sim_plugin::spec_impl::{pack, rel::CallResult};
use crate::{
    lang::data::value::{Value, ValueArena},
    runner::ExternError,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectResult<Object> {
    pub object: Object,
    pub result: CallResult,
}

fn finish<Object>(
    arena: &mut ValueArena,
    object: Object,
    value_ctx: Value,
    value_arch: Value,
    value: Option<Value>,
) -> Result<ObjectResult<Object>, ExternError> {
    let value_call_result = pack::return_result(arena, value)?;
    Ok(ObjectResult {
        object,
        result: CallResult {
            value_ctx,
            value_arch,
            value_call_result,
        },
    })
}

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
