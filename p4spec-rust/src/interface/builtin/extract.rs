use crate::domain::source::Region;

use super::BuiltinError;

// Shorthand for extracting type arguments and values

fn arity(span: &Region) -> BuiltinError {
    BuiltinError::new(span.clone(), "arity mismatch")
}

pub fn zero<T>(span: &Region, values: &[T]) -> Result<(), BuiltinError> {
    match values {
        [] => Ok(()),
        _ => Err(arity(span)),
    }
}

pub fn one<'a, T>(span: &Region, values: &'a [T]) -> Result<&'a T, BuiltinError> {
    match values {
        [value] => Ok(value),
        _ => Err(arity(span)),
    }
}

pub fn two<'a, T>(span: &Region, values: &'a [T]) -> Result<(&'a T, &'a T), BuiltinError> {
    match values {
        [value_a, value_b] => Ok((value_a, value_b)),
        _ => Err(arity(span)),
    }
}

pub fn three<'a, T>(span: &Region, values: &'a [T]) -> Result<(&'a T, &'a T, &'a T), BuiltinError> {
    match values {
        [value_a, value_b, value_c] => Ok((value_a, value_b, value_c)),
        _ => Err(arity(span)),
    }
}

pub fn four<'a, T>(
    span: &Region,
    values: &'a [T],
) -> Result<(&'a T, &'a T, &'a T, &'a T), BuiltinError> {
    match values {
        [value_a, value_b, value_c, value_d] => Ok((value_a, value_b, value_c, value_d)),
        _ => Err(arity(span)),
    }
}
