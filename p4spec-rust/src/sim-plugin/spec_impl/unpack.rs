use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::{
    lang::{
        data::value::{Value, ValueArena, ValueError, get},
        xl::num,
    },
    runner::ExternError,
};

pub fn p4_bool(arena: &ValueArena, value: &Value) -> Result<bool, ExternError> {
    get::matches! { arena,
        value,
        "_B bool" => |values| {
            let [value] = values.as_slice() else {
                return Err(ValueError::ExpectedCount {
                    expected: 1,
                    actual: values.len(),
                }.into());
            };
            get::bool(arena, value).map_err(ExternError::from)
        },
        _ => Err(ExternError::Failure("expected P4 bool value".to_owned())),
    }
}

pub fn p4_string(arena: &ValueArena, value: &Value) -> Result<String, ExternError> {
    get::matches! { arena,
        value,
        "'\"' text '\"'" => |values| {
            let [value] = values.as_slice() else {
                return Err(ValueError::ExpectedCount {
                    expected: 1,
                    actual: values.len(),
                }.into());
            };
            get::text(arena, value).map(str::to_owned).map_err(ExternError::from)
        },
        _ => Err(ExternError::Failure("expected P4 string value".to_owned())),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrecisionNumber {
    pub width: BigInt,
    pub int: BigInt,
}

pub fn p4_fixed_bit(arena: &ValueArena, value: &Value) -> Result<PrecisionNumber, ExternError> {
    get::matches! { arena, value,
        "nat W int" => |values| {
            let [value_width, value_int] = values.as_slice() else {
                return Err(ValueError::ExpectedCount {
                    expected: 2,
                    actual: values.len(),
                }.into());
            };
            Ok(PrecisionNumber {
                width: num::to_int(get::num(arena, value_width)?).clone(),
                int: num::to_int(get::num(arena, value_int)?).clone(),
            })
        },
        _ => Err(ExternError::Failure("expected P4 fixed-bit value".to_owned())),
    }
}

pub fn size(int: &BigInt) -> Result<usize, ExternError> {
    int.to_u64()
        .filter(|size| *size <= ((1_u64 << 62) - 1))
        .and_then(|size| usize::try_from(size).ok())
        .ok_or_else(|| ExternError::Failure(format!("invalid packet size: {int}")))
}

pub fn p4_precision_number(
    arena: &ValueArena,
    value: &Value,
) -> Result<PrecisionNumber, ExternError> {
    get::matches! { arena, value,
        "nat W int" | "nat S int" => |values| {
            let [value_width, value_int] = values.as_slice() else {
                return Err(ValueError::ExpectedCount { expected: 2, actual: values.len() }.into());
            };
            Ok(PrecisionNumber {
                width: num::to_int(get::num(arena, value_width)?).clone(),
                int: num::to_int(get::num(arena, value_int)?).clone(),
            })
        },
        "nat '.' nat V int" => |values| {
            let [value_width_max, value_width, value_int] = values.as_slice() else {
                return Err(ValueError::ExpectedCount { expected: 3, actual: values.len() }.into());
            };
            get::num(arena, value_width_max)?;
            Ok(PrecisionNumber {
                width: num::to_int(get::num(arena, value_width)?).clone(),
                int: num::to_int(get::num(arena, value_int)?).clone(),
            })
        },
        _ => Err(ExternError::Failure("expected P4 precision number value".to_owned())),
    }
}

#[derive(Clone, Debug)]
pub struct ArgumentValue {
    pub name: String,
    pub value: Value,
}

pub fn assoc_args(
    arena: &ValueArena,
    value_ids: Value,
    value_args: Value,
) -> Result<Vec<ArgumentValue>, ExternError> {
    let names = get::list(arena, &value_ids)?
        .iter()
        .map(|value_id| get::text(arena, value_id).map(str::to_owned))
        .collect::<Result<Vec<_>, _>>()?;
    let values = get::list(arena, &value_args)?;
    if names.len() != values.len() {
        return Err(ValueError::ExpectedCount {
            expected: names.len(),
            actual: values.len(),
        }
        .into());
    }
    Ok(names
        .into_iter()
        .zip(values)
        .map(|(name, value)| ArgumentValue {
            name,
            value: *value,
        })
        .collect())
}

pub fn signed_int(int: &BigInt) -> Result<i64, ExternError> {
    int.to_i64()
        .filter(|int| (-(1_i64 << 62)..(1_i64 << 62)).contains(int))
        .ok_or_else(|| ExternError::Failure(format!("integer outside OCaml int range: {int}")))
}
