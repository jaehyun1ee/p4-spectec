//! Stateful fresh-type-id builtin.

use crate::{
    lang::common::source::Span,
    lang::il::ast::Typ,
    runtime::value::{ValueRef, make},
};

use super::{BuiltinResult, extract};

// dec $fresh_typeId() : typeId

pub fn fresh_type_id(
    counter: &mut u64,
    span: &Span,
    targs: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, targs)?;
    extract::zero(span, values)?;
    let type_id = format!("FRESH__{counter}");
    *counter += 1;
    let value = make::text(type_id, Span::default());
    Ok(value)
}
