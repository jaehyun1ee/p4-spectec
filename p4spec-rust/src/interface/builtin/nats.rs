//! Natural-number aggregation builtins in specification order.

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
    let number = get::num(arena, value).map_err(|error| BuiltinError::new(error.to_string()))?;
    Ok(num::to_int(number))
}

fn value_of_bigint(arena: &mut ValueArena, value: BigInt) -> Result<Value, BuiltinError> {
    let value =
        num::Natural::try_from(value).map_err(|error| BuiltinError::new(error.to_string()))?;
    let value = make::nat(arena, value, Span::default())?;
    Ok(value)
}

fn input_values<'a>(arena: &'a ValueArena, values: &[Value]) -> Result<&'a [Value], BuiltinError> {
    let value = extract::one(values)?;
    get::list(arena, value).map_err(|error| BuiltinError::new(error.to_string()))
}

// == Built-in implementations

// dec $sum_nat(arena, nat*) : nat

pub fn sum_nat(
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

// dec $max_nat(arena, nat*) : nat

pub fn max_nat(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    extract::zero(targs)?;
    let values = input_values(arena, values)?;
    let (first, rest) = values
        .split_first()
        .ok_or_else(|| BuiltinError::new("max of empty list"))?;
    let mut maximum = bigint_of_value(arena, first)?.clone();
    for value in rest {
        let value = bigint_of_value(arena, value)?.clone();
        maximum = maximum.max(value);
    }
    value_of_bigint(arena, maximum)
}

// dec $min_nat(arena, nat*) : nat

pub fn min_nat(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    extract::zero(targs)?;
    let values = input_values(arena, values)?;
    let (first, rest) = values
        .split_first()
        .ok_or_else(|| BuiltinError::new("min of empty list"))?;
    let mut minimum = bigint_of_value(arena, first)?.clone();
    for value in rest {
        let value = bigint_of_value(arena, value)?.clone();
        minimum = minimum.min(value);
    }
    value_of_bigint(arena, minimum)
}
