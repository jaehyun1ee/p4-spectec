use std::rc::Rc;

use thiserror::Error;

use crate::{domain::source::Region, runtime::value::ValueRef};

pub mod extract;
pub mod ints;
pub mod lists;
pub mod maps;
pub mod nats;
pub mod sets;
pub mod texts;

// Error

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("{message} at {span}")]
pub struct BuiltinError {
    pub span: Region,
    pub message: String,
}

impl BuiltinError {
    pub fn new(span: Region, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }
}

pub type BuiltinResult = Result<ValueRef, BuiltinError>;

pub(crate) fn return_value(add: &mut dyn FnMut(ValueRef), value: ValueRef) -> BuiltinResult {
    add(Rc::clone(&value));
    Ok(value)
}
