//! Fixed-arity argument extraction for builtin calls.
//!
//! Each helper returns borrowed arguments on an exact match and an arity error
//! otherwise; for example, `two(&[a, b])` returns `(a, b)`.

use super::BuiltinError;

// == Shorthand for extracting type arguments and values

pub fn zero<T>(values: &[T]) -> Result<(), BuiltinError> {
    match values {
        [] => Ok(()),
        _ => Err(BuiltinError::arity(0, values.len())),
    }
}

pub fn one<T>(values: &[T]) -> Result<&T, BuiltinError> {
    match values {
        [value] => Ok(value),
        _ => Err(BuiltinError::arity(1, values.len())),
    }
}

pub fn two<T>(values: &[T]) -> Result<(&T, &T), BuiltinError> {
    match values {
        [value_a, value_b] => Ok((value_a, value_b)),
        _ => Err(BuiltinError::arity(2, values.len())),
    }
}

pub fn three<T>(values: &[T]) -> Result<(&T, &T, &T), BuiltinError> {
    match values {
        [value_a, value_b, value_c] => Ok((value_a, value_b, value_c)),
        _ => Err(BuiltinError::arity(3, values.len())),
    }
}

pub fn four<T>(values: &[T]) -> Result<(&T, &T, &T, &T), BuiltinError> {
    match values {
        [value_a, value_b, value_c, value_d] => Ok((value_a, value_b, value_c, value_d)),
        _ => Err(BuiltinError::arity(4, values.len())),
    }
}
