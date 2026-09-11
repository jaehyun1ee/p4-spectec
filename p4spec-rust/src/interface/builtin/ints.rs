//! Integer aggregation builtins in specification order.

use num_bigint::BigInt;
use num_traits::Zero;

use crate::{
    lang::common::source::Span,
    lang::data::value::{Value, ValueArena, get, make},
    lang::{il::ast::Typ, xl::num},
};

use super::{BuiltinError, extract};

// == Conversion between meta-numerics and Rust numerics

fn bigint_of_value<'a>(arena: &'a ValueArena, value: &Value) -> Result<&'a BigInt, BuiltinError> {
    let num = get::num(arena, value).map_err(|error| BuiltinError::new(error.to_string()))?;
    Ok(num::to_int(num))
}

fn value_of_bigint(arena: &mut ValueArena, value: BigInt) -> Result<Value, BuiltinError> {
    let value = make::int(arena, value, Span::default())?;
    Ok(value)
}

fn input_values<'a>(arena: &'a ValueArena, values: &[Value]) -> Result<&'a [Value], BuiltinError> {
    let value = extract::one(values)?;
    get::list(arena, value).map_err(|error| BuiltinError::new(error.to_string()))
}

// == Built-in implementations

// dec $sum_int(nat*) : nat

pub fn sum_int(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    extract::zero(targs)?;
    let mut sum = BigInt::zero();
    for value in input_values(arena, values)? {
        sum += bigint_of_value(arena, value)?;
    }
    value_of_bigint(arena, sum)
}

// dec $max_int(int*) : int

pub fn max_int(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    extract::zero(targs)?;
    let values = input_values(arena, values)?;
    let mut maximum = match values.split_first() {
        Some((first, _)) => {
            let first = bigint_of_value(arena, first)?;
            first.clone()
        }
        None => BigInt::zero(),
    };
    for value in values.get(1..).unwrap_or_default() {
        let value = bigint_of_value(arena, value)?.clone();
        maximum = maximum.max(value);
    }
    value_of_bigint(arena, maximum)
}

// dec $min_int(int*) : int

pub fn min_int(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    extract::zero(targs)?;
    let values = input_values(arena, values)?;
    let mut minimum = match values.split_first() {
        Some((first, _)) => {
            let first = bigint_of_value(arena, first)?;
            first.clone()
        }
        None => BigInt::zero(),
    };
    for value in values.get(1..).unwrap_or_default() {
        let value = bigint_of_value(arena, value)?.clone();
        minimum = minimum.min(value);
    }
    value_of_bigint(arena, minimum)
}
