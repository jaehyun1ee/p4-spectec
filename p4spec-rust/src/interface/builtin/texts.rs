//! Text builtins, preserving the function order of the OCaml implementation.
//!
//! Arguments are decoded from runtime values before the textual operation is
//! performed, then the encoded result is returned. For example,
//! `strip_prefix("prebody", "pre")` yields `"body"`.

use std::rc::Rc;

use num_bigint::BigInt;

use crate::{
    lang::common::source::Span,
    lang::{il::ast::Typ, traits::print::Print},
    runtime::{
        types::typ,
        value::{Value, get, make},
    },
};

use super::{BuiltinError, BuiltinResult, extract};

// == Conversion between runtime values and text

fn text_of_value<'a>(span: &Span, value: &'a Rc<Value>) -> Result<&'a str, BuiltinError> {
    get::text(value).map_err(|error| BuiltinError::new(span.clone(), error.to_string()))
}

fn numeric_text(span: &Span, value: &Rc<Value>) -> Result<String, BuiltinError> {
    let number =
        get::num(value).map_err(|error| BuiltinError::new(span.clone(), error.to_string()))?;
    Ok(Print::to_string(number))
}

// == Built-in implementations

// dec $text_to_int(text) : int

pub fn text_to_int(span: &Span, targs: &[Typ], values: &[Rc<Value>]) -> BuiltinResult {
    extract::zero(span, targs)?;
    let value_text = extract::one(span, values)?;
    let text = text_of_value(span, value_text)?;
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
    let value = make::int(integer, Span::default());
    Ok(value)
}

// dec $int_to_text(int) : text

pub fn int_to_text(span: &Span, targs: &[Typ], values: &[Rc<Value>]) -> BuiltinResult {
    extract::zero(span, targs)?;
    let value_int = extract::one(span, values)?;
    let text = numeric_text(span, value_int)?;
    let value = make::text(text, Span::default());
    Ok(value)
}

// dec $split_text(text, text) : text*

pub fn split_text(span: &Span, targs: &[Typ], values: &[Rc<Value>]) -> BuiltinResult {
    extract::zero(span, targs)?;
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
    let typ_list = typ::list(typ::bool());
    let value = make::list(&typ_list, parts, Span::default());
    Ok(value)
}

// dec $strip_prefix(text, text) : text

pub fn strip_prefix(span: &Span, targs: &[Typ], values: &[Rc<Value>]) -> BuiltinResult {
    extract::zero(span, targs)?;
    let (value_text, value_prefix) = extract::two(span, values)?;
    let text = text_of_value(span, value_text)?;
    let prefix = text_of_value(span, value_prefix)?;
    let text = text
        .strip_prefix(prefix)
        .ok_or_else(|| BuiltinError::new(span.clone(), "text does not start with prefix"))?;
    let value = make::text(text.to_owned(), Span::default());
    Ok(value)
}

// dec $strip_suffix(text, text) : text

pub fn strip_suffix(span: &Span, targs: &[Typ], values: &[Rc<Value>]) -> BuiltinResult {
    extract::zero(span, targs)?;
    let (value_text, value_suffix) = extract::two(span, values)?;
    let text = text_of_value(span, value_text)?;
    let suffix = text_of_value(span, value_suffix)?;
    let text = text
        .strip_suffix(suffix)
        .ok_or_else(|| BuiltinError::new(span.clone(), "text does not end with suffix"))?;
    let value = make::text(text.to_owned(), Span::default());
    Ok(value)
}

// dec $strip_all_whitespace(text) : text

pub fn strip_all_whitespace(span: &Span, targs: &[Typ], values: &[Rc<Value>]) -> BuiltinResult {
    extract::zero(span, targs)?;
    let value_text = extract::one(span, values)?;
    let text = text_of_value(span, value_text)?.replace(' ', "");
    let value = make::text(text, Span::default());
    Ok(value)
}
