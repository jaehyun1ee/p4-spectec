use crate::{
    domain::source::Region,
    lang::il::ast::Typ,
    runtime::value::{ValueRef, make},
};

use super::{BuiltinResult, extract, return_value};

// dec $fresh_typeId() : typeId

pub fn fresh_type_id(
    counter: &mut u64,
    add: &mut dyn FnMut(ValueRef),
    span: &Region,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    extract::zero(span, type_args)?;
    extract::zero(span, values)?;
    let type_id = format!("FRESH__{counter}");
    *counter += 1;
    return_value(add, make::text(type_id, Region::none()))
}
