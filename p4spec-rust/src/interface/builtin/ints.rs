//! Integer aggregation builtins in specification order.

use std::rc::Rc;

use num_bigint::BigInt;
use num_traits::Zero;

use crate::{
    lang::common::source::Span,
    lang::data::value::{Value, get, make},
    lang::{il::ast::Typ, xl::num},
};

use super::{BuiltinError, extract};

// == Conversion between meta-numerics and Rust numerics

fn bigint_of_value(value: &Value) -> Result<&BigInt, BuiltinError> {
    let number = get::num(value).map_err(|error| BuiltinError::new(error.to_string()))?;
    Ok(num::to_int(number))
}

fn value_of_bigint(value: BigInt) -> Result<Rc<Value>, BuiltinError> {
    let value = make::int(value, Span::default());
    Ok(value)
}

fn input_values(values: &[Rc<Value>]) -> Result<&[Rc<Value>], BuiltinError> {
    let value = extract::one(values)?;
    get::list(value).map_err(|error| BuiltinError::new(error.to_string()))
}

// == Built-in implementations

// dec $sum_int(nat*) : nat

pub fn sum_int(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    extract::zero(targs)?;
    let mut sum = BigInt::zero();
    for value in input_values(values)? {
        sum += bigint_of_value(value)?;
    }
    value_of_bigint(sum)
}

// dec $max_int(int*) : int

pub fn max_int(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    extract::zero(targs)?;
    let values = input_values(values)?;
    let mut maximum = match values.split_first() {
        Some((first, _)) => {
            let first = bigint_of_value(first)?;
            first.clone()
        }
        None => BigInt::zero(),
    };
    for value in values.get(1..).unwrap_or_default() {
        let value = bigint_of_value(value)?.clone();
        maximum = maximum.max(value);
    }
    value_of_bigint(maximum)
}

// dec $min_int(int*) : int

pub fn min_int(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    extract::zero(targs)?;
    let values = input_values(values)?;
    let mut minimum = match values.split_first() {
        Some((first, _)) => {
            let first = bigint_of_value(first)?;
            first.clone()
        }
        None => BigInt::zero(),
    };
    for value in values.get(1..).unwrap_or_default() {
        let value = bigint_of_value(value)?.clone();
        minimum = minimum.min(value);
    }
    value_of_bigint(minimum)
}
