//! Stateful fresh-type-id builtin.

use std::{
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::{
    lang::common::source::Span,
    lang::il::ast::Typ,
    runtime::value::{Value, make},
};

use super::{BuiltinError, extract};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn init() {
    COUNTER.store(0, Ordering::Relaxed);
}

// dec $fresh_typeId() : typeId

pub fn fresh_type_id(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    extract::zero(targs)?;
    extract::zero(values)?;
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let type_id = format!("FRESH__{counter}");
    let value = make::text(type_id, Span::default());
    Ok(value)
}
