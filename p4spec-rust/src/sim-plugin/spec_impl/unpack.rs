use num_bigint::BigInt;

use crate::{
    lang::{
        data::value::{Value, ValueArena, ValueError, get},
        xl::num,
    },
    runner::ExternError,
};

// == P4 values

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

pub fn p4_enum(arena: &ValueArena, value: &Value) -> Result<(String, String), ExternError> {
    get::matches! { arena, value,
        "tid '.' id" => |values| {
            let [value_enum, value_id] = values.as_slice() else {
                return Err(ValueError::ExpectedCount { expected: 2, actual: values.len() }.into());
            };
            Ok((get::text(arena, value_enum)?.to_owned(), get::text(arena, value_id)?.to_owned()))
        },
        _ => Err(ExternError::Failure("expected P4 enum value".to_owned())),
    }
}

pub fn p4_tuple(arena: &ValueArena, value: &Value) -> Result<Vec<Value>, ExternError> {
    get::matches! { arena, value,
        "TUPLE `( value* `)" => |values| {
            let [value_list] = values.as_slice() else {
                return Err(ValueError::ExpectedCount { expected: 1, actual: values.len() }.into());
            };
            Ok(get::list(arena, value_list)?.to_vec())
        },
        _ => Err(ExternError::Failure("expected P4 tuple value".to_owned())),
    }
}

// - Numbers

pub fn p4_fixed_bit(arena: &ValueArena, value: &Value) -> Result<(BigInt, BigInt), ExternError> {
    get::matches! { arena, value,
        "nat W int" => |values| {
            let [value_width, value_int] = values.as_slice() else {
                return Err(ValueError::ExpectedCount {
                    expected: 2,
                    actual: values.len(),
                }.into());
            };
            Ok((
                num::to_int(get::num(arena, value_width)?).clone(),
                num::to_int(get::num(arena, value_int)?).clone(),
            ))
        },
        _ => Err(ExternError::Failure("expected P4 fixed-bit value".to_owned())),
    }
}

pub fn p4_precision_number(
    arena: &ValueArena,
    value: &Value,
) -> Result<(BigInt, BigInt), ExternError> {
    get::matches! { arena, value,
        "nat W int" | "nat S int" => |values| {
            let [value_width, value_int] = values.as_slice() else {
                return Err(ValueError::ExpectedCount { expected: 2, actual: values.len() }.into());
            };
            Ok((
                num::to_int(get::num(arena, value_width)?).clone(),
                num::to_int(get::num(arena, value_int)?).clone(),
            ))
        },
        "nat '.' nat V int" => |values| {
            let [value_width_max, value_width, value_int] = values.as_slice() else {
                return Err(ValueError::ExpectedCount { expected: 3, actual: values.len() }.into());
            };
            get::num(arena, value_width_max)?;
            Ok((
                num::to_int(get::num(arena, value_width)?).clone(),
                num::to_int(get::num(arena, value_int)?).clone(),
            ))
        },
        _ => Err(ExternError::Failure("expected P4 precision number value".to_owned())),
    }
}

// == Arguments

pub fn assoc_args(
    arena: &ValueArena,
    value_ids: Value,
    value_args: Value,
) -> Result<Vec<(String, Value)>, ExternError> {
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
    Ok(names.into_iter().zip(values.iter().copied()).collect())
}

/// Finds the first argument with the requested name
pub fn find_arg(args: &[(String, Value)], name: &str) -> Result<Value, ExternError> {
    args.iter()
        .find(|(name_arg, _)| name_arg == name)
        .map(|(_, value)| *value)
        .ok_or_else(|| ExternError::Failure(format!("argument not found: {name}")))
}
