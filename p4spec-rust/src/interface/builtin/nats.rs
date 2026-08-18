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
    return_value(add, make::nat(value, Region::none()))
}

fn input_values<'a>(span: &Region, values: &'a [ValueRef]) -> Result<&'a [ValueRef], BuiltinError> {
    let value = extract::one(span, values)?;
    get::list(value).map_err(|error| BuiltinError::new(span.clone(), error.to_string()))
}

// dec $sum_nat(nat* ) : nat

pub fn sum_nat(
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

// dec $max_nat(nat* ) : nat

pub fn max_nat(
    add: &mut dyn FnMut(ValueRef),
    span: &Region,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let values = input_values(span, values)?;
    let (first, rest) = values
        .split_first()
        .ok_or_else(|| BuiltinError::new(span.clone(), "max of empty list"))?;
    let mut maximum = bigint_of_value(span, first)?.clone();
    for value in rest {
        maximum = maximum.max(bigint_of_value(span, value)?.clone());
    }
    value_of_bigint(add, maximum)
}

// dec $min_nat(nat* ) : nat

pub fn min_nat(
    add: &mut dyn FnMut(ValueRef),
    span: &Region,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let values = input_values(span, values)?;
    let (first, rest) = values
        .split_first()
        .ok_or_else(|| BuiltinError::new(span.clone(), "min of empty list"))?;
    let mut minimum = bigint_of_value(span, first)?.clone();
    for value in rest {
        minimum = minimum.min(bigint_of_value(span, value)?.clone());
    }
    value_of_bigint(add, minimum)
}
