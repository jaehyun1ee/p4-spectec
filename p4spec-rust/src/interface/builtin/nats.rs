//! Natural-number aggregation builtins, ordered to match `nats.ml`.

use num_bigint::BigInt;
use num_traits::Zero;

use crate::{
    lang::common::source::Span,
    lang::{il::ast::Typ, xl::num},
    runtime::value::{Value, ValueRef, get, make},
};

use super::{BuiltinError, BuiltinResult, extract, return_value};

// == Conversion between meta-numerics and Rust numerics

fn bigint_of_value<'a>(span: &Span, value: &'a Value) -> Result<&'a BigInt, BuiltinError> {
    let number =
        get::num(value).map_err(|error| BuiltinError::new(span.clone(), error.to_string()))?;
    Ok(num::to_int(number))
}

fn value_of_bigint(add: &mut dyn FnMut(ValueRef), value: BigInt) -> BuiltinResult {
    let value = num::Natural::try_from(value)
        .map_err(|error| BuiltinError::new(Span::default(), error.to_string()))?;
    let value = make::nat(value, Span::default());
    return_value(add, value)
}

fn input_values<'a>(span: &Span, values: &'a [ValueRef]) -> Result<&'a [ValueRef], BuiltinError> {
    let value = extract::one(span, values)?;
    get::list(value).map_err(|error| BuiltinError::new(span.clone(), error.to_string()))
}

// == Built-in implementations

// dec $sum_nat(nat* ) : nat

pub fn sum_nat(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
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
    span: &Span,
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
        let value = bigint_of_value(span, value)?.clone();
        maximum = maximum.max(value);
    }
    value_of_bigint(add, maximum)
}

// dec $min_nat(nat* ) : nat

pub fn min_nat(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
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
        let value = bigint_of_value(span, value)?.clone();
        minimum = minimum.min(value);
    }
    value_of_bigint(add, minimum)
}
