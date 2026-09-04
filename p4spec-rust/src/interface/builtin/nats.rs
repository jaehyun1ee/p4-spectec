//! Natural-number aggregation builtins in specification order.

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
    let value =
        num::Natural::try_from(value).map_err(|error| BuiltinError::new(error.to_string()))?;
    let value = make::nat(value, Span::default());
    Ok(value)
}

fn input_values(values: &[Rc<Value>]) -> Result<&[Rc<Value>], BuiltinError> {
    let value = extract::one(values)?;
    get::list(value).map_err(|error| BuiltinError::new(error.to_string()))
}

// == Built-in implementations

// dec $sum_nat(nat*) : nat

pub fn sum_nat(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    extract::zero(targs)?;
    let mut sum = BigInt::zero();
    for value in input_values(values)? {
        sum += bigint_of_value(value)?;
    }
    value_of_bigint(sum)
}

// dec $max_nat(nat*) : nat

pub fn max_nat(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    extract::zero(targs)?;
    let values = input_values(values)?;
    let (first, rest) = values
        .split_first()
        .ok_or_else(|| BuiltinError::new("max of empty list"))?;
    let mut maximum = bigint_of_value(first)?.clone();
    for value in rest {
        let value = bigint_of_value(value)?.clone();
        maximum = maximum.max(value);
    }
    value_of_bigint(maximum)
}

// dec $min_nat(nat*) : nat

pub fn min_nat(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    extract::zero(targs)?;
    let values = input_values(values)?;
    let (first, rest) = values
        .split_first()
        .ok_or_else(|| BuiltinError::new("min of empty list"))?;
    let mut minimum = bigint_of_value(first)?.clone();
    for value in rest {
        let value = bigint_of_value(value)?.clone();
        minimum = minimum.min(value);
    }
    value_of_bigint(minimum)
}
