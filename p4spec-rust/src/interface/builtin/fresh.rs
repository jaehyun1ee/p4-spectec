//! Stateful fresh-type-id builtin.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::{
    lang::common::source::Span,
    lang::data::value::{Value, ValueArena, make},
    lang::il::ast::Typ,
};

use super::{BuiltinError, extract};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn init() {
    COUNTER.store(0, Ordering::Relaxed);
}

// dec $fresh_typeId() : typeId

pub fn fresh_type_id(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    extract::zero(targs)?;
    extract::zero(values)?;
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let type_id = format!("FRESH__{counter}");
    let value = make::text(arena, type_id, Span::default())?;
    Ok(value)
}
