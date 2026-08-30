use crate::lang::common::source::Span;

use super::BuiltinError;

// Shorthand for extracting type arguments and values

pub fn zero<T>(span: &Span, values: &[T]) -> Result<(), BuiltinError> {
    match values {
        [] => Ok(()),
        _ => Err(BuiltinError::arity(span.clone(), 0, values.len())),
    }
}

pub fn one<'a, T>(span: &Span, values: &'a [T]) -> Result<&'a T, BuiltinError> {
    match values {
        [value] => Ok(value),
        _ => Err(BuiltinError::arity(span.clone(), 1, values.len())),
    }
}

pub fn two<'a, T>(span: &Span, values: &'a [T]) -> Result<(&'a T, &'a T), BuiltinError> {
    match values {
        [value_a, value_b] => Ok((value_a, value_b)),
        _ => Err(BuiltinError::arity(span.clone(), 2, values.len())),
    }
}

pub fn three<'a, T>(span: &Span, values: &'a [T]) -> Result<(&'a T, &'a T, &'a T), BuiltinError> {
    match values {
        [value_a, value_b, value_c] => Ok((value_a, value_b, value_c)),
        _ => Err(BuiltinError::arity(span.clone(), 3, values.len())),
    }
}

pub fn four<'a, T>(
    span: &Span,
    values: &'a [T],
) -> Result<(&'a T, &'a T, &'a T, &'a T), BuiltinError> {
    match values {
        [value_a, value_b, value_c, value_d] => Ok((value_a, value_b, value_c, value_d)),
        _ => Err(BuiltinError::arity(span.clone(), 4, values.len())),
    }
}
