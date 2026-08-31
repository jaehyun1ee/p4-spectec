use num_bigint::BigInt;

use crate::{
    lang::common::source::Span,
    lang::{il::ast::Typ, xl::num},
    runtime::{
        types::typ as make_type,
        value::{ValueRef, get, make},
    },
};

use super::{BuiltinError, BuiltinResult, extract, return_value};

fn text_of_value<'a>(span: &Span, value: &'a ValueRef) -> Result<&'a str, BuiltinError> {
    get::text(value).map_err(|error| BuiltinError::new(span.clone(), error.to_string()))
}

fn numeric_text(span: &Span, value: &ValueRef) -> Result<String, BuiltinError> {
    match get::num(value).map_err(|error| BuiltinError::new(span.clone(), error.to_string()))? {
        num::Number::Nat(value) => Ok(value.to_string()),
        num::Number::Int(value) => Ok(value.to_string()),
    }
}

// dec $text_to_int(text) : int

pub fn text_to_int(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let text = text_of_value(span, extract::one(span, values)?)?;
    let (negative, unsigned) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    let (radix, digits) = if let Some(digits) = unsigned.strip_prefix("0x") {
        (16, digits)
    } else if let Some(digits) = unsigned.strip_prefix("0X") {
        (16, digits)
    } else if let Some(digits) = unsigned.strip_prefix("0o") {
        (8, digits)
    } else if let Some(digits) = unsigned.strip_prefix("0O") {
        (8, digits)
    } else if let Some(digits) = unsigned.strip_prefix("0b") {
        (2, digits)
    } else if let Some(digits) = unsigned.strip_prefix("0B") {
        (2, digits)
    } else {
        (10, unsigned)
    };
    let mut integer = BigInt::parse_bytes(digits.as_bytes(), radix)
        .ok_or_else(|| BuiltinError::new(span.clone(), "invalid digit found in string"))?;
    if negative {
        integer = -integer;
    }
    return_value(add, make::int(integer, Span::default()))
}

// dec $int_to_text(int) : text

pub fn int_to_text(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let text = numeric_text(span, extract::one(span, values)?)?;
    return_value(add, make::text(text, Span::default()))
}

// dec $split_text(text, text) : text*

pub fn split_text(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let (value_text, value_separator) = extract::two(span, values)?;
    let text = text_of_value(span, value_text)?;
    let separator = text_of_value(span, value_separator)?;
    if separator.len() != 1 {
        return Err(BuiltinError::new(
            span.clone(),
            "separator must be one byte",
        ));
    }
    let separator = char::from(separator.as_bytes()[0]);
    let parts = text
        .split(separator)
        .map(|part| make::text(part.to_owned(), Span::default()))
        .collect();
    let list_type = make_type::list(make_type::bool());
    return_value(add, make::list(&list_type, parts, Span::default()))
}

// dec $strip_prefix(text, text) : text

pub fn strip_prefix(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let (value_text, value_prefix) = extract::two(span, values)?;
    let text = text_of_value(span, value_text)?;
    let prefix = text_of_value(span, value_prefix)?;
    let text = text
        .strip_prefix(prefix)
        .ok_or_else(|| BuiltinError::new(span.clone(), "text does not start with prefix"))?;
    return_value(add, make::text(text.to_owned(), Span::default()))
}

// dec $strip_suffix(text, text) : text

pub fn strip_suffix(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let (value_text, value_suffix) = extract::two(span, values)?;
    let text = text_of_value(span, value_text)?;
    let suffix = text_of_value(span, value_suffix)?;
    let text = text
        .strip_suffix(suffix)
        .ok_or_else(|| BuiltinError::new(span.clone(), "text does not end with suffix"))?;
    return_value(add, make::text(text.to_owned(), Span::default()))
}

// dec $strip_all_whitespace(text) : text

pub fn strip_all_whitespace(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    let text = text_of_value(span, extract::one(span, values)?)?.replace(' ', "");
    return_value(add, make::text(text, Span::default()))
}
