use num_bigint::BigInt;
use num_traits::Zero;

use crate::{
    domain::source::Region,
    lang::{il::ast::Typ, xl::num},
    runtime::value::{Value, ValueRef, get, make},
};

use super::{BuiltinError, BuiltinResult, extract, return_value};

// Conversion between meta-numerics and Rust numerics

fn bigint_of_value<'a>(span: &Region, value: &'a Value) -> Result<&'a BigInt, BuiltinError> {
    match get::num(value).map_err(|error| BuiltinError::new(span.clone(), error.to_string()))? {
        num::T::Nat(value) | num::T::Int(value) => Ok(value),
    }
}

fn value_of_bigint(add: &mut dyn FnMut(ValueRef), value: BigInt) -> BuiltinResult {
    return_value(add, make::int(value, Region::none()))
}

fn input_values<'a>(span: &Region, values: &'a [ValueRef]) -> Result<&'a [ValueRef], BuiltinError> {
    let value = extract::one(span, values)?;
    get::list(value).map_err(|error| BuiltinError::new(span.clone(), error.to_string()))
}

// dec $sum_int(nat* ) : nat

pub fn sum_int(
    add: &mut dyn FnMut(ValueRef),
    span: &Region,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let mut sum = BigInt::zero();
    for value in input_values(span, values)? {
        sum += bigint_of_value(span, value)?;
    }
    value_of_bigint(add, sum)
}

// dec $max_int(int* ) : int

pub fn max_int(
    add: &mut dyn FnMut(ValueRef),
    span: &Region,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let values = input_values(span, values)?;
    let mut maximum = match values.split_first() {
        Some((first, _)) => bigint_of_value(span, first)?.clone(),
        None => BigInt::zero(),
    };
    for value in values.get(1..).unwrap_or_default() {
        maximum = maximum.max(bigint_of_value(span, value)?.clone());
    }
    value_of_bigint(add, maximum)
}

// dec $min_int(int* ) : int

pub fn min_int(
    add: &mut dyn FnMut(ValueRef),
    span: &Region,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let values = input_values(span, values)?;
    let mut minimum = match values.split_first() {
        Some((first, _)) => bigint_of_value(span, first)?.clone(),
        None => BigInt::zero(),
    };
    for value in values.get(1..).unwrap_or_default() {
        minimum = minimum.min(bigint_of_value(span, value)?.clone());
    }
    value_of_bigint(add, minimum)
}
