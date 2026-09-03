//! Integer aggregation builtins, ordered to match `ints.ml`.

use num_bigint::BigInt;
use num_traits::Zero;

use crate::{
    lang::common::source::Span,
    lang::{il::ast::Typ, xl::num},
    runtime::value::{Value, ValueRef, get, make},
};

use super::{BuiltinError, BuiltinResult, extract};

// == Conversion between meta-numerics and Rust numerics

fn bigint_of_value<'a>(span: &Span, value: &'a Value) -> Result<&'a BigInt, BuiltinError> {
    let number =
        get::num(value).map_err(|error| BuiltinError::new(span.clone(), error.to_string()))?;
    Ok(num::to_int(number))
}

fn value_of_bigint(value: BigInt) -> BuiltinResult {
    let value = make::int(value, Span::default());
    Ok(value)
}

fn input_values<'a>(span: &Span, values: &'a [ValueRef]) -> Result<&'a [ValueRef], BuiltinError> {
    let value = extract::one(span, values)?;
    get::list(value).map_err(|error| BuiltinError::new(span.clone(), error.to_string()))
}

// == Built-in implementations

// dec $sum_int(nat*) : nat

pub fn sum_int(span: &Span, targs: &[Typ], values: &[ValueRef]) -> BuiltinResult {
    extract::zero(span, targs)?;
    let mut sum = BigInt::zero();
    for value in input_values(span, values)? {
        sum += bigint_of_value(span, value)?;
    }
    value_of_bigint(sum)
}

// dec $max_int(int*) : int

pub fn max_int(span: &Span, targs: &[Typ], values: &[ValueRef]) -> BuiltinResult {
    extract::zero(span, targs)?;
    let values = input_values(span, values)?;
    let mut maximum = match values.split_first() {
        Some((first, _)) => {
            let first = bigint_of_value(span, first)?;
            first.clone()
        }
        None => BigInt::zero(),
    };
    for value in values.get(1..).unwrap_or_default() {
        let value = bigint_of_value(span, value)?.clone();
        maximum = maximum.max(value);
    }
    value_of_bigint(maximum)
}

// dec $min_int(int*) : int

pub fn min_int(span: &Span, targs: &[Typ], values: &[ValueRef]) -> BuiltinResult {
    extract::zero(span, targs)?;
    let values = input_values(span, values)?;
    let mut minimum = match values.split_first() {
        Some((first, _)) => {
            let first = bigint_of_value(span, first)?;
            first.clone()
        }
        None => BigInt::zero(),
    };
    for value in values.get(1..).unwrap_or_default() {
        let value = bigint_of_value(span, value)?.clone();
        minimum = minimum.min(value);
    }
    value_of_bigint(minimum)
}
